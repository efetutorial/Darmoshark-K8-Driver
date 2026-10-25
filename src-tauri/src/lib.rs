mod controller;
mod display;
mod keys;
mod live;
mod requests;
mod transport;
#[cfg(target_os = "windows")]
mod windows_capture;

use controller::{Controller, LightingState};
use serde::Serialize;
use serde_json::{json, Value};
use std::{
    collections::HashMap,
    env,
    sync::{Arc, Mutex},
};
#[cfg(target_os = "linux")]
use tauri::Manager;

#[derive(Clone, Serialize)]
struct Progress {
    active: bool,
    value: f64,
    label: String,
}
impl Default for Progress {
    fn default() -> Self {
        Self {
            active: false,
            value: 0.0,
            label: String::new(),
        }
    }
}
struct AppState {
    device: Arc<Mutex<()>>,
    progress: Mutex<Progress>,
    live: Mutex<Option<live::LiveSession>>,
    upload: Mutex<()>,
}

fn request_body<'a>(value: &'a Option<Value>) -> Result<&'a Value, String> {
    value
        .as_ref()
        .ok_or_else(|| "Request body is missing".into())
}
fn text_value<'a>(value: &'a Value, name: &str) -> Result<&'a str, String> {
    value
        .get(name)
        .and_then(Value::as_str)
        .ok_or_else(|| format!("{name} is missing"))
}
fn controller_locked(
    state: &AppState,
) -> Result<(std::sync::MutexGuard<'_, ()>, Controller), String> {
    let guard = state
        .device
        .lock()
        .map_err(|_| "Keyboard lock failed".to_string())?;
    let controller = Controller::open()?;
    Ok((guard, controller))
}

#[tauri::command]
async fn api(
    state: tauri::State<'_, Arc<AppState>>,
    path: String,
    body_value: Option<Value>,
    bytes: Option<Vec<u8>>,
    query: Option<HashMap<String, String>>,
) -> Result<Value, String> {
    let state = state.inner().clone();
    tauri::async_runtime::spawn_blocking(move || handle_api(&state, path, body_value, bytes, query))
        .await
        .map_err(|error| format!("Keyboard operation failed: {error}"))?
}

fn handle_api(
    state: &AppState,
    path: String,
    body_value: Option<Value>,
    bytes: Option<Vec<u8>>,
    query: Option<HashMap<String, String>>,
) -> Result<Value, String> {
    if path == "/api/live-native-stop" {
        state
            .live
            .lock()
            .map_err(|_| "Live sync lock failed")?
            .take();
        return Ok(json!({"ok":true}));
    }
    if path == "/api/live-native-start" {
        let data = request_body(&body_value)?;
        let kind = text_value(data, "kind")?;
        if !["screen", "audio"].contains(&kind) {
            return Err(format!("Unknown live sync type: {kind}"));
        }
        let lighting = data
            .get("lighting")
            .cloned()
            .map(serde_json::from_value::<LightingState>)
            .transpose()
            .map_err(|error| error.to_string())?;
        if let Some(value) = &lighting {
            value.validate()?;
        }
        let mut session = state.live.lock().map_err(|_| "Live sync lock failed")?;
        session.take();
        *session = Some(match kind {
            "screen" => live::start_screen(state.device.clone(), lighting)?,
            "audio" => live::start_audio(state.device.clone(), lighting)?,
            _ => return Err(format!("Unknown live sync type: {kind}")),
        });
        return Ok(json!({"ok":true}));
    }

    match path.as_str() {
        "/api/device" => {
            let devices = transport::discover();
            Ok(
                json!({"connected":!devices.is_empty(),"devices":devices,"keys":keys::keys(),"targets":keys::target_names(),"effects":[]}),
            )
        }
        "/api/progress" => Ok(serde_json::to_value(
            state
                .progress
                .lock()
                .map_err(|_| "Progress lock failed")?
                .clone(),
        )
        .unwrap()),
        "/api/system-card" => {
            let (memory_percent, uptime_seconds) = display::system_card()?;
            Ok(json!({"ok":true,"memory_percent":memory_percent,"uptime_seconds":uptime_seconds}))
        }
        "/api/display-capture" => Ok(json!({"ok":true,"png":display::capture_screen()?})),
        "/api/status" => {
            let (_guard, controller) = controller_locked(&state)?;
            Ok(json!({"ok":true,"status":controller.status()?}))
        }
        "/api/display" | "/api/display-frame" => {
            let _upload = state
                .upload
                .try_lock()
                .map_err(|_| "A display upload is already running")?;
            *state.progress.lock().map_err(|_| "Progress lock failed")? = Progress {
                active: true,
                value: 0.0,
                label: "Preparing image".into(),
            };
            let result = upload_display(state, &path, bytes, query);
            if result.is_err() {
                *state.progress.lock().map_err(|_| "Progress lock failed")? = Progress {
                    active: false,
                    value: 0.0,
                    label: "Upload failed".into(),
                };
            }
            result
        }
        _ => {
            let request = requests::WriteRequest::parse(&path, body_value)?;
            if matches!(request, requests::WriteRequest::Lighting(_)) {
                state
                    .live
                    .lock()
                    .map_err(|_| "Live sync lock failed")?
                    .take();
            }
            let (_guard, controller) = controller_locked(state)?;
            request.execute(&controller)?;
            Ok(json!({"ok":true}))
        }
    }
}

fn upload_display(
    state: &AppState,
    path: &str,
    bytes: Option<Vec<u8>>,
    query: Option<HashMap<String, String>>,
) -> Result<Value, String> {
    let data = bytes.ok_or_else(|| "Image data is missing".to_string())?;
    let query = query.unwrap_or_default();
    let layer = query
        .get("layer")
        .map(|value| value.parse::<u8>())
        .transpose()
        .map_err(|_| "Invalid display slot")?
        .unwrap_or(0);
    if layer > 4 {
        return Err("Display slot must be 1..5".into());
    }
    if query.get("single").is_some_and(|value| value == "true") {
        return Err("This window uses an old display mode. Restart the application to upload GIFs and individual slots.".into());
    }
    let animation = if path == "/api/display-frame" {
        display::encode(vec![data], 100)?
    } else {
        let filename = query
            .get("filename")
            .map(String::as_str)
            .unwrap_or("image.png");
        let delay = query
            .get("delay")
            .map(|value| value.parse::<u8>())
            .transpose()
            .map_err(|_| "GIF delay must be 1..255 ms")?;
        if delay == Some(0) {
            return Err("GIF delay must be 1..255 ms".into());
        }
        display::decode(&data, filename, delay)?
    };
    state
        .live
        .lock()
        .map_err(|_| "Live sync lock failed")?
        .take();
    *state.progress.lock().map_err(|_| "Progress lock failed")? = Progress {
        active: true,
        value: 0.0,
        label: "Uploading".into(),
    };
    let result = (|| {
        let (_guard, controller) = controller_locked(state)?;
        let progress = |value, label| {
            if let Ok(mut progress) = state.progress.lock() {
                *progress = Progress {
                    active: value < 1.0,
                    value,
                    label,
                }
            }
        };
        display::upload(&controller.transport, &animation, layer, progress)
    })();
    result?;
    Ok(
        json!({"ok":true,"frames":animation.frames.len(),"delay":animation.delay,"bounds":animation.bounds}),
    )
}

#[tauri::command]
fn frontend_report(report: String) {
    eprintln!("K8 Studio frontend: {report}")
}

pub fn run() {
    #[cfg(target_os = "linux")]
    {
        if env::var_os("GSETTINGS_SCHEMA_DIR").is_none() {
            let system = std::path::PathBuf::from("/run/current-system/sw/share/glib-2.0/schemas");
            let mut schema_dir = system.join("gschemas.compiled").exists().then_some(system);
            if schema_dir.is_none() {
                'packages: for package in ["gtk+3-", "gsettings-desktop-schemas-"] {
                    if let Ok(entries) = std::fs::read_dir("/nix/store") {
                        for entry in entries.flatten() {
                            if !entry.file_name().to_string_lossy().contains(package) {
                                continue;
                            }
                            let roots = entry.path().join("share/gsettings-schemas");
                            if let Ok(versions) = std::fs::read_dir(roots) {
                                for version in versions.flatten() {
                                    let candidate = version.path().join("glib-2.0/schemas");
                                    if candidate.join("gschemas.compiled").exists() {
                                        schema_dir = Some(candidate);
                                        break 'packages;
                                    }
                                }
                            }
                        }
                    }
                }
            }
            if let Some(path) = schema_dir {
                env::set_var("GSETTINGS_SCHEMA_DIR", path);
            }
        }
        if env::var_os("GDK_BACKEND").is_none() {
            env::set_var("GDK_BACKEND", "x11")
        }
        if env::var_os("WEBKIT_DISABLE_DMABUF_RENDERER").is_none() {
            env::set_var("WEBKIT_DISABLE_DMABUF_RENDERER", "1")
        }
    }
    tauri::Builder::default()
        .manage(Arc::new(AppState {
            device: Arc::new(Mutex::new(())),
            progress: Mutex::new(Progress::default()),
            live: Mutex::new(None),
            upload: Mutex::new(()),
        }))
        .setup(|_app| {
            #[cfg(target_os = "linux")]
            if let Some(window) = _app.get_webview_window("main") {
                window.with_webview(|webview| {
                    use webkit2gtk::{
                        glib::prelude::ObjectExt, PermissionRequestExt, UserMediaPermissionRequest,
                        WebViewExt,
                    };
                    webview.inner().connect_permission_request(|_, request| {
                        if request.is::<UserMediaPermissionRequest>() {
                            request.allow();
                            true
                        } else {
                            false
                        }
                    });
                })?;
            }
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![api, frontend_report])
        .run(tauri::generate_context!())
        .expect("Could not run Darmoshark K8 Studio");
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{thread, time::Duration};

    #[test]
    fn failed_image_preparation_clears_progress() {
        let state = AppState {
            device: Arc::new(Mutex::new(())),
            progress: Mutex::new(Progress::default()),
            live: Mutex::new(None),
            upload: Mutex::new(()),
        };
        let result = handle_api(
            &state,
            "/api/display".into(),
            None,
            Some(vec![0, 1, 2]),
            None,
        );
        assert!(result.is_err());
        let progress = state.progress.lock().unwrap();
        assert!(!progress.active);
        assert_eq!(progress.label, "Upload failed");
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
        assert!(!controller::KeyboardOptions::default().win_lock);
    }

    #[test]
    #[ignore = "requires the attached K8 and changes lighting briefly"]
    fn hardware_lighting_and_live_sync_round_trip() {
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
