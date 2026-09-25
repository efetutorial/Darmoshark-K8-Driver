mod controller;
mod display;
mod keys;
mod live;
mod transport;

use controller::{Controller, KeyboardOptions, LightingState};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::{
    collections::HashMap,
    env,
    sync::{Arc, Mutex},
};
use tauri::Manager;

#[derive(Clone, Serialize)]
struct Progress {
    active: bool,
    value: f64,
    label: String,
}
impl Default for Progress {
    fn default() -> Self {
        Self {
            active: false,
            value: 0.0,
            label: String::new(),
        }
    }
}
struct AppState {
    device: Arc<Mutex<()>>,
    progress: Mutex<Progress>,
    live: Mutex<Option<live::LiveSession>>,
}

fn request_body<'a>(value: &'a Option<Value>) -> Result<&'a Value, String> {
    value
        .as_ref()
        .ok_or_else(|| "Request body is missing".into())
}
fn number(value: &Value, name: &str) -> Result<u64, String> {
    value
        .get(name)
        .and_then(Value::as_u64)
        .ok_or_else(|| format!("{name} is missing"))
}
fn text_value<'a>(value: &'a Value, name: &str) -> Result<&'a str, String> {
    value
        .get(name)
        .and_then(Value::as_str)
        .ok_or_else(|| format!("{name} is missing"))
}
fn controller_locked(
    state: &AppState,
) -> Result<(std::sync::MutexGuard<'_, ()>, Controller), String> {
    let guard = state
        .device
        .lock()
        .map_err(|_| "Keyboard lock failed".to_string())?;
    let controller = Controller::open()?;
    Ok((guard, controller))
}

#[derive(Deserialize)]
struct MacroEvent {
    key: String,
    action: String,
    delay: u16,
}

#[tauri::command]
fn api(
    state: tauri::State<'_, AppState>,
    path: String,
    body_value: Option<Value>,
    bytes: Option<Vec<u8>>,
    query: Option<HashMap<String, String>>,
) -> Result<Value, String> {
    if path == "/api/live-native-stop" {
        state
            .live
            .lock()
            .map_err(|_| "Live sync lock failed")?
            .take();
        return Ok(json!({"ok":true}));
    }
    if path == "/api/live-native-start" {
        let data = request_body(&body_value)?;
        let kind = text_value(data, "kind")?;
        let mut session = state.live.lock().map_err(|_| "Live sync lock failed")?;
        session.take();
        *session = Some(match kind {
            "screen" => live::start_screen(state.device.clone())?,
            "audio" => live::start_audio(state.device.clone())?,
            _ => return Err(format!("Unknown live sync type: {kind}")),
        });
        return Ok(json!({"ok":true}));
    }

    match path.as_str() {
        "/api/device" => {
            let devices = transport::discover();
            Ok(
                json!({"connected":!devices.is_empty(),"devices":devices,"keys":keys::keys(),"targets":keys::target_names(),"effects":[]}),
            )
        }
        "/api/progress" => Ok(serde_json::to_value(
            state
                .progress
                .lock()
                .map_err(|_| "Progress lock failed")?
                .clone(),
        )
        .unwrap()),
        "/api/system-card" => {
            let (memory_percent, uptime_seconds) = display::system_card()?;
            Ok(json!({"ok":true,"memory_percent":memory_percent,"uptime_seconds":uptime_seconds}))
        }
        "/api/display-capture" => Ok(json!({"ok":true,"png":display::capture_screen()?})),
        "/api/status" => {
            let (_guard, controller) = controller_locked(&state)?;
            Ok(json!({"ok":true,"status":controller.status()?}))
        }
        "/api/display" | "/api/display-frame" => {
            state
                .live
                .lock()
                .map_err(|_| "Live sync lock failed")?
                .take();
            let data = bytes.ok_or_else(|| "Image data is missing".to_string())?;
            let query = query.unwrap_or_default();
            let layer = query.get("layer").and_then(|v| v.parse().ok()).unwrap_or(0);
            let animation = if path == "/api/display-frame" {
                display::encode(vec![data], 100)?
            } else {
                let filename = query
                    .get("filename")
                    .map(String::as_str)
                    .unwrap_or("image.png");
                let delay = query.get("delay").and_then(|v| v.parse().ok());
                display::decode(&data, filename, delay)?
            };
            *state.progress.lock().map_err(|_| "Progress lock failed")? = Progress {
                active: true,
                value: 0.0,
                label: "Uploading".into(),
            };
            let (_guard, controller) = controller_locked(&state)?;
            let result =
                display::upload(&controller.transport, &animation, layer, |value, label| {
                    if let Ok(mut progress) = state.progress.lock() {
                        *progress = Progress {
                            active: value < 1.0,
                            value,
                            label,
                        }
                    }
                });
            if result.is_err() {
                *state.progress.lock().map_err(|_| "Progress lock failed")? = Progress {
                    active: false,
                    value: 0.0,
                    label: "Upload failed".into(),
                };
            }
            result?;
            Ok(
                json!({"ok":true,"frames":animation.frames.len(),"delay":animation.delay,"bounds":animation.bounds}),
            )
        }
        _ => {
            let data = request_body(&body_value)?;
            if path == "/api/rgb" {
                state
                    .live
                    .lock()
                    .map_err(|_| "Live sync lock failed")?
                    .take();
            }
            let (_guard, controller) = controller_locked(&state)?;
            match path.as_str() {
                "/api/rgb" => controller.set_lighting(
                    &serde_json::from_value::<LightingState>(data.clone())
                        .map_err(|error| error.to_string())?,
                )?,
                "/api/profile" => controller.set_profile(number(data, "profile")? as u8)?,
                "/api/rate" => {
                    let hz = number(data, "hz")? as u16;
                    let profile = number(data, "profile")? as u8;
                    controller.set_report_rate(hz, profile)?;
                    let actual = controller.get_report_rate(profile)?;
                    if actual != hz {
                        return Err(format!(
                            "Keyboard reported {actual} Hz after requesting {hz} Hz"
                        ));
                    }
                }
                "/api/debounce" => controller.set_debounce(number(data, "milliseconds")? as u8)?,
                "/api/options" => {
                    let profile = number(data, "profile")? as u8;
                    let options: KeyboardOptions =
                        serde_json::from_value(data.clone()).map_err(|error| error.to_string())?;
                    controller.set_options(&options, profile)?
                }
                "/api/sleep" => controller.set_sleep([
                    (number(data, "bluetooth")? * 60) as u16,
                    (number(data, "wireless")? * 60) as u16,
                    (number(data, "bluetooth_deep")? * 60) as u16,
                    (number(data, "wireless_deep")? * 60) as u16,
                ])?,
                "/api/clock" => controller.set_clock()?,
                "/api/remap" => {
                    let modifier = match data
                        .get("modifier")
                        .and_then(Value::as_str)
                        .unwrap_or("none")
                    {
                        "ctrl" => 224,
                        "shift" => 225,
                        "alt" => 226,
                        "meta" => 227,
                        _ => 0,
                    };
                    let source = keys::parse_key(text_value(data, "source")?)?;
                    let target = keys::parse_target(
                        data.get("target")
                            .and_then(Value::as_str)
                            .unwrap_or("disabled"),
                        modifier,
                        number(data, "second_usage").unwrap_or(0) as u8,
                    )?;
                    let profile = number(data, "profile")? as u8;
                    let layer = data
                        .get("fn_layer")
                        .and_then(Value::as_u64)
                        .map(|v| v as u8);
                    controller.set_key_payload(source, target, profile, layer)?
                }
                "/api/key-color" => controller.set_key_color(
                    keys::parse_key(text_value(data, "key")?)?,
                    text_value(data, "color")?,
                    number(data, "slot")? as u8,
                )?,
                "/api/live-screen" => {
                    let rgba = data
                        .get("rgba")
                        .and_then(Value::as_array)
                        .ok_or("RGBA values missing")?
                        .iter()
                        .map(|v| v.as_u64().unwrap_or(0) as u8)
                        .collect::<Vec<_>>();
                    controller.screen_color(&rgba)?
                }
                "/api/live-audio" => {
                    let levels = data
                        .get("levels")
                        .and_then(Value::as_array)
                        .ok_or("Audio levels missing")?
                        .iter()
                        .map(|v| v.as_u64().unwrap_or(0) as u8)
                        .collect::<Vec<_>>();
                    controller.music(&levels)?
                }
                "/api/macro" => {
                    let events: Vec<MacroEvent> = serde_json::from_value(
                        data.get("events").cloned().ok_or("Events missing")?,
                    )
                    .map_err(|error| error.to_string())?;
                    let encoded = events
                        .into_iter()
                        .map(|event| {
                            Ok((
                                keys::parse_key(&event.key)?,
                                event.action == "down",
                                event.delay,
                            ))
                        })
                        .collect::<Result<Vec<_>, String>>()?;
                    controller.macro_assign(
                        keys::parse_key(text_value(data, "source")?)?,
                        &encoded,
                        number(data, "index")? as u8,
                        number(data, "repeat_count")? as u16,
                        text_value(data, "mode")?,
                        number(data, "profile")? as u8,
                    )?
                }
                "/api/reset" => controller.reset()?,
                _ => return Err(format!("Unknown API: {path}")),
            }
            Ok(json!({"ok":true}))
        }
    }
}

#[tauri::command]
fn frontend_report(report: String) {
    eprintln!("K8 Studio frontend: {report}")
}

pub fn run() {
    #[cfg(target_os = "linux")]
    {
        if env::var_os("GSETTINGS_SCHEMA_DIR").is_none() {
            let system = std::path::PathBuf::from("/run/current-system/sw/share/glib-2.0/schemas");
            let mut schema_dir = system.join("gschemas.compiled").exists().then_some(system);
            if schema_dir.is_none() {
                'packages: for package in ["gtk+3-", "gsettings-desktop-schemas-"] {
                    if let Ok(entries) = std::fs::read_dir("/nix/store") {
                        for entry in entries.flatten() {
                            if !entry.file_name().to_string_lossy().contains(package) {
                                continue;
                            }
                            let roots = entry.path().join("share/gsettings-schemas");
                            if let Ok(versions) = std::fs::read_dir(roots) {
                                for version in versions.flatten() {
                                    let candidate = version.path().join("glib-2.0/schemas");
                                    if candidate.join("gschemas.compiled").exists() {
                                        schema_dir = Some(candidate);
                                        break 'packages;
                                    }
                                }
                            }
                        }
                    }
                }
            }
            if let Some(path) = schema_dir {
                env::set_var("GSETTINGS_SCHEMA_DIR", path);
            }
        }
        if env::var_os("GDK_BACKEND").is_none() {
            env::set_var("GDK_BACKEND", "x11")
        }
        if env::var_os("WEBKIT_DISABLE_DMABUF_RENDERER").is_none() {
            env::set_var("WEBKIT_DISABLE_DMABUF_RENDERER", "1")
        }
    }
    tauri::Builder::default()
        .manage(AppState {
            device: Arc::new(Mutex::new(())),
            progress: Mutex::new(Progress::default()),
            live: Mutex::new(None),
        })
        .setup(|app| {
            if let Some(window) = app.get_webview_window("main") {
                #[cfg(target_os = "linux")]
                window.with_webview(|webview| {
                    use webkit2gtk::{
                        glib::prelude::ObjectExt, PermissionRequestExt, UserMediaPermissionRequest,
                        WebViewExt,
                    };
                    webview.inner().connect_permission_request(|_, request| {
                        if request.is::<UserMediaPermissionRequest>() {
                            request.allow();
                            true
                        } else {
                            false
                        }
                    });
                })?;
            }
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![api, frontend_report])
        .run(tauri::generate_context!())
        .expect("Could not run Darmoshark K8 Studio");
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{thread, time::Duration};
    #[test]
    fn named_targets_are_stable() {
        assert_eq!(
            keys::parse_target("media:volume-up", 0, 0).unwrap(),
            [3, 0, 233, 0]
        );
        assert_eq!(keys::matrix_index(57).unwrap(), 3);
    }
    #[test]
    fn options_default_is_clear() {
        assert!(!controller::KeyboardOptions::default().win_lock);
    }

    #[test]
    #[ignore = "requires the attached K8 and changes lighting briefly"]
    fn hardware_lighting_and_live_sync_round_trip() {
        let controller = Controller::open().unwrap();
        let original = controller.get_lighting().unwrap();
        let requested = LightingState {
            effect: "solid".into(),
            brightness: 3,
            speed: 2,
            color: "#123456".into(),
            rainbow: false,
            direction: "right".into(),
            slot: 1,
        };
        controller.set_lighting(&requested).unwrap();
        let actual = controller.get_lighting().unwrap();
        drop(controller);
        let live_result = (|| -> Result<(), String> {
            let lock = Arc::new(Mutex::new(()));
            let screen = live::start_screen(lock.clone())?;
            thread::sleep(Duration::from_millis(350));
            drop(screen);
            let audio = live::start_audio(lock)?;
            thread::sleep(Duration::from_millis(350));
            drop(audio);
            Ok(())
        })();
        Controller::open().unwrap().set_lighting(&original).unwrap();
        live_result.unwrap();

        assert_eq!(actual.effect, requested.effect);
        assert_eq!(actual.color, requested.color);
        assert!(!actual.rainbow);
    }
}
