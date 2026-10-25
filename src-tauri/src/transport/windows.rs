use super::{DeviceInfo, REPORT_SIZE};
use fs2::FileExt;
use hidapi::{HidApi, HidDevice};
use std::fs::{File, OpenOptions};

fn is_control(device: &hidapi::DeviceInfo) -> bool {
    device.vendor_id() == 0x3151
        && device.product_id() == 0x4015
        && device.usage_page() == 0xffff
        && device.usage() == 0x0002
}

pub fn discover() -> Vec<DeviceInfo> {
    let Ok(api) = HidApi::new() else {
        return Vec::new();
    };
    api.device_list()
        .filter(|device| is_control(device))
        .map(|device| DeviceInfo {
            path: device.path().to_string_lossy().into_owned(),
            name: device
                .product_string()
                .unwrap_or("Darmoshark K8")
                .to_owned(),
        })
        .collect()
}

pub struct Transport {
    device: HidDevice,
    // Keep the lock until after the HID handle is dropped. This serializes
    // separate Studio processes, matching Linux's advisory device lock.
    _lock: File,
}

impl Transport {
    pub fn open() -> Result<Self, String> {
        let lock = OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .truncate(false)
            .open(std::env::temp_dir().join("darmoshark-k8-studio-hid.lock"))
            .map_err(|error| format!("Could not open keyboard lock: {error}"))?;
        lock.try_lock_exclusive()
            .map_err(|error| format!("Keyboard is busy in another Studio window: {error}"))?;
        let api =
            HidApi::new().map_err(|error| format!("Could not enumerate USB devices: {error}"))?;
        let info = api
            .device_list()
            .find(|device| is_control(device))
            .ok_or("Darmoshark K8 not found. Switch to wired mode and check the USB cable.")?;
        let device = info.open_device(&api).map_err(|error| {
            format!("Could not open the K8. Close the manufacturer's driver and retry: {error}")
        })?;
        Ok(Self {
            device,
            _lock: lock,
        })
    }

    pub fn write_report(&self, report: &mut [u8; REPORT_SIZE + 1]) -> Result<(), String> {
        self.device
            .send_feature_report(report)
            .map_err(|error| format!("Could not send keyboard settings: {error}"))
    }

    pub fn read_report(&self) -> Result<[u8; REPORT_SIZE], String> {
        let mut report = [0u8; REPORT_SIZE + 1];
        let received = self
            .device
            .get_feature_report(&mut report)
            .map_err(|error| format!("Could not read keyboard settings: {error}"))?;
        if received != report.len() || report[0] != 0 {
            return Err(format!(
                "Invalid keyboard response: received {received} bytes, report ID {}",
                report[0]
            ));
        }
        Ok(report[1..].try_into().unwrap())
    }
}
