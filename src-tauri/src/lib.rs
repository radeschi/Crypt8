use tauri::Manager;

mod archive;
mod commands;
mod crypto;
mod errors;
mod filesystem;
mod platform;
mod progress;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_process::init())
        .plugin(tauri_plugin_updater::Builder::new().build())
        .setup(|app| {
            if let Some(window) = app.get_webview_window("main") {
                platform::configure_utility_window(&window);
            }
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::inspect_file,
            commands::inspect_entries,
            commands::suggested_output_for,
            commands::classify_plaintext,
            commands::prepare_open_directory,
            commands::suggested_gpg_path,
            commands::suggested_plaintext_name_for,
            commands::prepare_open_target,
            commands::path_exists,
            commands::folder_action_label,
            commands::cancel_encrypt,
            commands::encrypt_file,
            commands::verify_decrypt_password,
            commands::decrypt_file,
        ])
        .build(tauri::generate_context!())
        .expect("error while running tauri application")
        .run(|_app, event| {
            if let tauri::RunEvent::Exit = event {
                filesystem::cleanup_temporary_files();
            }
        });
}
