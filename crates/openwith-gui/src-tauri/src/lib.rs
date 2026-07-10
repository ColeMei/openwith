mod commands;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_dialog::init())
        .invoke_handler(tauri::generate_handler![
            commands::detect_cli,
            commands::relaunch_finder,
            commands::get_snapshot,
            commands::set_default,
            commands::set_scheme_default,
            commands::export_toml,
            commands::import_toml,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
