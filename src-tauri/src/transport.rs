use std::{
    thread,
    time::{Duration, Instant},
};

#[cfg(target_os = "linux")]
mod linux;
#[cfg(target_os = "windows")]
mod windows;
#[cfg(target_os = "linux")]
use linux as platform;
#[cfg(target_os = "windows")]
use windows as platform;
#[cfg(not(any(target_os = "linux", target_os = "windows")))]
compile_error!("K8 Studio currently supports Linux and Windows.");

const REPORT_SIZE: usize = 64;

#[derive(Clone, serde::Serialize)]
pub struct DeviceInfo {
    pub path: String,
    pub name: String,
}

pub fn discover() -> Vec<DeviceInfo> {
    platform::discover()
}

pub struct Transport {
    inner: platform::Transport,
}
impl Transport {
    pub fn open() -> Result<Self, String> {
        Ok(Self {
            inner: platform::Transport::open()?,
        })
    }
    pub fn send(&self, data: &[u8], checksum_at: Option<usize>) -> Result<(), String> {
        let mut report = feature_report(data, checksum_at)?;
        self.inner.write_report(&mut report)
    }
    pub fn receive(&self) -> Result<[u8; REPORT_SIZE], String> {
        self.inner.read_report()
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
}

fn feature_report(
    data: &[u8],
    checksum_at: Option<usize>,
) -> Result<[u8; REPORT_SIZE + 1], String> {
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
    Ok(report)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn feature_report_preserves_report_id_and_checksum_position() {
        let report = feature_report(&[0x0e, 0x12, 0x34, 0x56, 0xff], Some(7)).unwrap();
        assert_eq!(report[0], 0);
        assert_eq!(&report[1..6], &[0x0e, 0x12, 0x34, 0x56, 0xff]);
        assert_eq!(report[8], 0x56);
        assert_eq!(
            report[1..9]
                .iter()
                .fold(0u8, |sum, byte| sum.wrapping_add(*byte)),
            0xff
        );
        assert!(report[9..].iter().all(|byte| *byte == 0));
    }

    #[test]
    fn full_lcd_packets_are_not_shifted_or_checksummed() {
        let data = [0x25; 64];
        let report = feature_report(&data, None).unwrap();
        assert_eq!(report[0], 0);
        assert_eq!(report[1..], data);
        assert!(feature_report(&[0; 65], None).is_err());
        assert!(feature_report(&[], Some(64)).is_err());
    }
}
