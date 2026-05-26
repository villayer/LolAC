// Prevents additional console window on Windows in release, DO NOT REMOVE!!
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod champ_select;
mod lcu;

use std::sync::{
    atomic::{AtomicBool, Ordering},
    Arc,
};

use serde::Serialize;
use tauri::Manager;

#[derive(Clone)]
struct AppState {
    auto_accept_enabled: Arc<AtomicBool>,
    connected: Arc<AtomicBool>,
}

#[derive(Serialize)]
struct StatusResponse {
    auto_accept_enabled: bool,
    connected: bool,
}

#[tauri::command]
fn set_auto_accept_enabled(state: tauri::State<'_, AppState>, enabled: bool) {
    state.auto_accept_enabled.store(enabled, Ordering::SeqCst);
}

#[tauri::command]
fn get_status(state: tauri::State<'_, AppState>) -> StatusResponse {
    StatusResponse {
        auto_accept_enabled: state.auto_accept_enabled.load(Ordering::SeqCst),
        connected: state.connected.load(Ordering::SeqCst),
    }
}

fn main() {
    let app_state = AppState {
        auto_accept_enabled: Arc::new(AtomicBool::new(true)),
        connected: Arc::new(AtomicBool::new(false)),
    };

    tauri::Builder::default()
        .plugin(tauri_plugin_shell::init())
        .manage(app_state.clone())
        .invoke_handler(tauri::generate_handler![set_auto_accept_enabled, get_status])
        .setup(move |app| {
            let handle = app.handle().clone();
            let shared = app.state::<AppState>().inner().clone();
            tauri::async_runtime::spawn(async move {
                lcu::run_lcu_loop(
                    handle,
                    shared.auto_accept_enabled.clone(),
                    shared.connected.clone(),
                )
                .await;
            });

            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
