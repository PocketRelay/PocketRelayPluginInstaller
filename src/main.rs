#![cfg_attr(
    all(target_os = "windows", not(debug_assertions),),
    windows_subsystem = "windows"
)]
#![warn(unused_crate_dependencies)]

mod bink;
mod github;
mod iced;
mod plugin;

/// Application crate version string
pub const APP_VERSION: &str = env!("CARGO_PKG_VERSION");

fn main() {
    // Initialize logging
    env_logger::builder()
        .filter_module("pocket_relay_plugin_installer", log::LevelFilter::Debug)
        .init();

    // Initialize the UI
    iced::init();
}
