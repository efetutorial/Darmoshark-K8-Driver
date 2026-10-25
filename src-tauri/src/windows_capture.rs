//! Windows capture owns every OS resource on the thread that created it.
use image::RgbaImage;
use std::{collections::VecDeque, mem::size_of};
use windows::Win32::{
    Graphics::Gdi::*,
    Media::Audio::*,
    System::{Com::*, SystemInformation::*},
};

#[derive(Default)]
struct ScreenDc {
    desktop: HDC,
    memory: HDC,
    bitmap: HBITMAP,
    previous: HGDIOBJ,
}
impl Drop for ScreenDc {
    fn drop(&mut self) {
        unsafe {
            if !self.previous.0.is_null() {
                SelectObject(self.memory, self.previous);
            }
            if !self.bitmap.0.is_null() {
                let _ = DeleteObject(self.bitmap.into());
            }
            if !self.memory.0.is_null() {
                let _ = DeleteDC(self.memory);
            }
            if !self.desktop.0.is_null() {
                ReleaseDC(None, self.desktop);
            }
        }
    }
}

/// Capture the primary desktop into a small top-down RGBA image.
pub fn screen(size: u32) -> Result<RgbaImage, String> {
    if !(1..=512).contains(&size) {
        return Err("Invalid screenshot size".into());
    }
    unsafe {
        let mut dc = ScreenDc {
            desktop: GetDC(None),
            ..Default::default()
        };
        if dc.desktop.0.is_null() {
            return Err("Could not access the Windows desktop".into());
        }
        let width = GetDeviceCaps(Some(dc.desktop), HORZRES);
        let height = GetDeviceCaps(Some(dc.desktop), VERTRES);
        if width <= 0 || height <= 0 {
            return Err("Desktop has no visible area".into());
        }
        dc.memory = CreateCompatibleDC(Some(dc.desktop));
        if dc.memory.0.is_null() {
            return Err("Could not create screenshot context".into());
        }
        dc.bitmap = CreateCompatibleBitmap(dc.desktop, size as i32, size as i32);
        if dc.bitmap.0.is_null() {
            return Err("Could not create screenshot bitmap".into());
        }
        dc.previous = SelectObject(dc.memory, dc.bitmap.into());
        if dc.previous.0.is_null() || dc.previous.0 as isize == -1 {
            dc.previous = HGDIOBJ::default();
            return Err("Could not select screenshot bitmap".into());
        }
        SetStretchBltMode(dc.memory, COLORONCOLOR);
        StretchBlt(
            dc.memory,
            0,
            0,
            size as i32,
            size as i32,
            Some(dc.desktop),
            0,
            0,
            width,
            height,
            SRCCOPY | CAPTUREBLT,
        )
        .ok()
        .map_err(|error| format!("Could not capture desktop: {error}"))?;
        // GetDIBits requires the bitmap to be deselected from its DC.
        SelectObject(dc.memory, dc.previous);
        dc.previous = HGDIOBJ::default();
        let mut info = BITMAPINFO::default();
        info.bmiHeader = BITMAPINFOHEADER {
            biSize: size_of::<BITMAPINFOHEADER>() as u32,
            biWidth: size as i32,
            biHeight: -(size as i32),
            biPlanes: 1,
            biBitCount: 32,
            biCompression: BI_RGB.0,
            ..Default::default()
        };
        let mut pixels = vec![0u8; (size * size * 4) as usize];
        if GetDIBits(
            dc.memory,
            dc.bitmap,
            0,
            size,
            Some(pixels.as_mut_ptr().cast()),
            &mut info,
            DIB_RGB_COLORS,
        ) != size as i32
        {
            return Err("Could not read screenshot pixels".into());
        }
        for pixel in pixels.chunks_exact_mut(4) {
            pixel.swap(0, 2);
            pixel[3] = 255;
        }
        RgbaImage::from_raw(size, size, pixels).ok_or_else(|| "Invalid screenshot pixels".into())
    }
}

pub fn system_card() -> Result<(u8, u64), String> {
    let mut memory = MEMORYSTATUSEX {
        dwLength: size_of::<MEMORYSTATUSEX>() as u32,
        ..Default::default()
    };
    unsafe {
        GlobalMemoryStatusEx(&mut memory)
            .map_err(|error| format!("Could not read memory usage: {error}"))?;
        Ok((memory.dwMemoryLoad.min(100) as u8, GetTickCount64() / 1000))
    }
}

struct ComApartment;
impl Drop for ComApartment {
    fn drop(&mut self) {
        unsafe {
            CoUninitialize();
        }
    }
}

pub struct AudioCapture {
    capture: IAudioCaptureClient,
    client: IAudioClient,
    // COM interfaces must be released before CoUninitialize on this thread.
    _apartment: ComApartment,
    samples: VecDeque<u8>,
    last_packet: std::time::Instant,
}

impl AudioCapture {
    pub fn open() -> Result<Self, String> {
        unsafe {
            CoInitializeEx(None, COINIT_MULTITHREADED)
                .ok()
                .map_err(|error| error.to_string())?;
            let apartment = ComApartment;
            let enumerator: IMMDeviceEnumerator =
                CoCreateInstance(&MMDeviceEnumerator, None, CLSCTX_ALL)
                    .map_err(|error| error.to_string())?;
            let endpoint = enumerator
                .GetDefaultAudioEndpoint(eRender, eConsole)
                .map_err(|error| format!("No default playback device: {error}"))?;
            let client: IAudioClient = endpoint
                .Activate(CLSCTX_ALL, None)
                .map_err(|error| error.to_string())?;
            // Ask the shared Windows audio engine for stereo float PCM. It handles
            // resampling and channel conversion, including surround endpoints.
            let format = WAVEFORMATEX {
                wFormatTag: 3,
                nChannels: 2,
                nSamplesPerSec: 48000,
                nAvgBytesPerSec: 48000 * 8,
                nBlockAlign: 8,
                wBitsPerSample: 32,
                cbSize: 0,
            };
            client
                .Initialize(
                    AUDCLNT_SHAREMODE_SHARED,
                    AUDCLNT_STREAMFLAGS_LOOPBACK
                        | AUDCLNT_STREAMFLAGS_AUTOCONVERTPCM
                        | AUDCLNT_STREAMFLAGS_SRC_DEFAULT_QUALITY,
                    1_000_000,
                    0,
                    &format,
                    None,
                )
                .map_err(|error| format!("Could not initialize playback capture: {error}"))?;
            let capture: IAudioCaptureClient =
                client.GetService().map_err(|error| error.to_string())?;
            client.Start().map_err(|error| error.to_string())?;
            // Locals declared after the apartment are dropped before it as well.
            drop(endpoint);
            drop(enumerator);
            Ok(Self {
                capture,
                client,
                _apartment: apartment,
                samples: VecDeque::new(),
                last_packet: std::time::Instant::now(),
            })
        }
    }

    /// Drain available packets without waiting for sound, so Stop also works in silence.
    pub fn read(&mut self) -> Result<Vec<u8>, String> {
        const WINDOW: usize = 2048 * 8;
        unsafe {
            for _ in 0..64 {
                if self
                    .capture
                    .GetNextPacketSize()
                    .map_err(|error| error.to_string())?
                    == 0
                {
                    break;
                }
                let mut data = std::ptr::null_mut();
                let mut frames = 0;
                let mut flags = 0;
                self.capture
                    .GetBuffer(&mut data, &mut frames, &mut flags, None, None)
                    .map_err(|error| error.to_string())?;
                let count = frames as usize * 8;
                let silent = flags & AUDCLNT_BUFFERFLAGS_SILENT.0 as u32 != 0;
                if !silent && data.is_null() && count != 0 {
                    let _ = self.capture.ReleaseBuffer(frames);
                    return Err("Playback capture returned an empty buffer".into());
                }
                if silent {
                    self.samples
                        .extend(std::iter::repeat_n(0, count.min(WINDOW)));
                } else if count > 0 {
                    let bytes = std::slice::from_raw_parts(data, count);
                    self.samples.extend(&bytes[count.saturating_sub(WINDOW)..]);
                }
                self.capture
                    .ReleaseBuffer(frames)
                    .map_err(|error| error.to_string())?;
                while self.samples.len() > WINDOW {
                    self.samples.pop_front();
                }
                self.last_packet = std::time::Instant::now();
            }
        }
        if self.last_packet.elapsed() > std::time::Duration::from_millis(100) {
            self.samples.clear();
        }
        let mut result = vec![0; WINDOW - self.samples.len()];
        result.extend(self.samples.iter());
        Ok(result)
    }
}

impl Drop for AudioCapture {
    fn drop(&mut self) {
        unsafe {
            let _ = self.client.Stop();
        }
    }
}
