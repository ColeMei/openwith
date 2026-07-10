use std::sync::atomic::Ordering;

use tauri::tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent};
use tauri::{AppHandle, Emitter, Manager};
use tauri_plugin_positioner::{Position, WindowExt};

use crate::commands::PopoverPinned;

const TRAY_ID: &str = "openwith-tray";

/// Add or remove the menu-bar icon. The app's tray registry is the source of
/// truth: `TrayIconBuilder::build` registers the icon there, so removal must
/// go through `remove_tray_by_id` — dropping a handle alone leaks the icon.
pub fn set_enabled(app: &AppHandle, enabled: bool) -> tauri::Result<()> {
    if enabled {
        if app.tray_by_id(TRAY_ID).is_none() {
            build(app)?;
        }
    } else {
        app.remove_tray_by_id(TRAY_ID);
    }
    Ok(())
}

fn build(app: &AppHandle) -> tauri::Result<()> {
    // Monochrome template image: macOS recolors it for light/dark menu bars
    // and the pressed state, like native status items.
    let icon = tauri::image::Image::from_bytes(include_bytes!("../icons/tray-template.png"))?;
    TrayIconBuilder::with_id(TRAY_ID)
        .tooltip("OpenWith")
        .icon(icon)
        .icon_as_template(true)
        .on_tray_icon_event(|tray, event| {
            tauri_plugin_positioner::on_tray_event(tray.app_handle(), &event);
            if let TrayIconEvent::Click {
                button: MouseButton::Left,
                button_state: MouseButtonState::Up,
                ..
            } = event
            {
                toggle_popover(tray.app_handle());
            }
        })
        .build(app)?;
    Ok(())
}

pub fn toggle_popover(app: &AppHandle) {
    let Some(window) = app.get_webview_window("menubar") else {
        return;
    };
    // Every toggle starts unpinned; pinning is a per-showing choice.
    app.state::<PopoverPinned>()
        .0
        .store(false, Ordering::Relaxed);
    if window.is_visible().unwrap_or(false) {
        let _ = window.hide();
        return;
    }
    // Anchor under the tray icon when we have one; otherwise (shortcut with
    // the tray disabled) fall back to the top-right corner.
    if window.move_window(Position::TrayBottomCenter).is_err() {
        let _ = window.move_window(Position::TopRight);
    }
    let _ = window.show();
    let _ = window.set_focus();
    // Focus events are unreliable for the transparent popover panel, so tell
    // the webview explicitly that it just opened (refresh + reset pin UI).
    let _ = window.emit("popover-shown", ());
}
