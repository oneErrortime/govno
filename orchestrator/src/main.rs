// ╔══════════════════════════════════════════════════════════════════════════╗
// ║  💩  ГОВНО-ОРКЕСТРАТОР  v0.3.0                                         ║
// ║                                                                          ║
// ║  Topology:                                                               ║
// ║    Workers ──ws /producer──► Orchestrator ──broadcast──► Consumers      ║
// ║    Each worker is a separate microservice that registers itself here.    ║
// ║    Workers can be added/removed at runtime — registry is dynamic.       ║
// ║                                                                          ║
// ║  Endpoints:                                                              ║
// ║    /producer      WS  — workers connect here                            ║
// ║    /ws            WS  — consumers (WASM clients) connect here           ║
// ║    /api/services  GET — live registry of connected producers             ║
// ║    /metrics       GET — Prometheus text                                  ║
// ║    /metrics/json  GET — JSON metrics                                     ║
// ║    /health        GET — liveness probe                                   ║
// ╚══════════════════════════════════════════════════════════════════════════╝

use std::{
    collections::{HashMap, HashSet},
    env,
    sync::{
        atomic::{AtomicI64, AtomicU64, Ordering},
        Arc,
    },
    time::{Duration, SystemTime, UNIX_EPOCH},
};

use axum::{
    extract::{
        ws::{Message, WebSocket, WebSocketUpgrade},
        State,
    },
    response::IntoResponse,
    routing::get,
    Router,
};
use rand::Rng;
use serde_json;
use tokio::sync::{broadcast, RwLock};
use tower_http::cors::CorsLayer;
use tracing::{info, warn};

use proto::{
    ClientCmd, OrchestratorMsg, Priority, ProducerMsg, ServerMsg, ServiceInfo, ShitMessage,
};

// ── Constants ─────────────────────────────────────────────────────────────────

const BROADCAST_CAP: usize  = 512;
const AUTH_TIMEOUT_SECS: u64 = 10;

// ── Metrics ───────────────────────────────────────────────────────────────────

#[derive(Default)]
struct Metrics {
    messages_total:     AtomicU64,
    bytes_sent:         AtomicU64,
    connected_consumers: AtomicI64,
    connected_producers: AtomicI64,
    auth_success:       AtomicU64,
    auth_failures:      AtomicU64,
    liquid_total:       AtomicU64,
    solid_total:        AtomicU64,
    gas_total:          AtomicU64,
    critical_total:     AtomicU64,
}

impl Metrics {
    fn inc_msg(&self, msg: &ShitMessage) {
        self.messages_total.fetch_add(1, Ordering::Relaxed);
        self.bytes_sent.fetch_add(msg.payload.len() as u64, Ordering::Relaxed);
        use proto::ShitTopic::*;
        match msg.topic {
            Liquid   => { self.liquid_total.fetch_add(1, Ordering::Relaxed); }
            Solid    => { self.solid_total.fetch_add(1, Ordering::Relaxed); }
            Gas      => { self.gas_total.fetch_add(1, Ordering::Relaxed); }
            Critical => { self.critical_total.fetch_add(1, Ordering::Relaxed); }
        }
    }

    fn prometheus(&self) -> String {
        let rows: &[(&str, &str, &str, i64)] = &[
            ("shit_messages_total",      "Total shit orchestrated",          "counter", self.messages_total.load(Ordering::Relaxed) as i64),
            ("shit_bytes_sent",           "Total payload bytes broadcast",    "counter", self.bytes_sent.load(Ordering::Relaxed) as i64),
            ("shit_auth_success",         "Successful consumer auths",        "counter", self.auth_success.load(Ordering::Relaxed) as i64),
            ("shit_auth_failures",        "Failed consumer auth attempts",    "counter", self.auth_failures.load(Ordering::Relaxed) as i64),
            ("shit_liquid_total",         "Liquid shit produced",             "counter", self.liquid_total.load(Ordering::Relaxed) as i64),
            ("shit_solid_total",          "Solid shit produced",              "counter", self.solid_total.load(Ordering::Relaxed) as i64),
            ("shit_gas_total",            "Gas shit produced",                "counter", self.gas_total.load(Ordering::Relaxed) as i64),
            ("shit_critical_total",       "Critical incidents",               "counter", self.critical_total.load(Ordering::Relaxed) as i64),
            ("connected_consumers",       "Active consumer WS connections",   "gauge",   self.connected_consumers.load(Ordering::Relaxed)),
            ("connected_producers",       "Active producer WS connections",   "gauge",   self.connected_producers.load(Ordering::Relaxed)),
        ];
        let mut s = String::new();
        for (name, help, kind, val) in rows {
            s.push_str(&format!(
                "# HELP {name} {help}\n# TYPE {name} {kind}\n{name} {val}\n\n"
            ));
        }
        s
    }

    fn to_json(&self) -> serde_json::Value {
        serde_json::json!({
            "shit_messages_total":   self.messages_total.load(Ordering::Relaxed),
            "shit_bytes_sent":       self.bytes_sent.load(Ordering::Relaxed),
            "connected_consumers":   self.connected_consumers.load(Ordering::Relaxed),
            "connected_producers":   self.connected_producers.load(Ordering::Relaxed),
            "shit_auth_success":     self.auth_success.load(Ordering::Relaxed),
            "shit_auth_failures":    self.auth_failures.load(Ordering::Relaxed),
            "shit_liquid_total":     self.liquid_total.load(Ordering::Relaxed),
            "shit_solid_total":      self.solid_total.load(Ordering::Relaxed),
            "shit_gas_total":        self.gas_total.load(Ordering::Relaxed),
            "shit_critical_total":   self.critical_total.load(Ordering::Relaxed),
        })
    }
}

// ── App state ─────────────────────────────────────────────────────────────────

#[derive(Clone)]
struct AppState {
    broadcast_tx: broadcast::Sender<ShitMessage>,
    services:     Arc<RwLock<HashMap<String, ServiceInfo>>>,
    metrics:      Arc<Metrics>,
    auth_token:   Arc<String>,
    seq:          Arc<AtomicU64>,
}

// ── Helpers ───────────────────────────────────────────────────────────────────

fn now_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as u64
}

fn gen_id() -> String {
    format!("💩-{:08x}", rand::thread_rng().gen::<u32>())
}

async fn send_json(socket: &mut WebSocket, msg: &impl serde::Serialize) {
    if let Ok(json) = serde_json::to_string(msg) {
        let _ = socket.send(Message::Text(json)).await;
    }
}

// ── Producer WS handler ────────────────────────────────────────────────────────
// Workers connect here. Protocol:
//   1. Worker sends ProducerMsg::Hello
//   2. Orchestrator replies OrchestratorMsg::Welcome
//   3. Worker sends ProducerMsg::Emit* freely
//   4. On disconnect: deregistered automatically

async fn producer_ws_handler(
    ws:           WebSocketUpgrade,
    State(state): State<AppState>,
) -> impl IntoResponse {
    ws.on_upgrade(|socket| handle_producer(socket, state))
}

async fn handle_producer(mut socket: WebSocket, state: AppState) {
    state.metrics.connected_producers.fetch_add(1, Ordering::Relaxed);

    // Step 1: Expect Hello within 15s
    let hello = tokio::time::timeout(
        Duration::from_secs(15),
        socket.recv(),
    ).await;

    let hello_msg = match hello {
        Ok(Some(Ok(Message::Text(t)))) => serde_json::from_str::<ProducerMsg>(&t).ok(),
        _ => None,
    };

    let ProducerMsg::Hello { service_id, topic, version, interval_ms, description } =
        hello_msg.unwrap_or(ProducerMsg::Bye)
    else {
        let _ = send_json(&mut socket, &OrchestratorMsg::Reject {
            reason: "first message must be Hello".into(),
        }).await;
        state.metrics.connected_producers.fetch_sub(1, Ordering::Relaxed);
        return;
    };

    let assigned_id = gen_id();

    // Register service
    state.services.write().await.insert(assigned_id.clone(), ServiceInfo {
        assigned_id:     assigned_id.clone(),
        service_id:      service_id.clone(),
        topic:           topic.clone(),
        version:         version.clone(),
        interval_ms,
        description:     description.clone(),
        connected_at_ms: now_ms(),
        messages_sent:   0,
    });

    send_json(&mut socket, &OrchestratorMsg::Welcome {
        assigned_id: assigned_id.clone(),
    }).await;

    info!(
        "🟢 Producer registered: {} ({}) topic={} v={}",
        assigned_id, service_id, topic.label(), version
    );

    // Step 2: Receive Emit messages
    while let Some(Ok(msg)) = socket.recv().await {
        let text = match msg {
            Message::Text(t)     => t,
            Message::Close(_)    => break,
            Message::Ping(data)  => { let _ = socket.send(Message::Pong(data)).await; continue; }
            _                    => continue,
        };

        match serde_json::from_str::<ProducerMsg>(&text) {
            Ok(ProducerMsg::Emit { payload, priority, tags }) => {
                let seq = state.seq.fetch_add(1, Ordering::Relaxed);

                // Log critical loudly
                if priority == Priority::Critical {
                    warn!("🚨 [seq={seq}] CRITICAL from {service_id}: {payload}");
                }

                let shit = ShitMessage {
                    seq,
                    topic:       topic.clone(),
                    payload,
                    priority,
                    tags,
                    producer_id: assigned_id.clone(),
                    service_id:  service_id.clone(),
                    ts_ms:       now_ms(),
                };

                state.metrics.inc_msg(&shit);

                // Ack back to producer
                let ack = OrchestratorMsg::Ack { seq };
                let _ = send_json(&mut socket, &ack).await;

                // Broadcast to all consumers
                let _ = state.broadcast_tx.send(shit);

                // Update message count in registry
                if let Some(info) = state.services.write().await.get_mut(&assigned_id) {
                    info.messages_sent += 1;
                }
            }
            Ok(ProducerMsg::Bye) => {
                info!("👋 Producer {} sent Bye", assigned_id);
                break;
            }
            Ok(ProducerMsg::Hello { .. }) => {
                // Already registered, ignore
            }
            Err(e) => {
                warn!("⚠️ Producer {} sent bad JSON: {}", assigned_id, e);
            }
        }
    }

    // Deregister
    state.services.write().await.remove(&assigned_id);
    state.metrics.connected_producers.fetch_sub(1, Ordering::Relaxed);
    info!("🔴 Producer deregistered: {} ({})", assigned_id, service_id);
}

// ── Consumer WS handler ────────────────────────────────────────────────────────

async fn consumer_ws_handler(
    ws:           WebSocketUpgrade,
    State(state): State<AppState>,
) -> impl IntoResponse {
    ws.on_upgrade(|socket| handle_consumer(socket, state))
}

async fn handle_consumer(mut socket: WebSocket, state: AppState) {
    state.metrics.connected_consumers.fetch_add(1, Ordering::Relaxed);

    // Auth gate
    let session_id = match authenticate(&mut socket, &state.auth_token, &state.metrics).await {
        Some(sid) => sid,
        None => {
            state.metrics.connected_consumers.fetch_sub(1, Ordering::Relaxed);
            return;
        }
    };

    send_json(&mut socket, &ServerMsg::Welcome {
        msg: "Выбери топики: жидкое / твёрдое / газообразное / критическое".into(),
    }).await;

    let mut bcast_rx = state.broadcast_tx.subscribe();
    let mut subs: HashSet<proto::ShitTopic> = HashSet::new();

    loop {
        tokio::select! {
            client_msg = socket.recv() => {
                match client_msg {
                    Some(Ok(Message::Text(text))) => {
                        match serde_json::from_str::<ClientCmd>(&text) {
                            Ok(cmd) => handle_client_cmd(cmd, &mut socket, &mut subs, &state).await,
                            Err(e)  => {
                                send_json(&mut socket, &ServerMsg::Error {
                                    msg: format!("bad json: {e}"),
                                }).await;
                            }
                        }
                    }
                    Some(Ok(Message::Ping(data))) => {
                        let _ = socket.send(Message::Pong(data)).await;
                    }
                    Some(Ok(Message::Close(_))) | None => {
                        info!("🚪 Consumer {} disconnected", session_id);
                        break;
                    }
                    Some(Err(e)) => {
                        warn!("💥 Consumer WS error: {}", e);
                        break;
                    }
                    _ => {}
                }
            }

            bcast = bcast_rx.recv() => {
                match bcast {
                    Ok(msg) => {
                        let deliver = subs.contains(&msg.topic)
                            || (subs.is_empty() && msg.priority == Priority::Critical);
                        if deliver {
                            send_json(&mut socket, &ServerMsg::Shit(msg)).await;
                        }
                    }
                    Err(broadcast::error::RecvError::Lagged(n)) => {
                        warn!("⚡ Consumer {} lagged by {} messages", session_id, n);
                        send_json(&mut socket, &ServerMsg::Error {
                            msg: format!("lagged: missed {n} messages"),
                        }).await;
                    }
                    Err(broadcast::error::RecvError::Closed) => break,
                }
            }
        }
    }

    state.metrics.connected_consumers.fetch_sub(1, Ordering::Relaxed);
}

async fn handle_client_cmd(
    cmd:    ClientCmd,
    socket: &mut WebSocket,
    subs:   &mut HashSet<proto::ShitTopic>,
    state:  &AppState,
) {
    match cmd {
        ClientCmd::Auth { .. } => {
            send_json(socket, &ServerMsg::Error {
                msg: "Already authenticated".into(),
            }).await;
        }
        ClientCmd::Subscribe { topic } => {
            let label = topic.label().to_string();
            info!("📬 Subscribe: {label}");
            subs.insert(topic);
            send_json(socket, &ServerMsg::Subscribed { topic: label }).await;
        }
        ClientCmd::Unsubscribe { topic } => {
            let label = topic.label().to_string();
            subs.remove(&topic);
            send_json(socket, &ServerMsg::Unsubscribed { topic: label }).await;
        }
        ClientCmd::UnsubscribeAll => {
            subs.clear();
            send_json(socket, &ServerMsg::UnsubscribedAll).await;
        }
        ClientCmd::Echo { text } => {
            send_json(socket, &ServerMsg::Echo {
                payload: format!("💩 ЭХО: {text}"),
            }).await;
        }
        ClientCmd::Ping => {
            // Also attach live service list on ping
            let services = state.services.read().await.values().cloned().collect();
            send_json(socket, &ServerMsg::ServiceList { services }).await;
            send_json(socket, &ServerMsg::Pong).await;
        }
    }
}

// ── Auth ──────────────────────────────────────────────────────────────────────

async fn authenticate(
    socket:     &mut WebSocket,
    auth_token: &str,
    metrics:    &Metrics,
) -> Option<String> {
    send_json(socket, &ServerMsg::AuthRequired {
        msg: "Токен или проваливай. 10 секунд.".into(),
    }).await;

    let result = tokio::time::timeout(
        Duration::from_secs(AUTH_TIMEOUT_SECS),
        socket.recv(),
    ).await;

    let raw = match result {
        Ok(Some(Ok(Message::Text(t)))) => t,
        _ => {
            send_json(socket, &ServerMsg::Unauthorized {
                msg: "💨 ПУУК! Таймаут — пошёл отсюда.".into(),
            }).await;
            metrics.auth_failures.fetch_add(1, Ordering::Relaxed);
            return None;
        }
    };

    match serde_json::from_str::<ClientCmd>(&raw) {
        Ok(ClientCmd::Auth { token }) if token == auth_token => {
            let sid = gen_id();
            send_json(socket, &ServerMsg::Authorized {
                puk:        "💨 ПУУК! Добро пожаловать в говно-систему.".into(),
                session_id: sid.clone(),
            }).await;
            metrics.auth_success.fetch_add(1, Ordering::Relaxed);
            info!("✅ Auth OK, session={sid}");
            Some(sid)
        }
        _ => {
            send_json(socket, &ServerMsg::Unauthorized {
                msg: "💨 ПУУК! Неверный токен. Иди отсюда.".into(),
            }).await;
            metrics.auth_failures.fetch_add(1, Ordering::Relaxed);
            warn!("❌ Auth FAIL");
            None
        }
    }
}

// ── HTTP handlers ─────────────────────────────────────────────────────────────

async fn services_handler(State(state): State<AppState>) -> impl IntoResponse {
    let services: Vec<ServiceInfo> = state.services.read().await.values().cloned().collect();
    axum::Json(services)
}

async fn metrics_handler(State(state): State<AppState>) -> impl IntoResponse {
    (
        [(axum::http::header::CONTENT_TYPE, "text/plain; version=0.0.4")],
        state.metrics.prometheus(),
    )
}

async fn metrics_json_handler(State(state): State<AppState>) -> impl IntoResponse {
    axum::Json(state.metrics.to_json())
}

// ── main ──────────────────────────────────────────────────────────────────────

#[tokio::main]
async fn main() {
    tracing_subscriber::fmt()
        .with_env_filter(
            std::env::var("RUST_LOG")
                .unwrap_or_else(|_| "govno_orchestrator=debug,tower_http=info".into()),
        )
        .init();

    let auth_token = Arc::new(
        env::var("GOVNO_TOKEN").unwrap_or_else(|_| "говно".to_string())
    );
    info!("🔑 Auth token configured (len={})", auth_token.len());

    let (broadcast_tx, _) = broadcast::channel::<ShitMessage>(BROADCAST_CAP);
    let services  = Arc::new(RwLock::new(HashMap::new()));
    let metrics   = Arc::new(Metrics::default());
    let seq       = Arc::new(AtomicU64::new(1));

    let state = AppState { broadcast_tx, services, metrics, auth_token, seq };

    let app = Router::new()
        .route("/producer",     get(producer_ws_handler))
        .route("/ws",           get(consumer_ws_handler))
        .route("/api/services", get(services_handler))
        .route("/metrics",      get(metrics_handler))
        .route("/metrics/json", get(metrics_json_handler))
        .route("/health",       get(|| async { "💩 живой" }))
        .layer(CorsLayer::permissive())
        .with_state(state);

    let addr = env::var("LISTEN_ADDR").unwrap_or_else(|_| "0.0.0.0:3000".into());
    info!("╔══════════════════════════════════════════════════╗");
    info!("║  💩 ГОВНО-ОРКЕСТРАТОР v0.3.0                     ║");
    info!("║  /producer     ← workers connect here            ║");
    info!("║  /ws           ← consumers connect here          ║");
    info!("║  /api/services ← live producer registry          ║");
    info!("║  /metrics      ← Prometheus                      ║");
    info!("║  listening on {}                      ║", addr);
    info!("╚══════════════════════════════════════════════════╝");

    let listener = tokio::net::TcpListener::bind(&addr).await
        .unwrap_or_else(|e| panic!("Cannot bind {addr}: {e}"));
    axum::serve(listener, app).await.unwrap();
}
