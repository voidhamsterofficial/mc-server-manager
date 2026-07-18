mod backups;
mod commands;
mod console;
mod error;
mod events;
mod files;
mod fsutil;
mod installers;
mod java;
mod platform;
mod playerdata;
mod plugins;
mod portforward;
mod process;
mod properties;
mod roster;
mod scheduler;
mod servers;
mod service;
mod settings;
mod state;
mod stats;

use tauri::Manager;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    // Route panics into the log file before the process aborts (the release
    // profile uses panic = "abort"), so a crash leaves a diagnostic trail
    // instead of vanishing silently for a user who launched the bundled app.
    let default_panic = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        log::error!("panic: {info}");
        default_panic(info);
    }));

    tauri::Builder::default()
        .plugin(
            // Rolling log file in the OS log dir, plus stdout during dev.
            tauri_plugin_log::Builder::new()
                .target(tauri_plugin_log::Target::new(
                    tauri_plugin_log::TargetKind::LogDir {
                        file_name: Some("blockparty".into()),
                    },
                ))
                .target(tauri_plugin_log::Target::new(
                    tauri_plugin_log::TargetKind::Stdout,
                ))
                .level(log::LevelFilter::Info)
                .build(),
        )
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
            commands::list_loader_versions,
            commands::create_server,
            commands::delete_server,
            commands::start_server,
            commands::stop_server,
            commands::kill_server,
            commands::send_server_command,
            commands::restart_server,
            commands::server_statuses,
            commands::server_players,
            commands::get_player_roster,
            commands::get_player_detail,
            commands::get_server_address,
            commands::open_port_forward,
            commands::close_port_forward,
            commands::port_forward_status,
            commands::detect_java,
            commands::open_logs_dir,
            commands::kill_all_java,
            commands::get_settings,
            commands::set_servers_base_dir,
            commands::preview_server_dir,
            commands::update_server,
            commands::set_server_icon,
            commands::get_server_icon,
            commands::remove_server_icon,
            commands::get_server_properties,
            commands::save_server_properties,
            commands::list_server_files,
            commands::read_server_file,
            commands::write_server_file,
            commands::delete_server_file,
            commands::list_plugins,
            commands::set_plugin_enabled,
            commands::delete_plugin,
            commands::search_plugins,
            commands::install_plugin,
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
