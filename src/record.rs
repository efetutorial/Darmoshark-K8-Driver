//! Layout-independent macro recording.
//!
//! On Linux the K8's own keyboard interfaces are read through hidraw, which
//! yields HID usages directly, exactly what the macro engine stores, whatever
//! keyboard layout the desktop uses. Reading hidraw does not take keys away
//! from the desktop. Other platforms fall back to window key events.

use std::time::Instant;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct KeyEvent {
    pub usage: u8,
    pub down: bool,
    pub at: Instant,
}

/// Tracks pressed usages per report source and turns reports into events.
#[derive(Default)]
pub struct ReportState {
    boot: Vec<u8>,
    nkro: Vec<u8>,
}

impl ReportState {
    fn pressed(&self) -> Vec<u8> {
        let mut all = self.boot.clone();
        for usage in &self.nkro {
            if !all.contains(usage) {
                all.push(*usage);
            }
        }
        all
    }

    fn diff(before: &[u8], after: &[u8], at: Instant) -> Vec<KeyEvent> {
        let mut events: Vec<KeyEvent> = before
            .iter()
            .filter(|usage| !after.contains(usage))
            .map(|&usage| KeyEvent { usage, down: false, at })
            .collect();
        events.extend(
            after
                .iter()
                .filter(|usage| !before.contains(usage))
                .map(|&usage| KeyEvent { usage, down: true, at }),
        );
        events
    }

    /// 8-byte boot keyboard report: modifier bits, reserved, six usages.
    pub fn boot_report(&mut self, report: &[u8], at: Instant) -> Vec<KeyEvent> {
        if report.len() < 8 {
            return Vec::new();
        }
        let before = self.pressed();
        let mut keys: Vec<u8> = (0..8)
            .filter(|bit| report[0] & (1 << bit) != 0)
            .map(|bit| 224 + bit)
            .collect();
        // Usages 1..3 are error/rollover codes, not keys.
        keys.extend(report[2..8].iter().copied().filter(|usage| *usage >= 4));
        self.boot = keys;
        Self::diff(&before, &self.pressed(), at)
    }

    /// K8 NKRO report: report ID 1 followed by a bitmap of usages 0..119.
    pub fn nkro_report(&mut self, report: &[u8], at: Instant) -> Vec<KeyEvent> {
        if report.first() != Some(&1) || report.len() < 16 {
            return Vec::new();
        }
        let before = self.pressed();
        self.nkro = (4u8..120)
            .filter(|usage| report[1 + *usage as usize / 8] & (1 << (usage % 8)) != 0)
            .collect();
        Self::diff(&before, &self.pressed(), at)
    }
}

#[cfg(target_os = "linux")]
mod platform {
    use super::{KeyEvent, ReportState};
    use std::{
        fs::{self, File},
        io::Read,
        os::fd::AsRawFd,
        sync::{
            atomic::{AtomicBool, Ordering},
            mpsc, Arc, Mutex,
        },
        thread,
        time::Instant,
    };

    const HID_ID: &str = "0003:00003151:00004015";
    const BOOT_KEYBOARD: &[u8] = b"\x05\x01\x09\x06\xa1\x01\x05\x07";
    const NKRO_KEYBOARD: &[u8] = b"\x05\x01\x09\x06\xa1\x01\x85\x01\x05\x07";

    #[derive(Clone, Copy)]
    enum Kind {
        Boot,
        Nkro,
    }

    fn interfaces() -> Vec<(std::path::PathBuf, Kind)> {
        let mut found = Vec::new();
        let Ok(entries) = fs::read_dir("/sys/class/hidraw") else {
            return found;
        };
        for entry in entries.flatten() {
            let device = entry.path().join("device");
            let Ok(uevent) = fs::read_to_string(device.join("uevent")) else {
                continue;
            };
            if !uevent.lines().any(|line| line == format!("HID_ID={HID_ID}")) {
                continue;
            }
            let Ok(descriptor) = fs::read(device.join("report_descriptor")) else {
                continue;
            };
            let has = |pattern: &[u8]| descriptor.windows(pattern.len()).any(|w| w == pattern);
            let kind = if descriptor.starts_with(BOOT_KEYBOARD) {
                Kind::Boot
            } else if has(NKRO_KEYBOARD) {
                Kind::Nkro
            } else {
                continue;
            };
            found.push((std::path::PathBuf::from("/dev").join(entry.file_name()), kind));
        }
        found
    }

    pub struct Recorder {
        stop: Arc<AtomicBool>,
        events: mpsc::Receiver<KeyEvent>,
    }

    impl Drop for Recorder {
        fn drop(&mut self) {
            self.stop.store(true, Ordering::Relaxed);
        }
    }

    impl Recorder {
        pub fn start() -> Result<Self, String> {
            let interfaces = interfaces();
            if interfaces.is_empty() {
                return Err("The K8 keyboard interface was not found".into());
            }
            let stop = Arc::new(AtomicBool::new(false));
            let state = Arc::new(Mutex::new(ReportState::default()));
            let (sender, events) = mpsc::channel();
            let mut opened = 0;
            for (path, kind) in interfaces {
                let Ok(mut file) = File::open(&path) else {
                    continue;
                };
                opened += 1;
                let (stop, state, sender) = (stop.clone(), state.clone(), sender.clone());
                thread::spawn(move || {
                    let mut buffer = [0u8; 64];
                    while !stop.load(Ordering::Relaxed) {
                        let mut poll = libc::pollfd {
                            fd: file.as_raw_fd(),
                            events: libc::POLLIN,
                            revents: 0,
                        };
                        // Wake regularly so the thread notices `stop`.
                        let ready = unsafe { libc::poll(&mut poll, 1, 100) };
                        if ready < 0 {
                            break;
                        }
                        if ready == 0 {
                            continue;
                        }
                        let Ok(count) = file.read(&mut buffer) else {
                            break;
                        };
                        let at = Instant::now();
                        let Ok(mut state) = state.lock() else {
                            break;
                        };
                        let events = match kind {
                            Kind::Boot => state.boot_report(&buffer[..count], at),
                            Kind::Nkro => state.nkro_report(&buffer[..count], at),
                        };
                        for event in events {
                            if sender.send(event).is_err() {
                                return;
                            }
                        }
                    }
                });
            }
            if opened == 0 {
                return Err("The K8 keyboard interface cannot be read. Reinstall the udev rule.".into());
            }
            Ok(Self { stop, events })
        }

        pub fn drain(&self) -> Vec<KeyEvent> {
            self.events.try_iter().collect()
        }
    }
}

#[cfg(not(target_os = "linux"))]
mod platform {
    use super::KeyEvent;

    pub struct Recorder;

    impl Recorder {
        pub fn start() -> Result<Self, String> {
            Err("Direct keyboard capture is only available on Linux".into())
        }

        pub fn drain(&self) -> Vec<KeyEvent> {
            Vec::new()
        }
    }
}

pub use platform::Recorder;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn boot_reports_produce_modifier_and_key_events() {
        let mut state = ReportState::default();
        let at = Instant::now();
        // Left shift + A
        let events = state.boot_report(&[0x02, 0, 4, 0, 0, 0, 0, 0], at);
        assert_eq!(events.len(), 2);
        assert!(events.iter().any(|e| e.usage == 225 && e.down));
        assert!(events.iter().any(|e| e.usage == 4 && e.down));
        // Release A, keep shift.
        let events = state.boot_report(&[0x02, 0, 0, 0, 0, 0, 0, 0], at);
        assert_eq!(events, vec![KeyEvent { usage: 4, down: false, at }]);
        // Rollover error codes are ignored.
        assert!(state.boot_report(&[0x02, 0, 1, 1, 1, 1, 1, 1], at).is_empty());
    }

    #[test]
    fn nkro_bitmap_maps_bits_to_usages() {
        let mut state = ReportState::default();
        let at = Instant::now();
        let mut report = [0u8; 16];
        report[0] = 1;
        report[1 + 41 / 8] |= 1 << (41 % 8); // Esc
        report[1 + 30 / 8] |= 1 << (30 % 8); // 1
        let mut usages: Vec<u8> = state.nkro_report(&report, at).iter().map(|e| e.usage).collect();
        usages.sort();
        assert_eq!(usages, vec![30, 41]);
        // A key held on both interfaces is reported once.
        assert!(state.boot_report(&[0, 0, 30, 0, 0, 0, 0, 0], at).is_empty());
    }
}
