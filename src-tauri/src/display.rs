use crate::transport::Transport;
use base64::{engine::general_purpose::STANDARD, Engine};
use image::{
    codecs::gif::GifDecoder, imageops::FilterType, AnimationDecoder, DynamicImage, GenericImage,
    ImageDecoder, ImageFormat, ImageReader, Limits, RgbaImage,
};
use std::{io::Cursor, thread, time::Duration};

pub const WIDTH: u32 = 128;
pub const HEIGHT: u32 = 128;
const MAX_FRAMES: usize = 222;
const MAX_INPUT_BYTES: usize = 100 * 1024 * 1024;

fn decode_limits() -> Limits {
    let mut limits = Limits::default();
    limits.max_image_width = Some(4096);
    limits.max_image_height = Some(4096);
    limits.max_alloc = Some(128 * 1024 * 1024);
    limits
}

#[derive(Debug)]
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
    decode_image(bytes, filename, delay_override)
}

fn decode_image(
    bytes: &[u8],
    filename: &str,
    delay_override: Option<u8>,
) -> Result<Animation, String> {
    if bytes.len() > MAX_INPUT_BYTES {
        return Err("Image exceeds 100 MB".into());
    }
    let format =
        image::guess_format(bytes).map_err(|error| format!("Unsupported image: {error}"))?;
    let mut delay = delay_override.unwrap_or(100);
    let frames = if format == ImageFormat::Gif || filename.to_ascii_lowercase().ends_with(".gif") {
        let mut decoder = GifDecoder::new(Cursor::new(bytes))
            .map_err(|error| format!("Could not decode GIF: {error}"))?;
        decoder
            .set_limits(decode_limits())
            .map_err(|error| format!("Image is too large: {error}"))?;
        let mut result = Vec::new();
        let mut total_delay_ms = 0.0;
        let mut decoded = decoder.into_frames();
        loop {
            if result.len() == MAX_FRAMES {
                if decoded.next().is_some() {
                    return Err(format!("GIF exceeds the K8 limit of {MAX_FRAMES} frames"));
                }
                break;
            }
            let Some(frame) = decoded.next() else {
                break;
            };
            let frame = frame.map_err(|error| format!("Could not decode GIF: {error}"))?;
            let (numerator, denominator) = frame.delay().numer_denom_ms();
            total_delay_ms += numerator as f64 / denominator.max(1) as f64;
            result.push(fit_frame(DynamicImage::ImageRgba8(frame.into_buffer()))?);
        }
        if delay_override.is_none() && !result.is_empty() {
            delay = (total_delay_ms / result.len() as f64)
                .round()
                .clamp(1.0, 255.0) as u8;
        }
        result
    } else {
        let mut reader = ImageReader::with_format(Cursor::new(bytes), format);
        reader.limits(decode_limits());
        vec![fit_frame(reader.decode().map_err(|error| {
            format!("Could not decode image: {error}")
        })?)?]
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
    // Always replace the complete raster. A cropped upload can leave pixels
    // from a previous slot image outside the new image's non-black bounds.
    let mut encoded = Vec::with_capacity(frames.len());
    for frame in frames {
        let mut output = Vec::new();
        for x in 0..WIDTH {
            for y in 0..HEIGHT {
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
        bounds: [0, 0, WIDTH as u16, HEIGHT as u16],
    })
}

pub trait DisplayTransport {
    fn send(&self, data: &[u8], checksum_at: Option<usize>) -> Result<(), String>;
    fn announce(&self, data: &[u8]) -> Result<[u8; 64], String>;
}

impl DisplayTransport for Transport {
    fn send(&self, data: &[u8], checksum_at: Option<usize>) -> Result<(), String> {
        Transport::send(self, data, checksum_at)
    }
    fn announce(&self, data: &[u8]) -> Result<[u8; 64], String> {
        self.query(data, Some(7), Duration::from_millis(100), 0xa5)
    }
}

pub fn upload(
    transport: &impl DisplayTransport,
    animation: &Animation,
    layer: u8,
    progress: impl Fn(f64, String),
) -> Result<(), String> {
    if layer > 4 {
        return Err("Display slot must be 1..5".into());
    }
    let count = animation.frames.len();
    if count == 0 || count > MAX_FRAMES {
        return Err("Invalid frame count".into());
    }
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
    let mut reply = transport.announce(&handshake)?;
    for _ in 0..9 {
        if reply[1] != 0 {
            break;
        }
        thread::sleep(Duration::from_millis(100));
        reply = transport.announce(&handshake)?;
    }
    if reply[1] != 1 {
        return Err(format!(
            "Upload rejected (display status 0x{:02x}). Check that the display is on and the keyboard is in wired mode.", reply[1]
        ));
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
    // The vendor uploader waits after the last pixel packet before starting
    // another transfer. The LCD controller still needs to commit this image.
    thread::sleep(Duration::from_millis(500));
    progress(1.0, "Complete".into());
    Ok(())
}

#[cfg(target_os = "linux")]
pub fn capture_screen() -> Result<String, String> {
    use std::{
        path::Path,
        process::{Command, Stdio},
    };
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

#[cfg(target_os = "linux")]
pub fn system_card() -> Result<(u8, u64), String> {
    use std::fs;
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

#[cfg(target_os = "windows")]
pub fn capture_screen() -> Result<String, String> {
    let image = DynamicImage::ImageRgba8(crate::windows_capture::screen(512)?);
    let mut png = Cursor::new(Vec::new());
    image
        .write_to(&mut png, ImageFormat::Png)
        .map_err(|error| error.to_string())?;
    Ok(STANDARD.encode(png.into_inner()))
}

#[cfg(target_os = "windows")]
pub fn system_card() -> Result<(u8, u64), String> {
    crate::windows_capture::system_card()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::cell::RefCell;

    #[derive(Default)]
    struct FakeDisplay {
        packets: RefCell<Vec<Vec<u8>>>,
        last_data: RefCell<Option<std::time::Instant>>,
    }
    impl DisplayTransport for FakeDisplay {
        fn send(&self, data: &[u8], _: Option<usize>) -> Result<(), String> {
            self.packets.borrow_mut().push(data.to_vec());
            *self.last_data.borrow_mut() = Some(std::time::Instant::now());
            Ok(())
        }
        fn announce(&self, data: &[u8]) -> Result<[u8; 64], String> {
            if let Some(last_data) = *self.last_data.borrow() {
                assert!(
                    last_data.elapsed() >= Duration::from_millis(500),
                    "Started another LCD transfer before the flash commit wait"
                );
            }
            self.packets.borrow_mut().push(data.to_vec());
            let mut reply = [0; 64];
            reply[0] = 0xa5;
            reply[1] = 1;
            Ok(reply)
        }
    }

    #[test]
    fn animated_upload_keeps_distinct_frames_and_one_announcement() {
        let red = [255, 0, 0, 255].repeat((WIDTH * HEIGHT) as usize);
        let blue = [0, 0, 255, 255].repeat((WIDTH * HEIGHT) as usize);
        let animation = encode(vec![red, blue], 75).unwrap();
        let display = FakeDisplay::default();
        let progress = RefCell::new(Vec::new());
        upload(&display, &animation, 4, |value, _| {
            progress.borrow_mut().push(value)
        })
        .unwrap();
        let packets = display.packets.borrow();
        let announcements: Vec<_> = packets.iter().filter(|packet| packet[0] == 0xa5).collect();
        assert_eq!(announcements.len(), 1);
        assert_eq!(&announcements[0][..6], &[0xa5, 0, 2, 75, 0, 128]);
        let pixels: Vec<_> = packets.iter().filter(|packet| packet[0] == 0x25).collect();
        assert_eq!(pixels.len(), 2 * 32768usize.div_ceil(56));
        for (index, pixel) in pixels.iter().enumerate() {
            let frame = index / 32768usize.div_ceil(56);
            assert_eq!(pixel[1], frame as u8);
            assert_eq!(pixel[2], 2);
            assert_eq!(pixel[3], 75);
            assert_eq!(
                u16::from_le_bytes([pixel[4], pixel[5]]) as usize,
                index % 32768usize.div_ceil(56)
            );
            let color = if frame == 0 { [0xf8, 0] } else { [0, 0x1f] };
            assert!(pixel[8..8 + pixel[6] as usize]
                .chunks_exact(2)
                .all(|value| value == color));
        }
        let progress = progress.borrow();
        assert!(progress.windows(2).all(|values| values[0] <= values[1]));
        assert_eq!(progress.last(), Some(&1.0));
    }

    #[test]
    fn still_upload_only_writes_selected_slot() {
        let frame = [255, 0, 0, 255].repeat((WIDTH * HEIGHT) as usize);
        let photo = encode(vec![frame], 100).unwrap();
        let display = FakeDisplay::default();
        for slot in 0..5 {
            let start = display.packets.borrow().len();
            upload(&display, &photo, slot, |_, _| {}).unwrap();
            assert!(display.packets.borrow()[start..]
                .iter()
                .all(|packet| packet[1] == slot && packet[2] == 1 && packet[3] == 0));
        }
    }

    fn gif(frames: usize) -> Vec<u8> {
        let mut data = Vec::new();
        let mut encoder = image::codecs::gif::GifEncoder::new(&mut data);
        for _ in 0..frames {
            encoder
                .encode_frame(image::Frame::new(RgbaImage::from_pixel(
                    1,
                    1,
                    image::Rgba([255, 0, 0, 255]),
                )))
                .unwrap();
        }
        drop(encoder);
        data
    }

    #[test]
    fn gif_limit_preserves_animation() {
        let bytes = gif(MAX_FRAMES + 1);
        assert!(decode(&bytes, "test.gif", None)
            .unwrap_err()
            .contains("limit"));
        assert_eq!(decode(&gif(2), "test.gif", Some(75)).unwrap().delay, 75);
    }

    #[test]
    fn gif_decode_keeps_motion_and_average_timing() {
        let mut bytes = Vec::new();
        let mut encoder = image::codecs::gif::GifEncoder::new(&mut bytes);
        for (color, delay) in [([255, 0, 0, 255], 40), ([0, 0, 255, 255], 120)] {
            encoder
                .encode_frame(image::Frame::from_parts(
                    RgbaImage::from_pixel(128, 128, image::Rgba(color)),
                    0,
                    0,
                    image::Delay::from_numer_denom_ms(delay, 1),
                ))
                .unwrap();
        }
        drop(encoder);
        let animation = decode(&bytes, "motion.gif", None).unwrap();
        assert_eq!(animation.frames.len(), 2);
        assert_eq!(animation.delay, 80);
        assert!(animation.frames[0]
            .chunks_exact(2)
            .all(|pixel| pixel == [0xf8, 0]));
        assert!(animation.frames[1]
            .chunks_exact(2)
            .all(|pixel| pixel == [0, 0x1f]));
        assert_eq!(decode(&bytes, "motion.gif", Some(75)).unwrap().delay, 75);
    }

    #[test]
    fn rejects_oversized_gif_before_allocating_frames() {
        let mut bytes = gif(1);
        bytes[6..8].copy_from_slice(&5000u16.to_le_bytes());
        assert!(decode(&bytes, "large.gif", None).is_err());
    }
    #[test]
    fn rgb565_uses_full_raster_in_column_order() {
        let mut frame = vec![0u8; (WIDTH * HEIGHT * 4) as usize];
        let p = ((3 * WIDTH + 2) * 4) as usize;
        frame[p..p + 4].copy_from_slice(&[255, 0, 0, 255]);
        let result = encode(vec![frame], 100).unwrap();
        assert_eq!(result.bounds, [0, 0, 128, 128]);
        assert_eq!(result.frames[0].len(), 32768);
        let pixel = ((2 * HEIGHT + 3) * 2) as usize;
        assert_eq!(&result.frames[0][pixel..pixel + 2], &[0xf8, 0]);
        assert!(result.frames[0][..pixel].iter().all(|value| *value == 0));
        assert!(result.frames[0][pixel + 2..]
            .iter()
            .all(|value| *value == 0));
    }
    #[test]
    fn black_frame_uses_full_raster() {
        let result = encode(vec![vec![0; (WIDTH * HEIGHT * 4) as usize]], 100).unwrap();
        assert_eq!(result.bounds, [0, 0, 128, 128]);
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
