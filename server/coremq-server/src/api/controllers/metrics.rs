use axum::{
    extract::{State, WebSocketUpgrade, ws::{Message, WebSocket}},
    response::IntoResponse,
};
use chrono::Local;
use serde::Serialize;
use sysinfo::{Pid, PidExt, ProcessExt, System, SystemExt};
use tokio::sync::oneshot;
use tokio::time::{Duration, interval};

use crate::{api::api_state::ApiState, engine::AdminCommand, models::topic_info::TopicInfo};

#[derive(Serialize)]
struct MetricsFrame {
    timestamp: String,
    memory_mb: f64,
    cpu_percent: f32,
    client_count: usize,
    topics: Vec<TopicInfo>,
}

pub async fn ws_metrics(
    ws: WebSocketUpgrade,
    State(state): State<ApiState>,
) -> impl IntoResponse {
    ws.on_upgrade(move |socket| stream_metrics(socket, state))
}

async fn stream_metrics(mut socket: WebSocket, state: ApiState) {
    let pid = Pid::from_u32(std::process::id());
    let mut sys = System::new();
    let mut ticker = interval(Duration::from_secs(1));

    loop {
        ticker.tick().await;

        sys.refresh_process(pid);
        let (memory_mb, cpu_percent) = match sys.process(pid) {
            Some(p) => (p.memory() as f64 / 1024.0 / 1024.0, p.cpu_usage()),
            None => (0.0, 0.0),
        };

        let (reply_tx, reply_rx) = oneshot::channel();
        if state.engine.send(AdminCommand::GetStats(reply_tx)).is_err() {
            break;
        }
        let (client_count, topics) = match reply_rx.await {
            Ok(stats) => stats,
            Err(_) => break,
        };

        let frame = MetricsFrame {
            timestamp: Local::now().to_rfc3339(),
            memory_mb,
            cpu_percent,
            client_count,
            topics,
        };

        let json = match serde_json::to_string(&frame) {
            Ok(j) => j,
            Err(_) => break,
        };

        if socket.send(Message::Text(json.into())).await.is_err() {
            break;
        }
    }
}
