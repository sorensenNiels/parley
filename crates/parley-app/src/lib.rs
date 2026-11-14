//! What both of Parley's programs need from the desktop: `parley`, the terminal, and
//! `parley-sql`, Parley SQL, the SQL window on its own (R-SQL-45). The window icon, the SQL
//! helper's jars, the file window, the snippet and macro folders, the update check and the
//! development screenshot. UI code belongs in `parley-ui-egui`; the engine in `parley-core`.

pub mod helpers;
pub mod macros;
#[expect(
    clippy::print_stdout,
    clippy::print_stderr,
    reason = "a development option of the binaries, which says where it saved the window"
)]
pub mod screenshot;
pub mod snippets;

use eframe::egui;

/// The window icon.
///
/// # Errors
/// When the bundled PNG cannot be decoded.
pub fn window_icon() -> Result<egui::IconData, String> {
    eframe::icon_data::from_png_bytes(include_bytes!("../icons/256.png")).map_err(|e| e.to_string())
}

/// The icon for the start of the menu bar.
///
/// # Errors
/// When the bundled PNG cannot be decoded.
pub fn logo() -> Result<egui::ColorImage, String> {
    let icon = eframe::icon_data::from_png_bytes(include_bytes!("../icons/64.png"))
        .map_err(|e| e.to_string())?;
    Ok(egui::ColorImage::from_rgba_unmultiplied(
        [icon.width as usize, icon.height as usize],
        &icon.rgba,
    ))
}

/// Asks GitHub for the latest release of Parley, which Parley SQL ships with (R-UI-11).
#[must_use]
pub fn update_checker() -> parley_ui_egui::Checker {
    std::sync::Arc::new(|| {
        parley_core::update::check(
            parley_core::update::LATEST_RELEASE_URL,
            env!("CARGO_PKG_VERSION"),
        )
        .map_err(|e| e.to_string())
    })
}
