use std::sync::Mutex;

use tauri::tray::{MouseButton, MouseButtonState, TrayIcon, TrayIconBuilder, TrayIconEvent};
use tauri::{AppHandle, Manager};
use tauri_plugin_positioner::{Position, WindowExt};

/// The live tray icon, if the "Show in menu bar" setting is on.
#[derive(Default)]
pub struct TrayState(pub Mutex<Option<TrayIcon>>);

pub fn set_enabled(app: &AppHandle, enabled: bool) -> tauri::Result<()> {
    let state = app.state::<TrayState>();
    let mut slot = state.0.lock().expect("tray state poisoned");
    if enabled {
        if slot.is_none() {
            *slot = Some(build(app)?);
        }
    } else if let Some(tray) = slot.take() {
        // Dropping the handle removes the icon from the menu bar.
        drop(tray);
    }
    Ok(())
}

fn build(app: &AppHandle) -> tauri::Result<TrayIcon> {
    // Monochrome template image: macOS recolors it for light/dark menu bars
    // and the pressed state, like native status items.
    let icon = tauri::image::Image::from_bytes(include_bytes!("../icons/tray-template.png"))?;
    TrayIconBuilder::with_id("openwith-tray")
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
        .build(app)
}

pub fn toggle_popover(app: &AppHandle) {
    let Some(window) = app.get_webview_window("menubar") else {
        return;
    };
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
}
