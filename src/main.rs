mod bot;

use axum::{
    extract::{
        ws::{Message, WebSocket, WebSocketUpgrade},
        State,
    },
    response::{Html, IntoResponse, Json},
    routing::{get, post},
    Router,
};
use bot::BotConfig;
use serde_json::json;
use sf_api::command::Flag;
use std::sync::Arc;
use strum::IntoEnumIterator;
use tokio::sync::{broadcast, watch, Mutex};

const CONFIG_FILE: &str = "config.json";

struct AppState {
    log_tx: broadcast::Sender<String>,
    status: Mutex<String>,
    stop_tx: Mutex<Option<watch::Sender<bool>>>,
}

#[tokio::main]
async fn main() {
    let (log_tx, _) = broadcast::channel::<String>(1000);

    let state = Arc::new(AppState {
        log_tx,
        status: Mutex::new("idle".to_string()),
        stop_tx: Mutex::new(None),
    });

    let app = Router::new()
        .route("/", get(index_handler))
        .route("/api/start", post(start_handler))
        .route("/api/stop", post(stop_handler))
        .route("/api/status", get(status_handler))
        .route("/api/flags", get(flags_handler))
        .route("/api/history", get(history_handler))
        .route("/api/config", get(get_config_handler).post(save_config_handler))
        .route("/ws", get(ws_handler))
        .with_state(state);

    println!("SFBot Web UI running at http://localhost:3000");

    let listener = tokio::net::TcpListener::bind("0.0.0.0:3000")
        .await
        .expect("Failed to bind to port 3000");
    axum::serve(listener, app)
        .await
        .expect("Server error");
}

async fn index_handler() -> impl IntoResponse {
    Html(include_str!("static/index.html"))
}

async fn start_handler(
    State(state): State<Arc<AppState>>,
    Json(config): Json<BotConfig>,
) -> impl IntoResponse {
    let mut status = state.status.lock().await;
    if *status != "idle" {
        return Json(json!({"ok": false, "error": "Bot is already running"}));
    }
    *status = "running".to_string();
    drop(status);

    let (stop_tx, stop_rx) = watch::channel(false);
    *state.stop_tx.lock().await = Some(stop_tx);

    let log_tx = state.log_tx.clone();
    let state_clone = state.clone();

    tokio::spawn(async move {
        bot::run_scan(config, log_tx, stop_rx).await;
        *state_clone.status.lock().await = "idle".to_string();
        *state_clone.stop_tx.lock().await = None;
    });

    Json(json!({"ok": true}))
}

async fn stop_handler(State(state): State<Arc<AppState>>) -> impl IntoResponse {
    let status = state.status.lock().await;
    if *status != "running" {
        return Json(json!({"ok": false, "error": "Bot is not running"}));
    }
    drop(status);

    if let Some(tx) = state.stop_tx.lock().await.as_ref() {
        let _ = tx.send(true);
    }
    *state.status.lock().await = "stopping".to_string();

    Json(json!({"ok": true}))
}

async fn status_handler(State(state): State<Arc<AppState>>) -> impl IntoResponse {
    let status = state.status.lock().await.clone();
    Json(json!({"status": status}))
}

async fn flags_handler() -> impl IntoResponse {
    let flags: Vec<String> = Flag::iter().map(|f| format!("{:?}", f)).collect();
    Json(flags)
}

async fn history_handler() -> impl IntoResponse {
    Json(bot::load_history_list())
}

async fn get_config_handler() -> impl IntoResponse {
    match std::fs::read_to_string(CONFIG_FILE) {
        Ok(contents) => Json(serde_json::from_str::<serde_json::Value>(&contents).unwrap_or(json!({}))),
        Err(_) => Json(json!({})),
    }
}

async fn save_config_handler(Json(body): Json<serde_json::Value>) -> impl IntoResponse {
    match std::fs::write(CONFIG_FILE, serde_json::to_string_pretty(&body).unwrap_or_default()) {
        Ok(_) => Json(json!({"ok": true})),
        Err(e) => Json(json!({"ok": false, "error": format!("Failed to save config: {}", e)})),
    }
}

async fn ws_handler(
    ws: WebSocketUpgrade,
    State(state): State<Arc<AppState>>,
) -> impl IntoResponse {
    ws.on_upgrade(move |socket| handle_ws(socket, state))
}

async fn handle_ws(mut socket: WebSocket, state: Arc<AppState>) {
    let mut rx = state.log_tx.subscribe();
    loop {
        match rx.recv().await {
            Ok(msg) => {
                if socket.send(Message::Text(msg.into())).await.is_err() {
                    break;
                }
            }
            Err(broadcast::error::RecvError::Lagged(_)) => continue,
            Err(broadcast::error::RecvError::Closed) => break,
        }
    }
}
