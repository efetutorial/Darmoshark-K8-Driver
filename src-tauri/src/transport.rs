use fs2::FileExt;
use std::{
    fs::{self, File, OpenOptions},
    io,
    os::fd::AsRawFd,
    path::PathBuf,
    thread,
    time::{Duration, Instant},
};

const REPORT_SIZE: usize = 64;
const HID_ID: &str = "0003:00003151:00004015";
const CONTROL_USAGE: &[u8] = b"\x06\xff\xff\x09\x02";
const FEATURE_REPORT: &[u8] = b"\x95\x40\x75\x08\xb1\x02";

#[derive(Clone, serde::Serialize)]
pub struct DeviceInfo {
    pub path: String,
    pub name: String,
}

fn uevent_value(contents: &str, key: &str) -> Option<String> {
    contents.lines().find_map(|line| {
        let (name, value) = line.split_once('=')?;
        (name == key).then(|| value.to_owned())
    })
}

pub fn discover() -> Vec<DeviceInfo> {
    let mut devices = Vec::new();
    let Ok(entries) = fs::read_dir("/sys/class/hidraw") else {
        return devices;
    };
    let mut entries: Vec<_> = entries.flatten().collect();
    entries.sort_by_key(|entry| entry.file_name());
    for entry in entries {
        let device = entry.path().join("device");
        let Ok(uevent) = fs::read_to_string(device.join("uevent")) else {
            continue;
        };
        if uevent_value(&uevent, "HID_ID").as_deref() != Some(HID_ID) {
            continue;
        }
        let Ok(descriptor) = fs::read(device.join("report_descriptor")) else {
            continue;
        };
        if !descriptor
            .windows(CONTROL_USAGE.len())
            .any(|part| part == CONTROL_USAGE)
            || !descriptor
                .windows(FEATURE_REPORT.len())
                .any(|part| part == FEATURE_REPORT)
        {
            continue;
        }
        let node = PathBuf::from("/dev").join(entry.file_name());
        devices.push(DeviceInfo {
            path: node.to_string_lossy().into_owned(),
            name: uevent_value(&uevent, "HID_NAME").unwrap_or_else(|| "Darmoshark K8".into()),
        });
    }
    devices
}

fn ioc(direction: u64, kind: u64, number: u64, size: usize) -> libc::c_ulong {
    const NR_BITS: u64 = 8;
    const TYPE_BITS: u64 = 8;
    const SIZE_BITS: u64 = 14;
    let type_shift = NR_BITS;
    let size_shift = type_shift + TYPE_BITS;
    let dir_shift = size_shift + SIZE_BITS;
    ((direction << dir_shift) | (kind << type_shift) | number | ((size as u64) << size_shift)) as _
}

fn hid_iocsfeature(size: usize) -> libc::c_ulong {
    ioc(3, b'H' as u64, 0x06, size)
}
fn hid_iocgfeature(size: usize) -> libc::c_ulong {
    ioc(3, b'H' as u64, 0x07, size)
}

pub struct Transport {
    file: File,
}

impl Transport {
    pub fn open() -> Result<Self, String> {
        let device = discover().into_iter().next().ok_or_else(|| {
            "Darmoshark K8 not found. Switch to wired mode and check the USB cable.".to_string()
        })?;
        let file = OpenOptions::new().read(true).write(true).open(&device.path)
            .map_err(|error| if error.kind() == io::ErrorKind::PermissionDenied {
                format!("{} cannot be accessed. Reinstall the udev rule, then reconnect the keyboard.", device.path)
            } else { format!("{} could not be opened: {error}", device.path) })?;
        file.lock_exclusive()
            .map_err(|error| format!("Could not lock keyboard: {error}"))?;
        Ok(Self { file })
    }

    pub fn send(&self, data: &[u8], checksum_at: Option<usize>) -> Result<(), String> {
        if data.len() > REPORT_SIZE {
            return Err("Feature report exceeds 64 bytes".into());
        }
        let mut report = [0u8; REPORT_SIZE + 1];
        report[1..1 + data.len()].copy_from_slice(data);
        if let Some(index) = checksum_at {
            if index >= REPORT_SIZE {
                return Err("Checksum index is outside report".into());
            }
            let sum = report[1..1 + index]
                .iter()
                .fold(0u8, |value, byte| value.wrapping_add(*byte));
            report[1 + index] = 0xffu8.wrapping_sub(sum);
        }
        let result = unsafe {
            libc::ioctl(
                self.file.as_raw_fd(),
                hid_iocsfeature(report.len()),
                report.as_mut_ptr(),
            )
        };
        if result < 0 {
            return Err(format!(
                "Could not send keyboard settings: {}",
                io::Error::last_os_error()
            ));
        }
        Ok(())
    }

    pub fn receive(&self) -> Result<[u8; REPORT_SIZE], String> {
        let mut report = [0u8; REPORT_SIZE + 1];
        let result = unsafe {
            libc::ioctl(
                self.file.as_raw_fd(),
                hid_iocgfeature(report.len()),
                report.as_mut_ptr(),
            )
        };
        if result < 0 {
            return Err(format!(
                "Could not read keyboard settings: {}",
                io::Error::last_os_error()
            ));
        }
        let mut output = [0u8; REPORT_SIZE];
        if report[0] == 0 {
            output.copy_from_slice(&report[1..]);
        } else {
            output.copy_from_slice(&report[..REPORT_SIZE]);
        }
        Ok(output)
    }

    pub fn query(
        &self,
        data: &[u8],
        checksum_at: Option<usize>,
        delay: Duration,
        expected: u8,
    ) -> Result<[u8; REPORT_SIZE], String> {
        self.send(data, checksum_at)?;
        thread::sleep(delay);
        let deadline = Instant::now() + Duration::from_millis(750);
        loop {
            let reply = self.receive()?;
            if reply[0] == expected {
                return Ok(reply);
            }
            if Instant::now() >= deadline {
                return Err(format!(
                    "Unexpected keyboard response: 0x{:02x}; expected 0x{expected:02x}",
                    reply[0]
                ));
            }
            thread::sleep(Duration::from_millis(20));
        }
    }

    pub fn query_any(
        &self,
        data: &[u8],
        checksum_at: Option<usize>,
        delay: Duration,
    ) -> Result<[u8; REPORT_SIZE], String> {
        self.send(data, checksum_at)?;
        thread::sleep(delay);
        self.receive()
    }
}
