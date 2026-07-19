mod addons;
mod commands;
mod error;
mod events;
mod installers;
mod java;
mod platform;
mod players;
mod portforward;
mod process;
mod servers;
mod storage;

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
            let app_state = servers::state::AppState::initialize(app.handle())?;
            let running = std::sync::Arc::clone(&app_state.running);
            app.manage(app_state);

            process::stats::spawn_sampler(app.handle().clone(), running);
            servers::scheduler::spawn_scheduler(app.handle().clone());

            // Clean up any server processes a previous crash left orphaned.
            let orphan_sweep_handle = app.handle().clone();
            tauri::async_runtime::spawn(async move {
                process::reclaim_all_orphans(orphan_sweep_handle).await;
            });
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::list_servers,
            commands::list_loader_versions,
            commands::create_server,
            commands::import_server,
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
            commands::get_storage_location,
            commands::set_storage_location,
            commands::reset_storage_location,
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
            commands::check_plugin_updates,
            commands::update_plugin,
            commands::list_mods,
            commands::set_mod_enabled,
            commands::delete_mod,
            commands::search_mods,
            commands::install_mod,
            commands::check_mod_updates,
            commands::update_mod,
            commands::get_curseforge_api_key,
            commands::set_curseforge_api_key,
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
        .build(tauri::generate_context!())
        .expect("error while running tauri application")
        .run(|app_handle, event| {
            // `code` is `Some` only when we requested the exit ourselves (the
            // `app_handle.exit(0)` below); skip re-entering cleanup for that one.
            if let tauri::RunEvent::ExitRequested { api, code: None, .. } = event {
                api.prevent_exit();
                let app_handle = app_handle.clone();
                tauri::async_runtime::spawn(async move {
                    commands::close_all_port_forwards(&app_handle).await;
                    app_handle.exit(0);
                });
            }
        });
}
