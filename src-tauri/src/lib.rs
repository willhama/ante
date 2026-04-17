mod ai;
mod commands;
mod errors;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .manage(ai::AiState::default())
        .invoke_handler(tauri::generate_handler![
            commands::open_file,
            commands::save_file,
            commands::save_file_as,
            ai::stream_completion,
            ai::cancel_completion,
            ai::get_ai_config,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
