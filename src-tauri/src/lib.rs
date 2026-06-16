mod spacetime;

use tauri::Manager;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .setup(|app| {
            let profiles = spacetime::load_profiles(app.handle());
            app.manage(spacetime::AppState {
                profiles: std::sync::Mutex::new(profiles),
            });
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            spacetime::list_connections,
            spacetime::save_connection,
            spacetime::delete_connection,
            spacetime::test_connection,
            spacetime::get_schema,
            spacetime::query_table,
            spacetime::execute_sql,
            spacetime::run_function,
            spacetime::create_row,
            spacetime::update_row,
            spacetime::delete_row,
            spacetime::get_logs,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
