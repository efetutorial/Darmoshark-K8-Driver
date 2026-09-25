use crate::{
    controller::{Controller, LightingState},
    transport::Transport,
};
use rustfft::{num_complex::Complex, FftPlanner};
use std::{
    io::Read,
    path::PathBuf,
    process::{Command, Stdio},
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc, Mutex,
    },
    thread::{self, JoinHandle},
    time::{Duration, Instant},
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

fn find_command(name: &str) -> Option<PathBuf> {
    let system_path = PathBuf::from("/run/current-system/sw/bin").join(name);
    if system_path.exists() {
        return Some(system_path);
    }
    let path = PathBuf::from(name);
    Command::new(&path)
        .arg("--help")
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .map(|status| status.success())
        .unwrap_or(false)
        .then_some(path)
}

fn set_effect(device: &Arc<Mutex<()>>, effect: &str) -> Result<(), String> {
    let _guard = device.lock().map_err(|_| "Keyboard lock failed")?;
    Controller::open()?.set_lighting(&LightingState {
        effect: effect.into(),
        brightness: 4,
        speed: 0,
        color: if effect == "screen_color" {
            "#000000"
        } else {
            "#ffffff"
        }
        .into(),
        rainbow: effect != "screen_color",
        direction: "right".into(),
        slot: 1,
    })
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

fn audio_levels(raw: &[u8], planner: &mut FftPlanner<f32>) -> [u8; 32] {
    const FFT_SIZE: usize = 2048;
    let stereo: Vec<f32> = raw
        .chunks_exact(8)
        .map(|sample| {
            let left = f32::from_le_bytes(sample[0..4].try_into().unwrap());
            let right = f32::from_le_bytes(sample[4..8].try_into().unwrap());
            (left + right) * 0.5
        })
        .collect();
    let start = stereo.len().saturating_sub(FFT_SIZE);
    let mut spectrum = vec![Complex::new(0.0, 0.0); FFT_SIZE];
    for (index, sample) in stereo[start..].iter().enumerate() {
        let window = 0.5 - 0.5 * (std::f32::consts::TAU * index as f32 / FFT_SIZE as f32).cos();
        spectrum[index].re = sample * window;
    }
    planner.plan_fft_forward(FFT_SIZE).process(&mut spectrum);
    let mut energy = [0.0f32; 32];
    for (band, value) in energy.iter_mut().enumerate() {
        let low = (2.0f32 * (256.0f32).powf(band as f32 / 32.0)) as usize;
        let high = (2.0f32 * (256.0f32).powf((band + 1) as f32 / 32.0)) as usize;
        *value = spectrum[low.min(1023)..high.clamp(low + 1, 1024)]
            .iter()
            .map(|sample| sample.norm_sqr())
            .sum::<f32>()
            .sqrt();
    }
    let peak = energy.iter().copied().fold(0.0001, f32::max);
    energy.map(|value| ((value / peak).sqrt() * 6.0).round().clamp(0.0, 6.0) as u8)
}

pub fn start_screen(device: Arc<Mutex<()>>) -> Result<LiveSession, String> {
    let grim = find_command("grim")
        .ok_or_else(|| "Screen sync needs the grim screenshot tool on Wayland.".to_string())?;
    set_effect(&device, "screen_color")?;
    let stop = Arc::new(AtomicBool::new(false));
    let signal = stop.clone();
    let worker = thread::spawn(move || {
        while !signal.load(Ordering::Relaxed) {
            let started = Instant::now();
            if let Ok(output) = Command::new(&grim).args(["-t", "ppm", "-"]).output() {
                if let Some(color) = ppm_average(&output.stdout) {
                    if let Ok(_guard) = device.lock() {
                        if let Ok(transport) = Transport::open() {
                            let mut report = vec![0x0e];
                            report.extend_from_slice(&color);
                            let _ = transport.send(&report, Some(7));
                        }
                    }
                }
            }
            let elapsed = started.elapsed();
            if elapsed < Duration::from_millis(160) {
                thread::sleep(Duration::from_millis(160) - elapsed);
            }
        }
    });
    Ok(LiveSession {
        stop,
        worker: Some(worker),
    })
}

pub fn start_audio(device: Arc<Mutex<()>>) -> Result<LiveSession, String> {
    let pw_cat = find_command("pw-cat")
        .ok_or_else(|| "Audio sync needs PipeWire's pw-cat tool.".to_string())?;
    set_effect(&device, "music_3")?;
    let mut child = Command::new(pw_cat)
        .args([
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
        ])
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .map_err(|error| format!("Could not start audio capture: {error}"))?;
    let mut stdout = child.stdout.take().ok_or("Audio capture has no output")?;
    let stop = Arc::new(AtomicBool::new(false));
    let signal = stop.clone();
    let worker = thread::spawn(move || {
        let mut planner = FftPlanner::new();
        let mut raw = vec![0u8; 2048 * 2 * 4];
        while !signal.load(Ordering::Relaxed) && stdout.read_exact(&mut raw).is_ok() {
            let levels = audio_levels(&raw, &mut planner);
            if let Ok(_guard) = device.lock() {
                if let Ok(transport) = Transport::open() {
                    let mut report = [0u8; 64];
                    report[0] = 0x0d;
                    report[8..40].copy_from_slice(&levels);
                    let _ = transport.send(&report, Some(7));
                }
            }
        }
        let _ = child.kill();
        let _ = child.wait();
    });
    Ok(LiveSession {
        stop,
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
}
