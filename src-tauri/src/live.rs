#[cfg(target_os = "linux")]
mod linux;
#[cfg(target_os = "windows")]
mod windows;
#[cfg(target_os = "linux")]
pub use linux::{start_audio, start_screen, LiveSession};
#[cfg(target_os = "windows")]
pub use windows::{start_audio, start_screen, LiveSession};

use crate::controller::{Controller, LightingState};
use rustfft::{num_complex::Complex, FftPlanner};
use std::sync::{Arc, Mutex};

pub(super) fn set_effect(
    device: &Arc<Mutex<()>>,
    kind: &str,
    value: Option<LightingState>,
) -> Result<(), String> {
    let _guard = device.lock().map_err(|_| "Keyboard lock failed")?;
    let controller = Controller::open()?;
    let mut value = match value {
        Some(value) => value,
        None => controller.get_lighting()?,
    };
    value.effect = if kind == "screen" {
        "screen_color".into()
    } else if value.effect == "music_2" {
        "music_2".into()
    } else {
        "music_3".into()
    };
    controller.set_lighting(&value)
}

pub(super) fn audio_levels(raw: &[u8], planner: &mut FftPlanner<f32>) -> [u8; 32] {
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
