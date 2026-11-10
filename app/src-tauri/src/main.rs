//! ByteForest desktop shell: window, IPC commands and logging.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod commands;
mod dto;
mod logging;
mod state;

fn main() {
    let _log_guard = logging::init();
    tracing::info!("ByteForest starting");

    let result = tauri::Builder::default()
        .manage(state::AppState::default())
        .invoke_handler(tauri::generate_handler![
            commands::list_drives,
            commands::start_scan,
            commands::cancel_scan,
            commands::get_children,
            commands::get_node,
            commands::get_path,
            commands::get_treemap,
            commands::is_elevated,
        ])
        .run(tauri::generate_context!());
    if let Err(e) = result {
        tracing::error!("tauri failed: {e}");
        eprintln!("error while running ByteForest: {e}");
        std::process::exit(1);
    }
}
