use super::{audio_levels, set_effect};
use crate::{
    controller::LightingState,
    transport::Transport,
    windows_capture::{self, AudioCapture},
};
use rustfft::FftPlanner;
use std::{
    sync::{
        atomic::{AtomicBool, Ordering},
        mpsc, Arc, Mutex,
    },
    thread::{self, JoinHandle},
    time::Duration,
};

pub struct LiveSession {
    stop: Arc<AtomicBool>,
    worker: Option<JoinHandle<()>>,
}
impl Drop for LiveSession {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Relaxed);
        if let Some(worker) = self.worker.take() {
            let _ = worker.join();
        }
    }
}

enum Capture {
    Screen,
    Audio(AudioCapture),
}

fn start(
    device: Arc<Mutex<()>>,
    lighting: Option<LightingState>,
    screen: bool,
) -> Result<LiveSession, String> {
    let stop = Arc::new(AtomicBool::new(false));
    let signal = stop.clone();
    let worker_device = device.clone();
    let (ready_tx, ready_rx) = mpsc::sync_channel(1);
    let (go_tx, go_rx) = mpsc::sync_channel(1);
    let worker = thread::spawn(move || {
        // Initialize capture on its own thread; WASAPI's COM objects never move
        // between threads. Report setup errors before changing keyboard state.
        let capture = if screen {
            windows_capture::screen(64).map(|_| Capture::Screen)
        } else {
            AudioCapture::open().map(Capture::Audio)
        };
        let mut capture = match capture {
            Ok(capture) => {
                let _ = ready_tx.send(Ok(()));
                capture
            }
            Err(error) => {
                let _ = ready_tx.send(Err(error));
                return;
            }
        };
        if go_rx.recv().is_err() {
            return;
        }
        let mut planner = FftPlanner::new();
        while !signal.load(Ordering::Relaxed) {
            let report = match &mut capture {
                Capture::Screen => {
                    let Ok(image) = windows_capture::screen(64) else {
                        break;
                    };
                    let mut sum = [0u64; 3];
                    for pixel in image.pixels() {
                        for channel in 0..3 {
                            sum[channel] += (pixel[channel] as u64).pow(2);
                        }
                    }
                    let mut report = [0; 64];
                    report[0] = 0x0e;
                    for channel in 0..3 {
                        report[channel + 1] = ((sum[channel] / 4096) as f64).sqrt() as u8;
                    }
                    report[4] = 255;
                    report
                }
                Capture::Audio(capture) => {
                    let Ok(raw) = capture.read() else {
                        break;
                    };
                    let mut report = [0; 64];
                    report[0] = 0x0d;
                    report[8..40].copy_from_slice(&audio_levels(&raw, &mut planner));
                    report
                }
            };
            if signal.load(Ordering::Relaxed) {
                break;
            }
            if let Ok(_guard) = worker_device.try_lock() {
                if let Ok(transport) = Transport::open() {
                    let _ = transport.send(&report, Some(7));
                }
            }
            thread::sleep(Duration::from_millis(if screen { 160 } else { 42 }));
        }
    });
    let session = LiveSession {
        stop,
        worker: Some(worker),
    };
    // Drop the go sender before joining on failure, releasing a worker waiting
    // for settings to be applied.
    let result = ready_rx
        .recv()
        .map_err(|_| "Capture worker stopped during setup".to_string())
        .and_then(|result| result)
        .and_then(|_| set_effect(&device, if screen { "screen" } else { "audio" }, lighting));
    if let Err(error) = result {
        drop(go_tx);
        drop(session);
        return Err(error);
    }
    go_tx
        .send(())
        .map_err(|_| "Capture worker stopped during setup".to_string())?;
    Ok(session)
}

pub fn start_screen(
    device: Arc<Mutex<()>>,
    lighting: Option<LightingState>,
) -> Result<LiveSession, String> {
    start(device, lighting, true)
}
pub fn start_audio(
    device: Arc<Mutex<()>>,
    lighting: Option<LightingState>,
) -> Result<LiveSession, String> {
    start(device, lighting, false)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn stopping_an_idle_worker_is_bounded() {
        let stop = Arc::new(AtomicBool::new(false));
        let signal = stop.clone();
        let worker = thread::spawn(move || {
            while !signal.load(Ordering::Relaxed) {
                thread::sleep(Duration::from_millis(10));
            }
        });
        let session = LiveSession {
            stop,
            worker: Some(worker),
        };
        let (tx, rx) = mpsc::channel();
        thread::spawn(move || {
            drop(session);
            tx.send(()).unwrap();
        });
        rx.recv_timeout(Duration::from_secs(2))
            .expect("Stop hung during silence");
    }
}
