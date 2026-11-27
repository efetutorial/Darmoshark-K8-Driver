//! Device access shared by the UI. Every method blocks on USB I/O and must be
//! called from a background task, never from the UI thread.

use crate::{
    controller::{Controller, LightingState, Status},
    display, live,
    requests::WriteRequest,
    transport,
};
use std::{
    path::Path,
    sync::{Arc, Mutex, MutexGuard},
};

#[derive(Clone, Debug, Default, PartialEq)]
pub struct Progress {
    pub active: bool,
    pub value: f64,
    pub label: String,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LiveKind {
    Screen,
    Audio,
}

#[derive(Clone, Debug)]
pub struct UploadResult {
    pub frames: usize,
    pub delay: u8,
}

pub struct Backend {
    device: Arc<Mutex<()>>,
    progress: Mutex<Progress>,
    live: Mutex<Option<(LiveKind, live::LiveSession)>>,
    upload: Mutex<()>,
}

impl Backend {
    pub fn new() -> Arc<Self> {
        Arc::new(Self {
            device: Arc::new(Mutex::new(())),
            progress: Mutex::new(Progress::default()),
            live: Mutex::new(None),
            upload: Mutex::new(()),
        })
    }

    pub fn connected(&self) -> bool {
        !transport::discover().is_empty()
    }

    fn controller(&self) -> Result<(MutexGuard<'_, ()>, Controller), String> {
        let guard = self
            .device
            .lock()
            .map_err(|_| "Keyboard lock failed".to_string())?;
        Ok((guard, Controller::open()?))
    }

    pub fn status(&self) -> Result<Status, String> {
        let (_guard, controller) = self.controller()?;
        controller.status()
    }

    /// Validate and run a write. Lighting writes stop live sync first so the
    /// worker cannot immediately overwrite the new effect.
    pub fn execute(&self, request: WriteRequest) -> Result<(), String> {
        request.validate()?;
        if matches!(request, WriteRequest::Lighting(_)) {
            self.stop_live();
        }
        let (_guard, controller) = self.controller()?;
        request.execute(&controller)
    }

    pub fn start_live(&self, kind: LiveKind, lighting: LightingState) -> Result<(), String> {
        lighting.validate()?;
        let mut session = self.live.lock().map_err(|_| "Live sync lock failed")?;
        session.take();
        let started = match kind {
            LiveKind::Screen => live::start_screen(self.device.clone(), Some(lighting))?,
            LiveKind::Audio => live::start_audio(self.device.clone(), Some(lighting))?,
        };
        *session = Some((kind, started));
        Ok(())
    }

    pub fn stop_live(&self) {
        if let Ok(mut session) = self.live.lock() {
            session.take();
        }
    }

    pub fn live_kind(&self) -> Option<LiveKind> {
        self.live
            .lock()
            .ok()
            .and_then(|session| session.as_ref().map(|(kind, _)| *kind))
    }

    pub fn progress(&self) -> Progress {
        self.progress
            .lock()
            .map(|progress| progress.clone())
            .unwrap_or_default()
    }

    fn set_progress(&self, active: bool, value: f64, label: &str) {
        if let Ok(mut progress) = self.progress.lock() {
            *progress = Progress {
                active,
                value,
                label: label.into(),
            };
        }
    }

    pub fn upload_display(
        &self,
        path: &Path,
        layer: u8,
        delay: Option<u8>,
    ) -> Result<UploadResult, String> {
        let _upload = self
            .upload
            .try_lock()
            .map_err(|_| "A display upload is already running".to_string())?;
        self.set_progress(true, 0.0, "Preparing image");
        let result = self.upload_inner(path, layer, delay);
        if result.is_err() {
            self.set_progress(false, 0.0, "Upload failed");
        }
        result
    }

    fn upload_inner(
        &self,
        path: &Path,
        layer: u8,
        delay: Option<u8>,
    ) -> Result<UploadResult, String> {
        if layer > 4 {
            return Err("Display slot must be 1..5".into());
        }
        if delay == Some(0) {
            return Err("GIF delay must be 1..255 ms".into());
        }
        let data = std::fs::read(path).map_err(|error| format!("Could not read image: {error}"))?;
        let filename = path
            .file_name()
            .map(|name| name.to_string_lossy().into_owned())
            .unwrap_or_else(|| "image.png".into());
        let animation = display::decode(&data, &filename, delay)?;
        self.stop_live();
        self.set_progress(true, 0.0, "Uploading");
        let (_guard, controller) = self.controller()?;
        display::upload(&controller.transport, &animation, layer, |value, label| {
            self.set_progress(value < 1.0, value, &label)
        })?;
        Ok(UploadResult {
            frames: animation.frames.len(),
            delay: animation.delay,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{controller::KeyboardOptions, keys};

    #[test]
    fn failed_image_preparation_clears_progress() {
        let backend = Backend::new();
        let path = std::env::temp_dir().join(format!("k8-invalid-{}.png", std::process::id()));
        std::fs::write(&path, [0u8, 1, 2]).unwrap();
        let result = backend.upload_display(&path, 0, None);
        let _ = std::fs::remove_file(&path);
        assert!(result.is_err());
        let progress = backend.progress();
        assert!(!progress.active);
        assert_eq!(progress.label, "Upload failed");
    }

    #[test]
    fn invalid_requests_are_rejected_before_device_access() {
        let backend = Backend::new();
        let error = backend
            .execute(WriteRequest::Profile { profile: 3 })
            .unwrap_err();
        assert_eq!(error, "Profile must be 0..2");
    }

    #[test]
    fn named_targets_are_stable() {
        assert_eq!(
            keys::parse_target("media:volume-up", 0, 0).unwrap(),
            [3, 0, 233, 0]
        );
        assert_eq!(keys::matrix_index(57).unwrap(), 3);
    }

    #[test]
    fn options_default_is_clear() {
        assert!(!KeyboardOptions::default().win_lock);
    }

    #[test]
    #[ignore = "requires the attached K8 and changes lighting briefly"]
    fn hardware_lighting_and_live_sync_round_trip() {
        use std::{thread, time::Duration};
        let controller = Controller::open().unwrap();
        let original = controller.get_lighting().unwrap();
        let requested = LightingState {
            effect: "solid".into(),
            brightness: 3,
            speed: 2,
            color: "#123456".into(),
            rainbow: false,
            direction: "right".into(),
            slot: 1,
        };
        controller.set_lighting(&requested).unwrap();
        let actual = controller.get_lighting().unwrap();
        drop(controller);
        let live_result = (|| -> Result<(), String> {
            let lock = Arc::new(Mutex::new(()));
            let screen = live::start_screen(lock.clone(), None)?;
            thread::sleep(Duration::from_millis(350));
            drop(screen);
            let audio = live::start_audio(lock, None)?;
            thread::sleep(Duration::from_millis(350));
            drop(audio);
            Ok(())
        })();
        Controller::open().unwrap().set_lighting(&original).unwrap();
        live_result.unwrap();

        assert_eq!(actual.effect, requested.effect);
        assert_eq!(actual.color, requested.color);
        assert!(!actual.rainbow);
    }
}
