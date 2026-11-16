//! ByteForest desktop shell: window, IPC commands and logging.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod commands;
mod dto;
mod elevation;
mod logging;
mod settings;
mod state;

fn main() {
    let _log_guard = logging::init();
    std::panic::set_hook(Box::new(|info| tracing::error!("panic: {info}")));
    tracing::info!("ByteForest starting");

    let args: Vec<String> = std::env::args().collect();
    let saved = settings::load_default_path();
    if elevation::should_auto_elevate(saved.always_elevated, bf_scan::is_elevated(), &args) {
        match elevation::relaunch_elevated(&elevation::build_elevated_args(
            elevation::parse_startup_scan(&args).as_deref(),
        )) {
            Ok(()) => return,
            Err(e) => tracing::info!("continuing without elevation: {e:?}"),
        }
    }

    let app_state = state::AppState::default();
    *app_state
        .startup_scan
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner) = elevation::parse_startup_scan(&args);

    let result = tauri::Builder::default()
        .manage(app_state)
        .invoke_handler(tauri::generate_handler![
            commands::list_drives,
            commands::start_scan,
            commands::cancel_scan,
            commands::get_children,
            commands::get_node,
            commands::get_path,
            commands::get_treemap,
            commands::is_elevated,
            commands::restart_elevated,
            commands::take_startup_scan,
            commands::get_settings,
            commands::set_settings,
        ])
        .run(tauri::generate_context!());
    if let Err(e) = result {
        tracing::error!("tauri failed: {e}");
        eprintln!("error while running ByteForest: {e}");
        std::process::exit(1);
    }
}
