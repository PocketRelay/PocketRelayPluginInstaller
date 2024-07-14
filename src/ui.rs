/// Title used for created windows
pub const WINDOW_TITLE: &str =
    concat!("Pocket Relay Plugin Installer v", env!("CARGO_PKG_VERSION"));
/// Window icon bytes
pub const ICON_BYTES: &[u8] = include_bytes!("./resources/icon.ico");

/// Shows a info message to the user.
///
/// ## Arguments
/// * `title` - The title for the dialog
/// * `text`  - The text for the dialog
pub fn show_info(title: &str, text: &str) {
    native_dialog::MessageDialog::new()
        .set_title(title)
        .set_text(text)
        .set_type(native_dialog::MessageType::Info)
        .show_alert()
        .unwrap()
}

/// Shows an error message to the user.
///
/// ## Arguments
/// * `title` - The title for the dialog
/// * `text`  - The text for the dialog
pub fn show_error(title: &str, text: &str) {
    native_dialog::MessageDialog::new()
        .set_title(title)
        .set_text(text)
        .set_type(native_dialog::MessageType::Error)
        .show_alert()
        .unwrap()
}
