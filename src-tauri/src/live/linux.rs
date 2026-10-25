use super::{audio_levels, set_effect};
use crate::{controller::LightingState, transport::Transport};
use rustfft::FftPlanner;
use std::{
    env,
    io::Read,
    path::PathBuf,
    process::{Child, ChildStdout, Command, Stdio},
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc, Mutex,
    },
    thread::{self, JoinHandle},
    time::{Duration, Instant},
};

pub struct LiveSession {
    stop: Arc<AtomicBool>,
    child: Arc<Mutex<Option<Child>>>,
    worker: Option<JoinHandle<()>>,
}

impl Drop for LiveSession {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Relaxed);
        if let Ok(mut child) = self.child.lock() {
            if let Some(child) = child.as_mut() {
                let _ = child.kill();
            }
        }
        if let Some(worker) = self.worker.take() {
            let _ = worker.join();
        }
    }
}

fn find_command(name: &str) -> Option<PathBuf> {
    env::var_os("PATH")
        .into_iter()
        .flat_map(|path| env::split_paths(&path).collect::<Vec<_>>())
        .map(|path| path.join(name))
        .chain(std::iter::once(
            PathBuf::from("/run/current-system/sw/bin").join(name),
        ))
        .find(|path| path.is_file())
}

fn spawn_capture(
    command: &mut Command,
    child: &Arc<Mutex<Option<Child>>>,
    stop: &AtomicBool,
) -> Result<ChildStdout, String> {
    let mut slot = child.lock().map_err(|_| "Capture process lock failed")?;
    if stop.load(Ordering::Relaxed) {
        return Err("Capture stopped".into());
    }
    let mut process = command
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .map_err(|error| format!("Could not start capture: {error}"))?;
    let stdout = process.stdout.take().ok_or("Capture has no output")?;
    *slot = Some(process);
    Ok(stdout)
}

fn reap_capture(child: &Arc<Mutex<Option<Child>>>) {
    if let Ok(mut slot) = child.lock() {
        if let Some(mut process) = slot.take() {
            let _ = process.kill();
            let _ = process.wait();
        }
    }
}

fn ppm_average(bytes: &[u8]) -> Option<[u8; 4]> {
    if !bytes.starts_with(b"P6") {
        return None;
    }
    let mut cursor = 2;
    let mut tokens = Vec::new();
    while tokens.len() < 3 {
        while bytes.get(cursor)?.is_ascii_whitespace() {
            cursor += 1;
        }
        if bytes.get(cursor) == Some(&b'#') {
            while bytes.get(cursor)? != &b'\n' {
                cursor += 1;
            }
            continue;
        }
        let start = cursor;
        while !bytes.get(cursor)?.is_ascii_whitespace() {
            cursor += 1;
        }
        tokens.push(
            std::str::from_utf8(&bytes[start..cursor])
                .ok()?
                .parse::<usize>()
                .ok()?,
        );
    }
    if tokens[2] != 255 {
        return None;
    }
    // The header ends with one delimiter. Pixel data itself may begin with a
    // byte that happens to be ASCII whitespace and must not be skipped.
    if !bytes.get(cursor)?.is_ascii_whitespace() {
        return None;
    }
    cursor += 1;
    let pixels = bytes.get(cursor..)?;
    let stride = ((tokens[0] * tokens[1]) / 4096).max(1);
    let mut sum = [0u64; 3];
    let mut count = 0u64;
    for pixel in pixels.chunks_exact(3).step_by(stride) {
        for channel in 0..3 {
            // Squared samples preserve bright colors instead of washing them into gray.
            sum[channel] += (pixel[channel] as u64).pow(2);
        }
        count += 1;
    }
    if count == 0 {
        return None;
    }
    Some([
        ((sum[0] / count) as f64).sqrt() as u8,
        ((sum[1] / count) as f64).sqrt() as u8,
        ((sum[2] / count) as f64).sqrt() as u8,
        255,
    ])
}

pub fn start_screen(
    device: Arc<Mutex<()>>,
    lighting: Option<LightingState>,
) -> Result<LiveSession, String> {
    let grim = find_command("grim")
        .ok_or_else(|| "Screen sync needs the grim screenshot tool on Wayland.".to_string())?;
    set_effect(&device, "screen", lighting)?;
    let stop = Arc::new(AtomicBool::new(false));
    let signal = stop.clone();
    let child = Arc::new(Mutex::new(None));
    let process = child.clone();
    let worker = thread::spawn(move || {
        while !signal.load(Ordering::Relaxed) {
            let started = Instant::now();
            let mut command = Command::new(&grim);
            command.args(["-t", "ppm", "-"]);
            let Ok(mut stdout) = spawn_capture(&mut command, &process, &signal) else {
                break;
            };
            let mut output = Vec::new();
            let captured = stdout.read_to_end(&mut output).is_ok();
            reap_capture(&process);
            if signal.load(Ordering::Relaxed) {
                break;
            }
            if !captured {
                break;
            }
            let Some(color) = ppm_average(&output) else {
                break;
            };
            if let Ok(_guard) = device.try_lock() {
                if let Ok(transport) = Transport::open() {
                    let mut report = vec![0x0e];
                    report.extend_from_slice(&color);
                    let _ = transport.send(&report, Some(7));
                }
            }
            let elapsed = started.elapsed();
            if elapsed < Duration::from_millis(160) {
                thread::sleep(Duration::from_millis(160) - elapsed);
            }
        }
        reap_capture(&process);
    });
    Ok(LiveSession {
        stop,
        child,
        worker: Some(worker),
    })
}

pub fn start_audio(
    device: Arc<Mutex<()>>,
    lighting: Option<LightingState>,
) -> Result<LiveSession, String> {
    let pw_cat = find_command("pw-cat")
        .ok_or_else(|| "Audio sync needs PipeWire's pw-cat tool.".to_string())?;
    let stop = Arc::new(AtomicBool::new(false));
    let signal = stop.clone();
    let child = Arc::new(Mutex::new(None));
    let process = child.clone();
    let mut command = Command::new(pw_cat);
    command.args([
        "--record",
        "--raw",
        "--target",
        "@DEFAULT_AUDIO_SINK@",
        "--format",
        "f32",
        "--rate",
        "48000",
        "--channels",
        "2",
        "-",
    ]);
    let mut stdout = spawn_capture(&mut command, &process, &signal)?;
    if let Err(error) = set_effect(&device, "audio", lighting) {
        reap_capture(&process);
        return Err(error);
    }
    let worker = thread::spawn(move || {
        let mut planner = FftPlanner::new();
        let mut raw = vec![0u8; 2048 * 2 * 4];
        while !signal.load(Ordering::Relaxed) && stdout.read_exact(&mut raw).is_ok() {
            if signal.load(Ordering::Relaxed) {
                break;
            }
            let levels = audio_levels(&raw, &mut planner);
            if let Ok(_guard) = device.try_lock() {
                if let Ok(transport) = Transport::open() {
                    let mut report = [0u8; 64];
                    report[0] = 0x0d;
                    report[8..40].copy_from_slice(&levels);
                    let _ = transport.send(&report, Some(7));
                }
            }
        }
        reap_capture(&process);
    });
    Ok(LiveSession {
        stop,
        child,
        worker: Some(worker),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_ppm_average() {
        let ppm = b"P6\n2 1\n255\n\xff\0\0\0\xff\0";
        assert_eq!(ppm_average(ppm), Some([180, 180, 0, 255]));
    }

    #[test]
    fn silent_audio_has_zero_levels() {
        let mut planner = FftPlanner::new();
        assert_eq!(audio_levels(&vec![0; 2048 * 8], &mut planner), [0; 32]);
    }

    #[test]
    fn stopping_capture_interrupts_a_stalled_read() {
        let stop = Arc::new(AtomicBool::new(false));
        let child = Arc::new(Mutex::new(None));
        let mut command = Command::new("sleep");
        command.arg("60");
        let mut stdout = spawn_capture(&mut command, &child, &stop).unwrap();
        let process = child.clone();
        let (ready_tx, ready_rx) = std::sync::mpsc::channel();
        let worker = thread::spawn(move || {
            ready_tx.send(()).unwrap();
            let _ = stdout.read_exact(&mut [0u8; 8]);
            reap_capture(&process);
        });
        ready_rx.recv().unwrap();
        let session = LiveSession {
            stop,
            child,
            worker: Some(worker),
        };
        let (done_tx, done_rx) = std::sync::mpsc::channel();
        thread::spawn(move || {
            drop(session);
            let _ = done_tx.send(());
        });
        assert!(
            done_rx.recv_timeout(Duration::from_secs(2)).is_ok(),
            "Stop waited for capture data"
        );
    }
}
