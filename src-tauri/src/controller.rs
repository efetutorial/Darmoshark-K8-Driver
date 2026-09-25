use crate::{keys, transport::Transport};
use chrono::{Datelike, Local, Timelike};
use serde::{Deserialize, Serialize};
use std::{thread, time::Duration};

#[derive(Clone, Serialize, Deserialize)]
pub struct LightingState {
    pub effect: String,
    pub brightness: u8,
    pub speed: u8,
    pub color: String,
    pub rainbow: bool,
    pub direction: String,
    pub slot: u8,
}

#[derive(Clone, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct KeyboardOptions {
    pub win_lock: bool,
    pub mac_mode: bool,
    pub swap_wasd_arrows: bool,
    pub led_off: bool,
    pub side_led_off: bool,
    pub gaming_mode: bool,
    pub keyboard_lock: bool,
    pub fn_remap: bool,
    pub power_save: bool,
}

#[derive(Serialize)]
pub struct Battery {
    pub percent: u8,
    pub state: String,
}
#[derive(Serialize)]
pub struct SleepTimes {
    pub bluetooth: u16,
    pub wireless: u16,
    pub bluetooth_deep: u16,
    pub wireless_deep: u16,
}
#[derive(Serialize)]
pub struct Status {
    pub firmware: u16,
    pub battery: Battery,
    pub lighting: LightingState,
    pub report_rate: u16,
    pub profile: u8,
    pub options: KeyboardOptions,
    pub sleep: SleepTimes,
    pub debounce: u8,
}

fn effect_id(name: &str) -> Option<u8> {
    Some(match name {
        "off" => 0,
        "solid" => 1,
        "breathing" => 2,
        "neon" => 3,
        "wave" => 4,
        "ripple" => 5,
        "raindrop" => 6,
        "snake" => 7,
        "reactive" => 8,
        "convergence" => 9,
        "sine" => 10,
        "kaleidoscope" => 11,
        "line_wave" => 12,
        "user_picture" => 13,
        "laser" => 14,
        "circle_wave" => 15,
        "dazzling" => 16,
        "rain_down" => 17,
        "meteor" => 18,
        "reactive_off" => 19,
        "music_3" => 20,
        "screen_color" => 21,
        "music_2" => 22,
        "train" => 23,
        "fireworks" => 24,
        "user_color" => 66,
        _ => return None,
    })
}
fn effect_name(id: u8) -> String {
    [
        "off",
        "solid",
        "breathing",
        "neon",
        "wave",
        "ripple",
        "raindrop",
        "snake",
        "reactive",
        "convergence",
        "sine",
        "kaleidoscope",
        "line_wave",
        "user_picture",
        "laser",
        "circle_wave",
        "dazzling",
        "rain_down",
        "meteor",
        "reactive_off",
        "music_3",
        "screen_color",
        "music_2",
        "train",
        "fireworks",
    ]
    .get(id as usize)
    .map(|name| (*name).to_owned())
    .unwrap_or_else(|| format!("unknown_{id}"))
}
fn direction_value(name: &str) -> u8 {
    match name {
        "left" | "in" | "clockwise" | "separate" => 1,
        "down" | "intersect" => 2,
        "up" => 3,
        _ => 0,
    }
}
fn color_bytes(color: &str) -> Result<[u8; 3], String> {
    let value = u32::from_str_radix(color.trim_start_matches('#'), 16)
        .map_err(|_| "Color must be #RRGGBB".to_string())?;
    if value > 0xffffff || color.trim_start_matches('#').len() != 6 {
        return Err("Color must be #RRGGBB".into());
    }
    let value = if value == 0xffffff { 0xfafffa } else { value };
    Ok([(value >> 16) as u8, (value >> 8) as u8, value as u8])
}

fn lighting_report(value: &LightingState) -> Result<[u8; 64], String> {
    if value.brightness > 4 || value.speed > 4 {
        return Err("Brightness and speed must be 0..4".into());
    }
    let id = effect_id(&value.effect).ok_or_else(|| format!("Unknown effect: {}", value.effect))?;
    let mut command = [0u8; 64];
    command[0] = 7;
    command[1] = id;
    command[2] = 4 - value.speed;
    command[3] = value.brightness;
    // K8 firmware calls 0x08 "dazzle" (automatic rainbow) and 0x07
    // "normal" (the RGB bytes below). Reversing these makes chosen colors
    // appear to have no effect.
    let color_mode = if value.rainbow { 8 } else { 7 };
    let direction = direction_value(&value.direction);
    command[4] = if matches!(
        value.effect.as_str(),
        "wave" | "snake" | "kaleidoscope" | "line_wave" | "circle_wave" | "fireworks"
    ) {
        (direction << 4) | color_mode
    } else if value.effect == "user_picture" {
        if !(1..=4).contains(&value.slot) {
            return Err("Lighting slot must be 1..4".into());
        }
        (value.slot - 1) << 4
    } else if matches!(value.effect.as_str(), "music_2" | "music_3") {
        (direction << 4) | if value.rainbow { 0 } else { 4 }
    } else {
        color_mode
    };
    command[5..8].copy_from_slice(&if value.effect == "user_picture" {
        [0, 200, 200]
    } else {
        color_bytes(&value.color)?
    });
    Ok(command)
}

pub struct Controller {
    pub transport: Transport,
}
impl Controller {
    pub fn open() -> Result<Self, String> {
        Ok(Self {
            transport: Transport::open()?,
        })
    }
    pub fn get_firmware(&self) -> Result<u16, String> {
        let r = self
            .transport
            .query(&[0x80], Some(7), Duration::from_millis(50), 0x80)?;
        Ok(u16::from_le_bytes([r[1], r[2]]))
    }
    pub fn get_battery(&self) -> Result<Battery, String> {
        let r = self
            .transport
            .query(&[0x83], Some(7), Duration::from_millis(50), 0x83)?;
        Ok(Battery {
            percent: r[1],
            state: match r[2] {
                0 => "discharging",
                1 => "charging",
                2 => "full",
                _ => "unknown",
            }
            .into(),
        })
    }
    pub fn set_lighting(&self, value: &LightingState) -> Result<(), String> {
        let c = lighting_report(value)?;
        self.transport.send(&c, Some(8))?;
        thread::sleep(Duration::from_millis(500));
        Ok(())
    }
    pub fn get_lighting(&self) -> Result<LightingState, String> {
        let r = self
            .transport
            .query(&[0x87], Some(7), Duration::from_millis(50), 0x87)?;
        let effect = effect_name(r[1]);
        let option = r[4];
        let mut color = ((r[5] as u32) << 16) | ((r[6] as u32) << 8) | r[7] as u32;
        if color == 0xfafffa {
            color = 0xffffff
        }
        Ok(LightingState {
            effect: effect.clone(),
            brightness: r[3],
            speed: 4u8.saturating_sub(r[2]),
            color: format!("#{color:06x}"),
            rainbow: (option & 15)
                == if matches!(effect.as_str(), "music_2" | "music_3") {
                    0
                } else {
                    8
                },
            direction: match option >> 4 {
                1 => "left",
                2 => "down",
                3 => "up",
                _ => "right",
            }
            .into(),
            slot: if effect == "user_picture" {
                (option >> 4) + 1
            } else {
                1
            },
        })
    }
    pub fn get_profile(&self) -> Result<u8, String> {
        Ok(self
            .transport
            .query(&[0x85], Some(7), Duration::from_millis(50), 0x85)?[1])
    }
    pub fn set_profile(&self, p: u8) -> Result<(), String> {
        if p > 2 {
            return Err("Profile must be 0..2".into());
        }
        self.transport.send(&[5, p], Some(7))?;
        thread::sleep(Duration::from_millis(500));
        Ok(())
    }
    pub fn get_report_rate(&self, p: u8) -> Result<u16, String> {
        let r = self
            .transport
            .query(&[0x84, p], Some(7), Duration::from_millis(50), 0x84)?;
        match r[2] {
            1 => Ok(1000),
            2 => Ok(500),
            4 => Ok(250),
            8 => Ok(125),
            v => Err(format!("Unknown report-rate value: {v}")),
        }
    }
    pub fn set_report_rate(&self, h: u16, p: u8) -> Result<(), String> {
        if p > 2 {
            return Err("Profile must be 0..2".into());
        }
        let v = match h {
            1000 => 1,
            500 => 2,
            250 => 4,
            125 => 8,
            _ => return Err("Report rate must be 125, 250, 500 or 1000".into()),
        };
        self.transport.send(&[4, p, v], Some(7))?;
        thread::sleep(Duration::from_secs(1));
        Ok(())
    }
    pub fn get_options(&self, p: u8) -> Result<KeyboardOptions, String> {
        let r = self
            .transport
            .query(&[0x86, p], Some(7), Duration::from_millis(50), 0x86)?;
        let f = r[2];
        Ok(KeyboardOptions {
            win_lock: f & 1 != 0,
            mac_mode: f & 4 != 0,
            swap_wasd_arrows: f & 8 != 0,
            led_off: f & 16 != 0,
            side_led_off: f & 32 != 0,
            gaming_mode: f & 64 != 0,
            keyboard_lock: f & 128 != 0,
            fn_remap: r[3] & 1 != 0,
            power_save: r[4] != 0,
        })
    }
    pub fn set_options(&self, o: &KeyboardOptions, p: u8) -> Result<(), String> {
        if p > 2 {
            return Err("Profile must be 0..2".into());
        }
        let f = o.win_lock as u8
            | (o.mac_mode as u8) << 2
            | (o.swap_wasd_arrows as u8) << 3
            | (o.led_off as u8) << 4
            | (o.side_led_off as u8) << 5
            | (o.gaming_mode as u8) << 6
            | (o.keyboard_lock as u8) << 7;
        let mut c = [0u8; 64];
        c[..5].copy_from_slice(&[6, p, f, o.fn_remap as u8, o.power_save as u8]);
        self.transport.send(&c, Some(7))
    }
    pub fn get_sleep(&self) -> Result<SleepTimes, String> {
        let r = self
            .transport
            .query(&[0x92], Some(7), Duration::from_millis(50), 0x92)?;
        Ok(SleepTimes {
            bluetooth: u16::from_le_bytes([r[1], r[2]]),
            wireless: u16::from_le_bytes([r[3], r[4]]),
            bluetooth_deep: u16::from_le_bytes([r[5], r[6]]),
            wireless_deep: u16::from_le_bytes([r[7], r[8]]),
        })
    }
    pub fn set_sleep(&self, v: [u16; 4]) -> Result<(), String> {
        let mut c = [0u8; 64];
        c[0] = 0x12;
        for (i, n) in v.iter().enumerate() {
            c[8 + i * 2..10 + i * 2].copy_from_slice(&n.to_le_bytes())
        }
        self.transport.send(&c, Some(7))?;
        thread::sleep(Duration::from_millis(500));
        Ok(())
    }
    pub fn get_debounce(&self) -> Result<u8, String> {
        Ok(self
            .transport
            .query(&[0x91], Some(7), Duration::from_millis(50), 0x91)?[2])
    }
    pub fn set_debounce(&self, v: u8) -> Result<(), String> {
        if v > 10 {
            return Err("Debounce must be 0..10 ms".into());
        }
        self.transport.send(&[0x11, 0, v], Some(7))?;
        thread::sleep(Duration::from_millis(500));
        Ok(())
    }
    pub fn set_clock(&self) -> Result<(), String> {
        let now = Local::now();
        let mut c = [0u8; 64];
        c[0] = 0x28;
        c[8..10].copy_from_slice(&(now.year() as u16).to_be_bytes());
        c[10..15].copy_from_slice(&[
            now.month() as u8,
            now.day() as u8,
            now.hour() as u8,
            now.minute() as u8,
            now.second() as u8,
        ]);
        self.transport.send(&c, Some(7))
    }
    pub fn set_key_payload(
        &self,
        source: u8,
        payload: [u8; 4],
        profile: u8,
        fn_layer: Option<u8>,
    ) -> Result<(), String> {
        if profile > 2 {
            return Err("Profile must be 0..2".into());
        }
        if !matches!(fn_layer, None | Some(0)) {
            return Err("K8 has one Fn layer".into());
        }
        let mut c = [0u8; 64];
        c[0] = if fn_layer.is_some() { 0x15 } else { 0x13 };
        c[1] = fn_layer.unwrap_or(profile);
        c[2] = keys::matrix_index(source)?;
        c[8..12].copy_from_slice(&payload);
        self.transport.send(&c, Some(7))
    }
    pub fn set_key_color(&self, source: u8, color: &str, slot: u8) -> Result<(), String> {
        if slot > 3 {
            return Err("Lighting slot must be 0..3".into());
        }
        let mut c = [0u8; 64];
        c[0] = 0x14;
        c[1] = slot;
        c[2] = keys::matrix_index(source)?;
        c[8..11].copy_from_slice(&color_bytes(color)?);
        self.transport.send(&c, Some(7))
    }
    pub fn screen_color(&self, rgba: &[u8]) -> Result<(), String> {
        if rgba.len() != 4 {
            return Err("RGBA must contain four values".into());
        }
        let mut c = vec![0x0e];
        c.extend_from_slice(rgba);
        self.transport.send(&c, Some(7))
    }
    pub fn music(&self, levels: &[u8]) -> Result<(), String> {
        if levels.len() != 32 {
            return Err("Music data must contain 32 values".into());
        }
        let mut c = [0u8; 64];
        c[0] = 0x0d;
        c[8..40].copy_from_slice(levels);
        self.transport.send(&c, Some(7))
    }
    pub fn macro_assign(
        &self,
        source: u8,
        events: &[(u8, bool, u16)],
        index: u8,
        repeat: u16,
        mode: &str,
        profile: u8,
    ) -> Result<(), String> {
        if index >= 50 {
            return Err("Macro slot must be 0..49".into());
        }
        let mode = match mode {
            "repeat" => 0,
            "toggle" => 1,
            "while_held" => 2,
            _ => return Err("Invalid macro mode".into()),
        };
        let mut encoded = repeat.to_le_bytes().to_vec();
        for (usage, down, delay) in events {
            if !(4..=239).contains(usage) {
                return Err("Invalid macro key".into());
            }
            encoded.push(*usage);
            encoded.push(
                (if *down { 0x80 } else { 0 }) | if *delay <= 127 { *delay as u8 } else { 0 },
            );
            if *delay > 127 || *delay == 0 {
                encoded.extend_from_slice(&delay.to_le_bytes())
            }
            if encoded.len() > 256 {
                return Err("Macro is too long".into());
            }
        }
        if events.is_empty() {
            return Err("Macro has no events".into());
        }
        for (i, chunk) in encoded.chunks(56).enumerate() {
            let mut c = [0u8; 64];
            c[..5].copy_from_slice(&[
                0x16,
                index,
                i as u8,
                56,
                (i + 1 == encoded.len().div_ceil(56)) as u8,
            ]);
            c[8..8 + chunk.len()].copy_from_slice(chunk);
            self.transport.send(&c, Some(7))?
        }
        thread::sleep(Duration::from_secs(1));
        self.set_key_payload(source, [9, mode, index, 0], profile, None)
    }
    pub fn reset(&self) -> Result<(), String> {
        self.transport.send(&[2], Some(7))?;
        thread::sleep(Duration::from_secs(1));
        Ok(())
    }
    pub fn status(&self) -> Result<Status, String> {
        let p = self.get_profile()?;
        Ok(Status {
            firmware: self.get_firmware()?,
            battery: self.get_battery()?,
            lighting: self.get_lighting()?,
            report_rate: self.get_report_rate(p)?,
            profile: p,
            options: self.get_options(p)?,
            sleep: self.get_sleep()?,
            debounce: self.get_debounce()?,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn lighting(effect: &str) -> LightingState {
        LightingState {
            effect: effect.into(),
            brightness: 4,
            speed: 2,
            color: "#123456".into(),
            rainbow: false,
            direction: "right".into(),
            slot: 1,
        }
    }

    #[test]
    fn encodes_fixed_and_rainbow_colors() {
        let fixed = lighting_report(&lighting("solid")).unwrap();
        assert_eq!(&fixed[..8], &[7, 1, 2, 4, 7, 0x12, 0x34, 0x56]);
        let mut rainbow = lighting("wave");
        rainbow.rainbow = true;
        rainbow.direction = "left".into();
        let report = lighting_report(&rainbow).unwrap();
        assert_eq!(report[4], 0x18);
    }

    #[test]
    fn encodes_music_direction_and_custom_slot() {
        let mut music = lighting("music_3");
        music.direction = "down".into();
        assert_eq!(lighting_report(&music).unwrap()[4], 0x24);
        let mut custom = lighting("user_picture");
        custom.slot = 4;
        assert_eq!(lighting_report(&custom).unwrap()[4], 0x30);
    }
}
