// ╔══════════════════════════════════════════════════════════════════════╗
// ║   💩  ГОВНО-СЕРВЕР  v0.3.0                                          ║
// ║                                                                      ║
// ║   CriticalShitWorker ──critical_tx──┐                               ║
// ║   LiquidShitWorker ─────────────────┤─ ShitOrchestrator             ║
// ║   SolidShitWorker ──────normal_tx───┤   (biased select)             ║
// ║   GasShitWorker ────────────────────┘    └─ broadcast               ║
// ║                                           └─ /metrics               ║
// ║   Auth gate: GOVNO_TOKEN env var. Wrong token = 💨 + close.         ║
// ╚══════════════════════════════════════════════════════════════════════╝

use std::{
    env,
    sync::{
        atomic::{AtomicI64, AtomicU64, Ordering},
        Arc,
    },
    time::Duration,
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
use serde::{Deserialize, Serialize};
use tokio::sync::{broadcast, mpsc};
use tower_http::cors::CorsLayer;
use tracing::{info, warn};

const BROADCAST_CAP: usize = 256;
const AUTH_TIMEOUT_SECS: u64 = 10;

// ── Топики ────────────────────────────────────────────────────────────────────

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
enum ShitTopic {
    #[serde(rename = "жидкое")]   Liquid,
    #[serde(rename = "твёрдое")]  Solid,
    #[serde(rename = "газообразное")] Gas,
    #[serde(rename = "критическое")] Critical,
}

impl ShitTopic {
    fn label(&self) -> &'static str {
        match self {
            Self::Liquid   => "жидкое",
            Self::Solid    => "твёрдое",
            Self::Gas      => "газообразное",
            Self::Critical => "критическое",
        }
    }
}

// ── Протокол ──────────────────────────────────────────────────────────────────

#[derive(Clone, Debug, Serialize)]
struct ShitMessage {
    topic:    ShitTopic,
    seq:      u64,
    payload:  String,
    priority: &'static str,
}

#[derive(Debug, Deserialize)]
#[serde(tag = "cmd", rename_all = "snake_case")]
enum ClientCmd {
    Auth        { token: String },
    Subscribe   { topic: ShitTopic },
    Unsubscribe,
    Echo        { text: String },
    Ping,
}

#[derive(Debug, Serialize)]
#[serde(tag = "type", rename_all = "snake_case")]
enum ServerMsg<'a> {
    AuthRequired { msg: &'a str },
    Authorized   { puk: &'a str, session_id: String },
    Unauthorized { msg: &'a str },
    Welcome      { msg: &'a str },
    Subscribed   { topic: &'a str },
    Unsubscribed,
    Shit         (ShitMessage),
    Echo         { payload: String },
    Pong,
    Error        { msg: String },
}

// ── Метрики ───────────────────────────────────────────────────────────────────

#[derive(Default)]
struct ShitMetrics {
    messages_total:     AtomicU64,
    bytes_sent:         AtomicU64,
    connected_assholes: AtomicI64,
    auth_success:       AtomicU64,
    auth_failures:      AtomicU64,
    liquid_total:       AtomicU64,
    solid_total:        AtomicU64,
    gas_total:          AtomicU64,
    critical_total:     AtomicU64,
}

impl ShitMetrics {
    fn inc_topic(&self, topic: &ShitTopic, bytes: u64) {
        self.messages_total.fetch_add(1, Ordering::Relaxed);
        self.bytes_sent.fetch_add(bytes, Ordering::Relaxed);
        match topic {
            ShitTopic::Liquid   => { self.liquid_total.fetch_add(1,   Ordering::Relaxed); }
            ShitTopic::Solid    => { self.solid_total.fetch_add(1,    Ordering::Relaxed); }
            ShitTopic::Gas      => { self.gas_total.fetch_add(1,      Ordering::Relaxed); }
            ShitTopic::Critical => { self.critical_total.fetch_add(1, Ordering::Relaxed); }
        }
    }

    fn prometheus(&self) -> String {
        let rows: &[(&str, &str, &str, u64)] = &[
            ("shit_messages_total",  "Total shit orchestrated",         "counter", self.messages_total.load(Ordering::Relaxed)),
            ("shit_bytes_sent",      "Total bytes broadcast",            "counter", self.bytes_sent.load(Ordering::Relaxed)),
            ("shit_auth_success",    "Successful authentications",       "counter", self.auth_success.load(Ordering::Relaxed)),
            ("shit_auth_failures",   "Failed authentication attempts",   "counter", self.auth_failures.load(Ordering::Relaxed)),
            ("shit_liquid_total",    "Liquid shit produced",             "counter", self.liquid_total.load(Ordering::Relaxed)),
            ("shit_solid_total",     "Solid shit produced",              "counter", self.solid_total.load(Ordering::Relaxed)),
            ("shit_gas_total",       "Gas shit produced",                "counter", self.gas_total.load(Ordering::Relaxed)),
            ("shit_critical_total",  "Critical incidents orchestrated",  "counter", self.critical_total.load(Ordering::Relaxed)),
        ];
        let mut s = String::new();
        for (name, help, t, val) in rows {
            s.push_str(&format!("# HELP {name} {help}\n# TYPE {name} {t}\n{name} {val}\n\n"));
        }
        let ca = self.connected_assholes.load(Ordering::Relaxed);
        s.push_str(&format!("# HELP connected_assholes Active WebSocket connections\n# TYPE connected_assholes gauge\nconnected_assholes {ca}\n\n"));
        s
    }

    fn to_json(&self) -> serde_json::Value {
        serde_json::json!({
            "shit_messages_total":  self.messages_total.load(Ordering::Relaxed),
            "shit_bytes_sent":      self.bytes_sent.load(Ordering::Relaxed),
            "connected_assholes":   self.connected_assholes.load(Ordering::Relaxed),
            "shit_auth_success":    self.auth_success.load(Ordering::Relaxed),
            "shit_auth_failures":   self.auth_failures.load(Ordering::Relaxed),
            "shit_liquid_total":    self.liquid_total.load(Ordering::Relaxed),
            "shit_solid_total":     self.solid_total.load(Ordering::Relaxed),
            "shit_gas_total":       self.gas_total.load(Ordering::Relaxed),
            "shit_critical_total":  self.critical_total.load(Ordering::Relaxed),
        })
    }
}

// ── AppState ──────────────────────────────────────────────────────────────────

#[derive(Clone)]
struct AppState {
    broadcast_tx: broadcast::Sender<ShitMessage>,
    metrics:      Arc<ShitMetrics>,
    auth_token:   Arc<String>,
}

// ── Воркеры ───────────────────────────────────────────────────────────────────

struct WorkerShit {
    topic:    ShitTopic,
    payload:  String,
    priority: &'static str,
}

macro_rules! shit_worker {
    ($fn_name:ident, $topic:expr, $prefix:literal, $priority:literal, $interval_ms:literal, $phrases:expr) => {
        async fn $fn_name(tx: mpsc::Sender<WorkerShit>) {
            let phrases: &[&str] = $phrases;
            let mut rng = rand::thread_rng();
            let mut i: u64 = 0;
            info!("🟢 {} запущен", stringify!($fn_name));
            loop {
                tokio::time::sleep(Duration::from_millis($interval_ms)).await;
                let phrase = phrases[rng.gen_range(0..phrases.len())];
                let payload = format!(concat!($prefix, " #{}: {}"), i, phrase);
                if tx.send(WorkerShit { topic: $topic, payload, priority: $priority }).await.is_err() {
                    break;
                }
                i += 1;
            }
        }
    };
}

shit_worker!(critical_shit_worker, ShitTopic::Critical, "🚨 КРИТИК", "critical", 15000, &[
    "БАЗА ДАННЫХ УТОНУЛА В ГОВНЕ",
    "PRODUCTION DOWN: сегфолт в оркестраторе",
    "MEMORY LEAK: 128GB говна и растёт",
    "DISK FULL: /var/log/govno заполнен",
    "KERNEL PANIC: говно переполнило стек",
]);

shit_worker!(liquid_shit_worker, ShitTopic::Liquid, "💧", "normal", 1800, &[
    "растекается по всей архитектуре",
    "проникает в каждый микросервис",
    "утекает в прод прямо сейчас",
    "затопило базу данных",
    "разлилось по логам на 3 гигабайта",
]);

shit_worker!(solid_shit_worker, ShitTopic::Solid, "🧱", "normal", 2500, &[
    "застряло в пайплайне уже 4 часа",
    "заблокировало CI/CD намертво",
    "лежит в очереди третий день",
    "упало на этапе деплоя с сегфолтом",
    "монолит не даёт себя разбить",
]);

shit_worker!(gas_shit_worker, ShitTopic::Gas, "💨", "normal", 1200, &[
    "заполнило весь Kubernetes кластер",
    "просочилось через firewall незаметно",
    "отравило продакшн окружение",
    "технический долг испаряется в воздух",
    "в атмосфере критическая концентрация",
]);

// ── Priority ShitOrchestrator ─────────────────────────────────────────────────
// Tier 2: biased select — critical_rx всегда дренируется первым.

async fn shit_orchestrator(
    mut critical_rx: mpsc::Receiver<WorkerShit>,
    mut normal_rx:   mpsc::Receiver<WorkerShit>,
    broadcast_tx:    broadcast::Sender<ShitMessage>,
    metrics:         Arc<ShitMetrics>,
) {
    info!("🎭 ShitOrchestrator (priority) запущен");
    let mut seq: u64 = 0;

    loop {
        let ws = tokio::select! {
            biased;                          // ← critical всегда первый
            msg = critical_rx.recv() => msg,
            msg = normal_rx.recv()   => msg,
        };

        let Some(ws) = ws else { break };

        seq += 1;
        metrics.inc_topic(&ws.topic, ws.payload.len() as u64);

        if ws.priority == "critical" {
            warn!("🚨 [seq={seq}] КРИТИК: {}", ws.payload);
        }

        let _ = broadcast_tx.send(ShitMessage {
            topic: ws.topic,
            seq,
            payload: ws.payload,
            priority: ws.priority,
        });
    }

    warn!("🎭 ShitOrchestrator завершил работу");
}

// ── Auth ───────────────────────────────────────────────────────────────────────

fn gen_session_id() -> String {
    let mut rng = rand::thread_rng();
    format!("💩-{:08x}", rng.gen::<u32>())
}

/// Возвращает session_id при успехе, None — при провале.
async fn authenticate(socket: &mut WebSocket, auth_token: &str, metrics: &ShitMetrics) -> Option<String> {
    send_json(socket, &ServerMsg::AuthRequired {
        msg: "Токен или проваливай. У тебя 10 секунд.",
    }).await;

    let result = tokio::time::timeout(
        Duration::from_secs(AUTH_TIMEOUT_SECS),
        socket.recv(),
    ).await;

    let raw = match result {
        Ok(Some(Ok(Message::Text(t)))) => t,
        _ => {
            send_json(socket, &ServerMsg::Unauthorized {
                msg: "💨 ПУУК! Таймаут авторизации. До свидания.",
            }).await;
            metrics.auth_failures.fetch_add(1, Ordering::Relaxed);
            return None;
        }
    };

    match serde_json::from_str::<ClientCmd>(&raw) {
        Ok(ClientCmd::Auth { token }) if token == auth_token => {
            let sid = gen_session_id();
            send_json(socket, &ServerMsg::Authorized {
                puk: "💨 ПУУК! Добро пожаловать в систему. Токен принят.",
                session_id: sid.clone(),
            }).await;
            metrics.auth_success.fetch_add(1, Ordering::Relaxed);
            info!("✅ Авторизован, session={sid}");
            Some(sid)
        }
        _ => {
            send_json(socket, &ServerMsg::Unauthorized {
                msg: "💨 ПУУК! Неверный токен. Иди отсюда.",
            }).await;
            metrics.auth_failures.fetch_add(1, Ordering::Relaxed);
            warn!("❌ Провал авторизации");
            None
        }
    }
}

// ── WebSocket handler ─────────────────────────────────────────────────────────

async fn ws_handler(
    ws:           WebSocketUpgrade,
    State(state): State<AppState>,
) -> impl IntoResponse {
    ws.on_upgrade(|socket| handle_socket(socket, state))
}

async fn handle_socket(mut socket: WebSocket, state: AppState) {
    state.metrics.connected_assholes.fetch_add(1, Ordering::Relaxed);

    // ── Auth gate ──
    let Some(_session_id) = authenticate(&mut socket, &state.auth_token, &state.metrics).await else {
        state.metrics.connected_assholes.fetch_sub(1, Ordering::Relaxed);
        return;
    };

    send_json(&mut socket, &ServerMsg::Welcome {
        msg: "Подпишитесь на топик: жидкое / твёрдое / газообразное / критическое",
    }).await;

    let mut bcast_rx = state.broadcast_tx.subscribe();
    let mut sub: Option<ShitTopic> = None;

    loop {
        tokio::select! {
            client_msg = socket.recv() => {
                match client_msg {
                    Some(Ok(Message::Text(text))) => {
                        match serde_json::from_str::<ClientCmd>(&text) {
                            Ok(cmd) => handle_cmd(cmd, &mut socket, &mut sub).await,
                            Err(e)  => send_json(&mut socket, &ServerMsg::Error {
                                msg: format!("bad json: {e}"),
                            }).await,
                        }
                    }
                    Some(Ok(Message::Close(_))) | None => break,
                    Some(Err(e)) => { warn!("WS: {e}"); break; }
                    _ => {}
                }
            }
            bcast = bcast_rx.recv() => {
                match bcast {
                    Ok(msg) => {
                        let send = match &sub {
                            Some(t) => t == &msg.topic,
                            None    => msg.priority == "critical",  // критику всегда
                        };
                        if send {
                            send_json(&mut socket, &ServerMsg::Shit(msg)).await;
                        }
                    }
                    Err(broadcast::error::RecvError::Lagged(n)) => {
                        warn!("⚡ Клиент отстал на {n} сообщений");
                    }
                    Err(broadcast::error::RecvError::Closed) => break,
                }
            }
        }
    }

    state.metrics.connected_assholes.fetch_sub(1, Ordering::Relaxed);
}

async fn handle_cmd(cmd: ClientCmd, socket: &mut WebSocket, sub: &mut Option<ShitTopic>) {
    match cmd {
        ClientCmd::Auth { .. } => {
            send_json(socket, &ServerMsg::Error {
                msg: "Уже авторизован, зачем снова?".into(),
            }).await;
        }
        ClientCmd::Subscribe { topic } => {
            let label = topic.label();
            info!("📬 Подписка: {label}");
            *sub = Some(topic);
            send_json(socket, &ServerMsg::Subscribed { topic: label }).await;
        }
        ClientCmd::Unsubscribe => {
            *sub = None;
            send_json(socket, &ServerMsg::Unsubscribed).await;
        }
        ClientCmd::Echo { text } => {
            send_json(socket, &ServerMsg::Echo {
                payload: format!("💩 ЭХО: {text}"),
            }).await;
        }
        ClientCmd::Ping => {
            send_json(socket, &ServerMsg::Pong).await;
        }
    }
}

async fn send_json(socket: &mut WebSocket, msg: &impl Serialize) {
    if let Ok(json) = serde_json::to_string(msg) {
        let _ = socket.send(Message::Text(json)).await;
    }
}

// ── HTTP handlers ─────────────────────────────────────────────────────────────

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
        .with_env_filter("govno_server=debug,tower_http=info")
        .init();

    let auth_token = Arc::new(
        env::var("GOVNO_TOKEN").unwrap_or_else(|_| "говно".to_string())
    );
    info!("🔑 Токен авторизации: [{}]", auth_token);

    let (broadcast_tx, _) = broadcast::channel::<ShitMessage>(BROADCAST_CAP);
    let metrics = Arc::new(ShitMetrics::default());

    // Tier 1: воркеры → два mpsc канала по приоритету
    let (critical_tx, critical_rx) = mpsc::channel::<WorkerShit>(16);
    let (normal_tx,   normal_rx)   = mpsc::channel::<WorkerShit>(64);

    tokio::spawn(critical_shit_worker(critical_tx));
    tokio::spawn(liquid_shit_worker(normal_tx.clone()));
    tokio::spawn(solid_shit_worker(normal_tx.clone()));
    tokio::spawn(gas_shit_worker(normal_tx));

    // Tier 2: приоритетный оркестратор
    tokio::spawn(shit_orchestrator(
        critical_rx, normal_rx, broadcast_tx.clone(), Arc::clone(&metrics),
    ));

    let state = AppState { broadcast_tx, metrics, auth_token };

    let app = Router::new()
        .route("/ws",           get(ws_handler))
        .route("/health",       get(|| async { "💩 живой" }))
        .route("/metrics",      get(metrics_handler))
        .route("/metrics/json", get(metrics_json_handler))
        .layer(CorsLayer::permissive())
        .with_state(state);

    let addr = "0.0.0.0:3000";
    info!("╔══════════════════════════════════════╗");
    info!("║  💩 ГОВНО-СЕРВЕР v0.3.0 → {}   ║", addr);
    info!("╚══════════════════════════════════════╝");

    axum::serve(
        tokio::net::TcpListener::bind(addr).await.unwrap(),
        app,
    ).await.unwrap();
}
