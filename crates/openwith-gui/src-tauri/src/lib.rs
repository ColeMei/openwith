mod commands;
mod tray;

use tauri_plugin_autostart::MacosLauncher;
use tauri_plugin_global_shortcut::{Code, Modifiers, Shortcut, ShortcutState};

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let toggle_shortcut = Shortcut::new(Some(Modifiers::ALT | Modifiers::SUPER), Code::KeyO);

    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_positioner::init())
        .plugin(tauri_plugin_autostart::init(
            MacosLauncher::LaunchAgent,
            None,
        ))
        .plugin(
            tauri_plugin_global_shortcut::Builder::new()
                .with_shortcuts([toggle_shortcut])
                .expect("valid shortcut")
                .with_handler(move |app, shortcut, event| {
                    if shortcut == &toggle_shortcut && event.state == ShortcutState::Pressed {
                        tray::toggle_popover(app);
                    }
                })
                .build(),
        )
        .manage(commands::AppsCache::default())
        .manage(tray::TrayState::default())
        .on_window_event(|window, event| {
            // The popover behaves like a menu: clicking anywhere else closes it.
            if window.label() == "menubar" && matches!(event, tauri::WindowEvent::Focused(false)) {
                let _ = window.hide();
            }
        })
        .invoke_handler(tauri::generate_handler![
            commands::detect_cli,
            commands::relaunch_finder,
            commands::get_history,
            commands::get_snapshot,
            commands::set_default,
            commands::set_scheme_default,
            commands::export_toml,
            commands::import_toml,
            commands::search_extensions,
            commands::get_ext_picker,
            commands::get_recent_changes,
            commands::show_main_window,
            commands::quit_app,
            commands::set_tray_enabled,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
