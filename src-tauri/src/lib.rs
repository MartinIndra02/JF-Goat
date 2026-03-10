mod api;
mod commands;
mod db;
mod error;
mod state;

use rusqlite::Connection;
use std::fs;
use std::sync::{Mutex, RwLock};
use tauri::Manager;

use state::AppState;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_shell::init())
        .setup(|app| {
            let app_data_dir = app.path().app_data_dir()?;
            fs::create_dir_all(&app_data_dir)?;

            // Initialize SQLite
            let db_path = app_data_dir.join("jfgoat.db");
            let conn = Connection::open(&db_path)?;
            db::init_db(&conn)?;
            println!("Database initialized at: {:?}", db_path);

            // Create HTTP client
            let http_client = reqwest::Client::new();

            // Create and manage AppState
            let app_state = AppState {
                db: Mutex::new(conn),
                http_client,
                server_url: RwLock::new(None),
                user_id: RwLock::new(None),
                token: RwLock::new(None),
            };
            app.manage(app_state);

            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::connect_to_server,
            commands::login,
            commands::check_auth,
            commands::logout,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
