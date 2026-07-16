mod backups;
mod commands;
mod console;
mod error;
mod events;
mod installers;
mod java;
mod platform;
mod process;
mod properties;
mod scheduler;
mod servers;
mod service;
mod settings;
mod state;
mod stats;

use tauri::Manager;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_dialog::init())
        .setup(|app| {
            let app_state = state::AppState::initialize(app.handle())?;
            let running = std::sync::Arc::clone(&app_state.running);
            app.manage(app_state);

            stats::spawn_sampler(app.handle().clone(), running);
            scheduler::spawn_scheduler(app.handle().clone());
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::list_servers,
            commands::list_minecraft_versions,
            commands::create_server,
            commands::delete_server,
            commands::start_server,
            commands::stop_server,
            commands::kill_server,
            commands::send_server_command,
            commands::restart_server,
            commands::server_statuses,
            commands::server_players,
            commands::detect_java,
            commands::get_settings,
            commands::set_servers_base_dir,
            commands::preview_server_dir,
            commands::update_server,
            commands::get_server_properties,
            commands::save_server_properties,
            commands::create_backup,
            commands::list_backups,
            commands::restore_backup,
            commands::delete_backup,
            commands::list_tasks,
            commands::upsert_task,
            commands::delete_task,
            commands::run_task_now,
            commands::preview_next_run,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
