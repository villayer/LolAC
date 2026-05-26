use std::{
    env,
    fs,
    path::PathBuf,
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc,
    },
    time::{Duration, SystemTime, UNIX_EPOCH},
};

use base64::{engine::general_purpose, Engine as _};
use futures_util::{SinkExt, StreamExt};
use reqwest::Client;
use serde::Serialize;
use serde_json::Value;
use tauri::{AppHandle, Emitter};
use tokio::time::sleep;
use tokio_tungstenite::{
    connect_async_tls_with_config,
    tungstenite::{http::Request, Message},
    Connector,
};

#[derive(Clone, Serialize)]
pub struct LcuStatus {
    pub connected: bool,
    pub message: String,
}

#[derive(Clone, Serialize)]
pub struct LcuLog {
    pub timestamp: String,
    pub text: String,
    pub level: String,
}

#[derive(Clone, Serialize)]
pub struct AcceptFired {
    pub success: bool,
}

#[derive(Clone)]
struct LockfileData {
    port: u16,
    password: String,
}

pub async fn run_lcu_loop(
    app_handle: AppHandle,
    auto_accept_enabled: Arc<AtomicBool>,
    connected: Arc<AtomicBool>,
) {
    loop {
        connected.store(false, Ordering::SeqCst);
        emit_status(&app_handle, false, "Waiting for League client...");

        let lockfile = wait_for_lockfile(&app_handle).await;
        connected.store(true, Ordering::SeqCst);
        emit_status(&app_handle, true, "League client detected.");
        emit_log(&app_handle, "League client detected.", "info");

        if let Err(error) = connect_and_watch(
            app_handle.clone(),
            auto_accept_enabled.clone(),
            connected.clone(),
            lockfile,
        )
        .await
        {
            emit_log(
                &app_handle,
                &format!("Disconnected from LCU: {error}"),
                "error",
            );
        }

        connected.store(false, Ordering::SeqCst);
        emit_status(&app_handle, false, "Disconnected. Reconnecting in 5s...");
        emit_log(&app_handle, "Disconnected. Reconnecting in 5s...", "error");
        sleep(Duration::from_secs(5)).await;
    }
}

async fn wait_for_lockfile(app_handle: &AppHandle) -> LockfileData {
    loop {
        for path in lockfile_paths() {
            if !path.exists() {
                continue;
            }

            match fs::read_to_string(&path) {
                Ok(content) => {
                    if let Some(lockfile) = parse_lockfile(content.trim()) {
                        emit_log(
                            app_handle,
                            &format!("Using lockfile: {}", path.display()),
                            "info",
                        );
                        return lockfile;
                    }
                }
                Err(error) => {
                    emit_log(
                        app_handle,
                        &format!("Failed reading lockfile at {}: {error}", path.display()),
                        "error",
                    );
                }
            }
        }

        sleep(Duration::from_secs(2)).await;
    }
}

fn lockfile_paths() -> Vec<PathBuf> {
    let mut paths = vec![PathBuf::from(r"C:\Riot Games\League of Legends\lockfile")];

    if let Ok(local_app_data) = env::var("LOCALAPPDATA") {
        paths.push(
            PathBuf::from(local_app_data).join(r"Riot Games\League of Legends\lockfile"),
        );
    }

    if let Ok(program_files) = env::var("ProgramFiles") {
        paths.push(PathBuf::from(program_files).join(r"Riot Games\League of Legends\lockfile"));
    }

    if let Ok(program_files_x86) = env::var("ProgramFiles(x86)") {
        paths.push(
            PathBuf::from(program_files_x86).join(r"Riot Games\League of Legends\lockfile"),
        );
    }

    paths
}

fn parse_lockfile(raw: &str) -> Option<LockfileData> {
    let parts: Vec<&str> = raw.split(':').collect();
    if parts.len() != 5 {
        return None;
    }

    let port = parts.get(2)?.parse::<u16>().ok()?;
    let password = parts.get(3)?.to_string();

    Some(LockfileData { port, password })
}

async fn connect_and_watch(
    app_handle: AppHandle,
    auto_accept_enabled: Arc<AtomicBool>,
    _connected: Arc<AtomicBool>,
    lockfile: LockfileData,
) -> Result<(), String> {
    let auth_token = basic_auth_token(&lockfile.password);
    let ws_url = format!("wss://127.0.0.1:{}/", lockfile.port);

    let request = Request::builder()
        .uri(&ws_url)
        .header("Authorization", format!("Basic {auth_token}"))
        .body(())
        .map_err(|error| format!("WS request build failed: {error}"))?;

    let tls = native_tls::TlsConnector::builder()
        .danger_accept_invalid_certs(true)
        .build()
        .map_err(|error| format!("TLS setup failed: {error}"))?;

    let connector = Connector::NativeTls(tls.into());

    emit_log(&app_handle, "Connecting to LCU WebSocket...", "info");

    let (ws_stream, _) = connect_async_tls_with_config(request, None, false, Some(connector))
        .await
        .map_err(|error| format!("WebSocket error: {error}"))?;

    emit_log(&app_handle, "Connected to LCU WebSocket.", "success");

    let (mut writer, mut reader) = ws_stream.split();

    let subscribe = serde_json::json!([5, "OnJsonApiEvent_lol-gameflow_v1_gameflow-phase"]);
    writer
        .send(Message::Text(subscribe.to_string()))
        .await
        .map_err(|error| format!("Failed subscribing to gameflow events: {error}"))?;

    emit_log(
        &app_handle,
        "Subscribed to gameflow phase events. Waiting for queue pop...",
        "info",
    );

    let http_client = build_http_client().map_err(|error| format!("HTTP client failed: {error}"))?;
    let accept_pending = Arc::new(AtomicBool::new(false));

    while let Some(next_message) = reader.next().await {
        let message = next_message.map_err(|error| format!("WebSocket read error: {error}"))?;

        match message {
            Message::Text(payload) => {
                handle_ws_payload(
                    app_handle.clone(),
                    auto_accept_enabled.clone(),
                    accept_pending.clone(),
                    http_client.clone(),
                    lockfile.clone(),
                    auth_token.clone(),
                    payload,
                )
                .await;
            }
            Message::Close(_) => break,
            _ => {}
        }
    }

    accept_pending.store(false, Ordering::SeqCst);
    Err("WebSocket closed".to_string())
}

async fn handle_ws_payload(
    app_handle: AppHandle,
    auto_accept_enabled: Arc<AtomicBool>,
    accept_pending: Arc<AtomicBool>,
    client: Client,
    lockfile: LockfileData,
    auth_token: String,
    payload: String,
) {
    let parsed: Value = match serde_json::from_str(&payload) {
        Ok(value) => value,
        Err(_) => return,
    };

    let Some(opcode) = parsed.get(0).and_then(|value| value.as_i64()) else {
        return;
    };

    if opcode != 8 {
        return;
    }

    let phase = parsed
        .get(2)
        .and_then(|event_data| event_data.get("data"))
        .and_then(|value| value.as_str());

    if phase != Some("ReadyCheck") {
        return;
    }

    if !auto_accept_enabled.load(Ordering::SeqCst) {
        emit_log(&app_handle, "Auto-accept is paused", "info");
        return;
    }

    if accept_pending
        .compare_exchange(false, true, Ordering::SeqCst, Ordering::SeqCst)
        .is_err()
    {
        return;
    }

    emit_log(
        &app_handle,
        "Queue pop detected! Waiting 2s before accepting...",
        "info",
    );

    let task_app = app_handle.clone();
    tokio::spawn(async move {
        sleep(Duration::from_secs(2)).await;

        let phase = get_current_phase(&client, &lockfile, &auth_token).await;
        if phase.as_deref() != Some("ReadyCheck") {
            let changed_to = phase.unwrap_or_else(|| "Unknown".to_string());
            emit_log(
                &task_app,
                &format!("Ready check no longer active (phase: {changed_to})."),
                "info",
            );
            accept_pending.store(false, Ordering::SeqCst);
            return;
        }

        match accept_ready_check(&client, &lockfile, &auth_token).await {
            Ok(true) => {
                emit_log(&task_app, "Match accepted!", "success");
                let _ = task_app.emit("accept-fired", AcceptFired { success: true });
            }
            Ok(false) => {
                emit_log(&task_app, "Failed to accept match.", "error");
                let _ = task_app.emit("accept-fired", AcceptFired { success: false });
            }
            Err(error) => {
                emit_log(&task_app, &format!("Accept request failed: {error}"), "error");
                let _ = task_app.emit("accept-fired", AcceptFired { success: false });
            }
        }

        accept_pending.store(false, Ordering::SeqCst);
    });
}

fn build_http_client() -> Result<Client, reqwest::Error> {
    Client::builder().danger_accept_invalid_certs(true).build()
}

fn basic_auth_token(password: &str) -> String {
    let token = format!("riot:{password}");
    general_purpose::STANDARD.encode(token.as_bytes())
}

async fn get_current_phase(client: &Client, lockfile: &LockfileData, auth_token: &str) -> Option<String> {
    let url = format!(
        "https://127.0.0.1:{}/lol-gameflow/v1/gameflow-phase",
        lockfile.port
    );

    let response = client
        .get(url)
        .header("Authorization", format!("Basic {auth_token}"))
        .send()
        .await
        .ok()?;

    if !response.status().is_success() {
        return None;
    }

    let body = response.text().await.ok()?;
    serde_json::from_str::<String>(&body).ok()
}

async fn accept_ready_check(
    client: &Client,
    lockfile: &LockfileData,
    auth_token: &str,
) -> Result<bool, reqwest::Error> {
    let url = format!(
        "https://127.0.0.1:{}/lol-matchmaking/v1/ready-check/accept",
        lockfile.port
    );

    let response = client
        .post(url)
        .header("Authorization", format!("Basic {auth_token}"))
        .send()
        .await?;

    Ok(response.status().is_success())
}

fn emit_status(app_handle: &AppHandle, connected: bool, message: &str) {
    let _ = app_handle.emit(
        "lcu-status",
        LcuStatus {
            connected,
            message: message.to_string(),
        },
    );
}

fn emit_log(app_handle: &AppHandle, text: &str, level: &str) {
    let _ = app_handle.emit(
        "lcu-log",
        LcuLog {
            timestamp: timestamp_string(),
            text: text.to_string(),
            level: level.to_string(),
        },
    );
}

fn timestamp_string() -> String {
    match SystemTime::now().duration_since(UNIX_EPOCH) {
        Ok(duration) => duration.as_secs().to_string(),
        Err(_) => "0".to_string(),
    }
}
