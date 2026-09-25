use crate::transport::Transport;
use base64::{engine::general_purpose::STANDARD, Engine};
use image::{
    codecs::gif::GifDecoder, imageops::FilterType, AnimationDecoder, DynamicImage, GenericImage,
    ImageFormat, RgbaImage,
};
use std::{
    fs,
    io::Cursor,
    path::Path,
    process::{Command, Stdio},
    thread,
    time::Duration,
};

pub const WIDTH: u32 = 128;
pub const HEIGHT: u32 = 128;
const MAX_FRAMES: usize = 222;

pub struct Animation {
    pub frames: Vec<Vec<u8>>,
    pub delay: u8,
    pub bounds: [u16; 4],
}

fn fit_frame(image: DynamicImage) -> Result<Vec<u8>, String> {
    let mut resized = image.resize(WIDTH, HEIGHT, FilterType::Lanczos3).to_rgba8();
    for pixel in resized.pixels_mut() {
        let alpha = pixel[3] as u16;
        pixel[0] = (pixel[0] as u16 * alpha / 255) as u8;
        pixel[1] = (pixel[1] as u16 * alpha / 255) as u8;
        pixel[2] = (pixel[2] as u16 * alpha / 255) as u8;
        pixel[3] = 255;
    }
    let mut canvas = RgbaImage::new(WIDTH, HEIGHT);
    let x = (WIDTH - resized.width()) / 2;
    let y = (HEIGHT - resized.height()) / 2;
    canvas
        .copy_from(&resized, x, y)
        .map_err(|error| error.to_string())?;
    Ok(canvas.into_raw())
}

pub fn decode(
    bytes: &[u8],
    filename: &str,
    delay_override: Option<u8>,
) -> Result<Animation, String> {
    let format =
        image::guess_format(bytes).map_err(|error| format!("Unsupported image: {error}"))?;
    let mut delay = delay_override.unwrap_or(100);
    let frames = if format == ImageFormat::Gif || filename.to_ascii_lowercase().ends_with(".gif") {
        let decoder = GifDecoder::new(Cursor::new(bytes))
            .map_err(|error| format!("Could not decode GIF: {error}"))?;
        let decoded = decoder
            .into_frames()
            .collect_frames()
            .map_err(|error| format!("Could not decode GIF: {error}"))?;
        if decoded.len() > MAX_FRAMES {
            return Err(format!(
                "GIF has {} frames; K8 limit is {MAX_FRAMES}",
                decoded.len()
            ));
        }
        let mut result = Vec::with_capacity(decoded.len());
        for (index, frame) in decoded.into_iter().enumerate() {
            if index == 0 && delay_override.is_none() {
                let (numerator, denominator) = frame.delay().numer_denom_ms();
                delay = (numerator / denominator.max(1)).clamp(1, 255) as u8;
            }
            result.push(fit_frame(DynamicImage::ImageRgba8(frame.into_buffer()))?);
        }
        result
    } else {
        vec![fit_frame(image::load_from_memory(bytes).map_err(
            |error| format!("Could not decode image: {error}"),
        )?)?]
    };
    encode(frames, delay)
}

pub fn encode(frames: Vec<Vec<u8>>, delay: u8) -> Result<Animation, String> {
    if frames.is_empty() || frames.len() > MAX_FRAMES {
        return Err("Invalid frame count".into());
    }
    if frames
        .iter()
        .any(|frame| frame.len() != (WIDTH * HEIGHT * 4) as usize)
    {
        return Err("Expected 128 x 128 RGBA frames".into());
    }
    let mut left = WIDTH;
    let mut top = HEIGHT;
    let mut right = 0;
    let mut bottom = 0;
    for frame in &frames {
        for y in 0..HEIGHT {
            for x in 0..WIDTH {
                let p = ((y * WIDTH + x) * 4) as usize;
                if frame[p] != 0 || frame[p + 1] != 0 || frame[p + 2] != 0 {
                    left = left.min(x);
                    top = top.min(y);
                    right = right.max(x + 1);
                    bottom = bottom.max(y + 1)
                }
            }
        }
    }
    let empty = right == 0;
    let bounds = if empty {
        [0, 0, 0, 0]
    } else {
        [left as u16, top as u16, right as u16, bottom as u16]
    };
    let mut encoded = Vec::with_capacity(frames.len());
    for frame in frames {
        let mut output = Vec::new();
        let (xs, xe, ys, ye) = if empty {
            (0, WIDTH, 0, HEIGHT)
        } else {
            (left, right, top, bottom)
        };
        for x in xs..xe {
            for y in ys..ye {
                let p = ((y * WIDTH + x) * 4) as usize;
                let value = ((frame[p] as u16 >> 3) << 11)
                    | ((frame[p + 1] as u16 >> 2) << 5)
                    | (frame[p + 2] as u16 >> 3);
                output.extend_from_slice(&value.to_be_bytes())
            }
        }
        encoded.push(output)
    }
    Ok(Animation {
        frames: encoded,
        delay: delay.clamp(1, 255),
        bounds,
    })
}

pub fn upload(
    transport: &Transport,
    animation: &Animation,
    layer: u8,
    progress: impl Fn(f64, String),
) -> Result<(), String> {
    if layer > 4 {
        return Err("Display slot must be 1..5".into());
    }
    let count = animation.frames.len();
    let size = animation.frames[0].len();
    if animation.frames.iter().any(|frame| frame.len() != size) {
        return Err("Animation frames have different bounds".into());
    }
    let mut handshake = [0u8; 64];
    handshake[..4].copy_from_slice(&[
        0xa5,
        if count == 1 { layer } else { 0 },
        count as u8,
        if count > 1 { animation.delay } else { 0 },
    ]);
    handshake[4..6].copy_from_slice(&(size as u16).to_le_bytes());
    for i in 0..4 {
        handshake[8 + i] = animation.bounds[i] as u8;
        handshake[12 + i] = (animation.bounds[i] >> 8) as u8
    }
    handshake[16..18].copy_from_slice(&((size as u32) >> 16).to_le_bytes()[..2]);
    let reply = transport.query_any(&handshake, Some(7), Duration::from_millis(100))?;
    if reply[1] != 1 {
        return Err(
            "Upload rejected. Check that the display is on and the keyboard is in wired mode."
                .into(),
        );
    }
    thread::sleep(Duration::from_millis(100));
    let packets = size.div_ceil(56);
    let total = packets * count;
    let mut sent = 0;
    for (frame_index, frame) in animation.frames.iter().enumerate() {
        for (packet_index, chunk) in frame.chunks(56).enumerate() {
            let mut packet = [0u8; 64];
            packet[..4].copy_from_slice(&[
                0x25,
                if count == 1 { layer } else { frame_index as u8 },
                count as u8,
                if count > 1 { animation.delay } else { 0 },
            ]);
            packet[4..6].copy_from_slice(&(packet_index as u16).to_le_bytes());
            packet[6] = chunk.len() as u8;
            packet[8..8 + chunk.len()].copy_from_slice(chunk);
            transport.send(&packet, Some(7))?;
            sent += 1;
            progress(
                sent as f64 / total as f64,
                format!("Frame {}/{}", frame_index + 1, count),
            );
            thread::sleep(Duration::from_millis(1))
        }
    }
    progress(1.0, "Complete".into());
    Ok(())
}

pub fn capture_screen() -> Result<String, String> {
    let grim = ["grim", "/run/current-system/sw/bin/grim"]
        .into_iter()
        .find(|command| {
            Path::new(command).is_absolute() && Path::new(command).exists()
                || Command::new(command)
                    .arg("--version")
                    .stdout(Stdio::null())
                    .stderr(Stdio::null())
                    .status()
                    .map(|status| status.success())
                    .unwrap_or(false)
        })
        .ok_or("Screen capture needs the grim screenshot tool.")?;
    let output = Command::new(grim)
        .args(["-t", "png", "-"])
        .output()
        .map_err(|error| format!("Could not capture screen: {error}"))?;
    if !output.status.success() || output.stdout.is_empty() {
        return Err("Could not capture screen".into());
    }
    let bytes = output.stdout;
    let image = image::load_from_memory(&bytes)
        .map_err(|error| format!("Could not decode screenshot: {error}"))?;
    let resized = image.resize(512, 512, FilterType::Lanczos3);
    let mut png = Cursor::new(Vec::new());
    resized
        .write_to(&mut png, ImageFormat::Png)
        .map_err(|error| error.to_string())?;
    Ok(STANDARD.encode(png.into_inner()))
}

pub fn system_card() -> Result<(u8, u64), String> {
    let memory = fs::read_to_string("/proc/meminfo").map_err(|error| error.to_string())?;
    let value = |name: &str| -> Option<u64> {
        memory
            .lines()
            .find(|line| line.starts_with(name))
            .and_then(|line| line.split_whitespace().nth(1)?.parse().ok())
    };
    let total = value("MemTotal:").ok_or("MemTotal missing")?;
    let available = value("MemAvailable:").ok_or("MemAvailable missing")?;
    let uptime = fs::read_to_string("/proc/uptime")
        .map_err(|error| error.to_string())?
        .split_whitespace()
        .next()
        .and_then(|v| v.parse::<f64>().ok())
        .unwrap_or(0.0) as u64;
    Ok((((total - available) * 100 / total) as u8, uptime))
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn rgb565_is_transposed_and_cropped() {
        let mut frame = vec![0u8; (WIDTH * HEIGHT * 4) as usize];
        let p = ((3 * WIDTH + 2) * 4) as usize;
        frame[p..p + 4].copy_from_slice(&[255, 0, 0, 255]);
        let result = encode(vec![frame], 100).unwrap();
        assert_eq!(result.bounds, [2, 3, 3, 4]);
        assert_eq!(result.frames[0], [0xf8, 0]);
    }
    #[test]
    fn black_frame_uses_full_raster() {
        let result = encode(vec![vec![0; (WIDTH * HEIGHT * 4) as usize]], 100).unwrap();
        assert_eq!(result.bounds, [0; 4]);
        assert_eq!(result.frames[0].len(), (WIDTH * HEIGHT * 2) as usize);
    }
    #[test]
    fn transparent_pixels_are_composited_on_black() {
        let image =
            DynamicImage::ImageRgba8(RgbaImage::from_pixel(1, 1, image::Rgba([255, 0, 0, 0])));
        let frame = fit_frame(image).unwrap();
        assert!(frame.chunks_exact(4).all(|pixel| pixel[..3] == [0, 0, 0]));
    }
}
