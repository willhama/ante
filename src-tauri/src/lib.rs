mod ai;
mod commands;
mod errors;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_store::Builder::new().build())
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_opener::init())
        .manage(ai::AiState::default())
        .invoke_handler(tauri::generate_handler![
            commands::open_file,
            commands::read_file,
            commands::list_directory,
            commands::create_document,
            commands::move_path,
            commands::save_file,
            commands::save_file_as,
            commands::save_docx_as,
            commands::pick_image,
            ai::stream_completion,
            ai::cancel_completion,
            ai::get_ai_config,
            ai::load_ai_config,
            ai::save_ai_config,
            ai::test_ai_config,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
