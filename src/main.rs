#![cfg_attr(
    all(target_os = "windows", not(debug_assertions)),
    windows_subsystem = "windows"
)]

mod backend;
mod controller;
mod display;
mod keys;
mod live;
mod model;
mod presets;
mod record;
mod requests;
mod transport;
mod ui;
#[cfg(target_os = "windows")]
mod windows_capture;

fn main() {
    ui::run();
}
