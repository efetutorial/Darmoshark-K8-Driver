//! Lighting presets, stored as JSON in the user's config directory. The file
//! format matches the old web UI's export (`{"version":1,"presets":[...]}`).

use crate::{controller::LightingState, model};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::path::{Path, PathBuf};

pub const MAX_PRESETS: usize = 20;

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Preset {
    pub name: String,
    pub effect: String,
    pub color: String,
    pub brightness: u8,
    pub speed: u8,
    pub rainbow: bool,
    pub direction: String,
    #[serde(default = "default_slot")]
    pub slot: u8,
}

fn default_slot() -> u8 {
    1
}

impl Preset {
    pub fn from_lighting(name: String, lighting: &LightingState) -> Self {
        Self {
            name,
            effect: lighting.effect.clone(),
            color: lighting.color.clone(),
            brightness: lighting.brightness,
            speed: lighting.speed,
            rainbow: lighting.rainbow,
            direction: lighting.direction.clone(),
            slot: lighting.slot,
        }
    }

    pub fn lighting(&self) -> LightingState {
        LightingState {
            effect: self.effect.clone(),
            brightness: self.brightness,
            speed: self.speed,
            color: self.color.clone(),
            rainbow: self.rainbow,
            direction: self.direction.clone(),
            slot: self.slot,
        }
    }

    pub fn validate(&self) -> Result<(), String> {
        let hex = self
            .color
            .strip_prefix('#')
            .is_some_and(|hex| hex.len() == 6 && hex.chars().all(|c| c.is_ascii_hexdigit()));
        if self.name.trim().is_empty()
            || self.name.chars().count() > 100
            || !model::is_listed_effect(&self.effect)
            || !hex
            || self.brightness > 4
            || self.speed > 4
            || !["right", "left", "down", "up"].contains(&self.direction.as_str())
            || !(1..=4).contains(&self.slot)
        {
            return Err("Invalid lighting preset".into());
        }
        Ok(())
    }
}

#[derive(Serialize)]
struct ExportFile<'a> {
    version: u8,
    presets: &'a [Preset],
}

/// Keep the valid presets from a JSON value and report how many were dropped.
/// Accepts either the export object or a bare array.
fn parse(value: &Value) -> Result<(Vec<Preset>, usize), String> {
    let list = match value {
        Value::Array(list) => list,
        Value::Object(map) => map
            .get("presets")
            .and_then(Value::as_array)
            .ok_or("The file has no presets list")?,
        _ => return Err("The file has no presets list".into()),
    };
    let mut valid = Vec::new();
    let mut dropped = 0;
    for item in list {
        match serde_json::from_value::<Preset>(item.clone()) {
            Ok(preset) if preset.validate().is_ok() => valid.push(preset),
            _ => dropped += 1,
        }
    }
    Ok((valid, dropped))
}

fn keep_latest(mut presets: Vec<Preset>) -> Vec<Preset> {
    if presets.len() > MAX_PRESETS {
        presets.drain(..presets.len() - MAX_PRESETS);
    }
    presets
}

pub fn config_dir() -> Option<PathBuf> {
    #[cfg(target_os = "windows")]
    let base = std::env::var_os("APPDATA").map(PathBuf::from);
    #[cfg(not(target_os = "windows"))]
    let base = std::env::var_os("XDG_CONFIG_HOME")
        .map(PathBuf::from)
        .filter(|path| path.is_absolute())
        .or_else(|| std::env::var_os("HOME").map(|home| PathBuf::from(home).join(".config")));
    base.map(|base| base.join("darmoshark-k8-studio"))
}

pub struct Store {
    path: Option<PathBuf>,
    pub presets: Vec<Preset>,
    /// Set when invalid entries were skipped while loading.
    pub recovered: bool,
}

impl Store {
    pub fn load() -> Self {
        Self::load_from(config_dir().map(|dir| dir.join("presets.json")))
    }

    pub fn load_from(path: Option<PathBuf>) -> Self {
        let mut store = Self {
            path,
            presets: Vec::new(),
            recovered: false,
        };
        let Some(path) = store.path.clone() else {
            return store;
        };
        let Ok(text) = std::fs::read_to_string(&path) else {
            return store;
        };
        match serde_json::from_str::<Value>(&text)
            .map_err(|e| e.to_string())
            .and_then(|v| parse(&v))
        {
            Ok((presets, dropped)) => {
                store.presets = keep_latest(presets);
                store.recovered = dropped > 0;
            }
            Err(_) => store.recovered = true,
        }
        if store.recovered {
            // Keep the unreadable original next to the cleaned file.
            let _ = std::fs::write(path.with_extension("json.recovery"), text);
            let _ = store.save();
        }
        store
    }

    pub fn save(&self) -> Result<(), String> {
        let Some(path) = &self.path else {
            return Err("No configuration directory is available".into());
        };
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).map_err(|error| error.to_string())?;
        }
        std::fs::write(path, self.export_json()).map_err(|error| error.to_string())
    }

    pub fn add(&mut self, preset: Preset) -> Result<(), String> {
        preset.validate()?;
        self.presets.push(preset);
        self.presets = keep_latest(std::mem::take(&mut self.presets));
        self.save()
    }

    pub fn remove(&mut self, index: usize) -> Result<(), String> {
        if index < self.presets.len() {
            self.presets.remove(index);
        }
        self.save()
    }

    pub fn export_json(&self) -> String {
        serde_json::to_string_pretty(&ExportFile {
            version: 1,
            presets: &self.presets,
        })
        .unwrap_or_default()
    }

    pub fn export_to(&self, path: &Path) -> Result<(), String> {
        std::fs::write(path, self.export_json()).map_err(|error| error.to_string())
    }

    /// Merge presets from an export file. Returns the number imported.
    pub fn import_from(&mut self, path: &Path) -> Result<usize, String> {
        let metadata = std::fs::metadata(path).map_err(|error| error.to_string())?;
        if metadata.len() > 1024 * 1024 {
            return Err("Preset file exceeds 1 MB".into());
        }
        let text = std::fs::read_to_string(path).map_err(|error| error.to_string())?;
        let value: Value = serde_json::from_str(&text).map_err(|error| error.to_string())?;
        let (presets, _) = parse(&value)?;
        if presets.is_empty() {
            return Err("The file contains no valid presets".into());
        }
        let count = presets.len();
        self.presets.extend(presets);
        self.presets = keep_latest(std::mem::take(&mut self.presets));
        self.save()?;
        Ok(count)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn preset(name: &str) -> Preset {
        Preset {
            name: name.into(),
            effect: "solid".into(),
            color: "#123456".into(),
            brightness: 3,
            speed: 2,
            rainbow: false,
            direction: "right".into(),
            slot: 1,
        }
    }

    fn temp(name: &str) -> PathBuf {
        std::env::temp_dir().join(format!("k8-presets-{}-{name}.json", std::process::id()))
    }

    #[test]
    fn imports_old_web_export_and_keeps_latest_twenty() {
        let source = temp("import");
        let list: Vec<_> = (0..25).map(|i| preset(&format!("P{i}"))).collect();
        std::fs::write(
            &source,
            serde_json::to_string(&serde_json::json!({"version":1,"presets":list})).unwrap(),
        )
        .unwrap();
        let target = temp("store");
        let mut store = Store::load_from(Some(target.clone()));
        assert_eq!(store.import_from(&source).unwrap(), 25);
        assert_eq!(store.presets.len(), MAX_PRESETS);
        assert_eq!(store.presets[0].name, "P5");
        let reloaded = Store::load_from(Some(target.clone()));
        assert_eq!(reloaded.presets, store.presets);
        let _ = std::fs::remove_file(source);
        let _ = std::fs::remove_file(target);
    }

    #[test]
    fn invalid_entries_are_skipped_and_flagged() {
        let target = temp("recover");
        let mut bad = preset("Bad");
        bad.effect = "nope".into();
        std::fs::write(
            &target,
            serde_json::to_string(&vec![preset("Good"), bad]).unwrap(),
        )
        .unwrap();
        let store = Store::load_from(Some(target.clone()));
        assert!(store.recovered);
        assert_eq!(store.presets.len(), 1);
        let _ = std::fs::remove_file(&target);
        let _ = std::fs::remove_file(target.with_extension("json.recovery"));
    }

    #[test]
    fn missing_slot_defaults_to_one() {
        let value = serde_json::json!([{
            "name":"Old","effect":"wave","color":"#ff0000","brightness":4,
            "speed":2,"rainbow":true,"direction":"left"
        }]);
        let (presets, dropped) = parse(&value).unwrap();
        assert_eq!(dropped, 0);
        assert_eq!(presets[0].slot, 1);
    }
}
