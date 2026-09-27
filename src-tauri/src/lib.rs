mod spacetime;

use tauri::Manager;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        // Logs go to stdout and to a rotating file in the OS log directory, so
        // failed requests can be diagnosed after the fact.
        .plugin(
            tauri_plugin_log::Builder::new()
                .level(log::LevelFilter::Info)
                .build(),
        )
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_dialog::init())
        .setup(|app| {
            let profiles = spacetime::load_profiles(app.handle());
            app.manage(spacetime::AppState::new(profiles));
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            spacetime::list_connections,
            spacetime::save_connection,
            spacetime::delete_connection,
            spacetime::get_cli_config,
            spacetime::test_connection,
            spacetime::get_schema,
            spacetime::query_table,
            spacetime::execute_sql,
            spacetime::run_function,
            spacetime::create_row,
            spacetime::update_row,
            spacetime::delete_row,
            spacetime::start_log_stream,
            spacetime::stop_log_stream,
            spacetime::export_rows,
            spacetime::export_table,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
