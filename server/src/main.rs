// ╔══════════════════════════════════════════════════════════════════════╗
// ║   💩  ГОВНО-СЕРВЕР  v0.2.0                                          ║
// ║                                                                      ║
// ║   GasShitWorker ──┐                                                  ║
// ║   LiquidShitWorker─┤─mpsc─► ShitOrchestrator ─broadcast─► clients   ║
// ║   SolidShitWorker ─┘                           └──► /metrics         ║
// ╚══════════════════════════════════════════════════════════════════════╝

use std::{
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

// ── Топики говна ─────────────────────────────────────────────────────────────

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
enum ShitTopic {
    #[serde(rename = "жидкое")]
    Liquid,
    #[serde(rename = "твёрдое")]
    Solid,
    #[serde(rename = "газообразное")]
    Gas,
}

impl ShitTopic {
    fn label(&self) -> &'static str {
        match self {
            Self::Liquid => "жидкое",
            Self::Solid  => "твёрдое",
            Self::Gas    => "газообразное",
        }
    }
}

// ── Сообщения протокола ───────────────────────────────────────────────────────

#[derive(Clone, Debug, Serialize)]
struct ShitMessage {
    topic:   ShitTopic,
    seq:     u64,
    payload: String,
}

#[derive(Debug, Deserialize)]
#[serde(tag = "cmd", rename_all = "snake_case")]
enum ClientCmd {
    Subscribe   { topic: ShitTopic },
    Unsubscribe,
    Echo        { text: String },
    Ping,
}

#[derive(Debug, Serialize)]
#[serde(tag = "type", rename_all = "snake_case")]
enum ServerMsg<'a> {
    Welcome     { msg: &'a str },
    Subscribed  { topic: &'a str },
    Unsubscribed,
    Shit        (ShitMessage),
    Echo        { payload: String },
    Pong,
    Error       { msg: String },
}

// ── Метрики ───────────────────────────────────────────────────────────────────

#[derive(Default)]
struct ShitMetrics {
    messages_total:     AtomicU64,
    bytes_sent:         AtomicU64,
    connected_assholes: AtomicI64,
    liquid_total:       AtomicU64,
    solid_total:        AtomicU64,
    gas_total:          AtomicU64,
}

impl ShitMetrics {
    fn inc_topic(&self, topic: &ShitTopic, bytes: u64) {
        self.messages_total.fetch_add(1, Ordering::Relaxed);
        self.bytes_sent.fetch_add(bytes, Ordering::Relaxed);
        match topic {
            ShitTopic::Liquid => { self.liquid_total.fetch_add(1, Ordering::Relaxed); }
            ShitTopic::Solid  => { self.solid_total.fetch_add(1,  Ordering::Relaxed); }
            ShitTopic::Gas    => { self.gas_total.fetch_add(1,    Ordering::Relaxed); }
        }
    }

    fn prometheus(&self) -> String {
        let mut s = String::new();
        let rows: &[(&str, &str, &str, u64)] = &[
            ("shit_messages_total",  "Total shit messages orchestrated", "counter", self.messages_total.load(Ordering::Relaxed)),
            ("shit_bytes_sent",      "Total bytes of shit broadcast",    "counter", self.bytes_sent.load(Ordering::Relaxed)),
            ("shit_liquid_total",    "Liquid shit messages produced",    "counter", self.liquid_total.load(Ordering::Relaxed)),
            ("shit_solid_total",     "Solid shit messages produced",     "counter", self.solid_total.load(Ordering::Relaxed)),
            ("shit_gas_total",       "Gas shit messages produced",       "counter", self.gas_total.load(Ordering::Relaxed)),
        ];
        for (name, help, type_, val) in rows {
            s.push_str(&format!("# HELP {name} {help}\n# TYPE {name} {type_}\n{name} {val}\n\n"));
        }
        // gauge отдельно — знаковый
        let ca = self.connected_assholes.load(Ordering::Relaxed);
        s.push_str(&format!("# HELP connected_assholes Current WebSocket connections\n# TYPE connected_assholes gauge\nconnected_assholes {ca}\n\n"));
        s
    }

    fn to_json(&self) -> serde_json::Value {
        serde_json::json!({
            "shit_messages_total":  self.messages_total.load(Ordering::Relaxed),
            "shit_bytes_sent":      self.bytes_sent.load(Ordering::Relaxed),
            "connected_assholes":   self.connected_assholes.load(Ordering::Relaxed),
            "shit_liquid_total":    self.liquid_total.load(Ordering::Relaxed),
            "shit_solid_total":     self.solid_total.load(Ordering::Relaxed),
            "shit_gas_total":       self.gas_total.load(Ordering::Relaxed),
        })
    }
}

// ── AppState ──────────────────────────────────────────────────────────────────

#[derive(Clone)]
struct AppState {
    broadcast_tx: broadcast::Sender<ShitMessage>,
    metrics:      Arc<ShitMetrics>,
}

// ── Воркеры (макрос убирает бойлерплейт) ─────────────────────────────────────

struct WorkerShit {
    topic:   ShitTopic,
    payload: String,
}

macro_rules! shit_worker {
    ($fn_name:ident, $topic:expr, $prefix:literal, $interval_ms:literal, $phrases:expr) => {
        async fn $fn_name(tx: mpsc::Sender<WorkerShit>) {
            let phrases: &[&str] = $phrases;
            let mut rng = rand::thread_rng();
            let mut i: u64 = 0;
            info!("🟢 {} запущен", stringify!($fn_name));
            loop {
                tokio::time::sleep(Duration::from_millis($interval_ms)).await;
                let phrase = phrases[rng.gen_range(0..phrases.len())];
                let payload = format!(concat!($prefix, " говно #{}: {}"), i, phrase);
                if tx.send(WorkerShit { topic: $topic, payload }).await.is_err() {
                    break;
                }
                i += 1;
            }
            warn!("🔴 {} завершён", stringify!($fn_name));
        }
    };
}

shit_worker!(liquid_shit_worker, ShitTopic::Liquid, "💧", 1800, &[
    "растекается по всей архитектуре",
    "проникает в каждый микросервис",
    "утекает в прод прямо сейчас",
    "затопило базу данных",
    "разлилось по логам на 3 гигабайта",
    "просочилось через все абстракции",
]);

shit_worker!(solid_shit_worker, ShitTopic::Solid, "🧱", 2500, &[
    "застряло в пайплайне уже 4 часа",
    "заблокировало CI/CD намертво",
    "лежит в очереди третий день",
    "не прошло code review снова",
    "упало на этапе деплоя с сегфолтом",
    "монолит не даёт себя разбить",
]);

shit_worker!(gas_shit_worker, ShitTopic::Gas, "💨", 1200, &[
    "заполнило весь Kubernetes кластер",
    "просочилось через firewall незаметно",
    "в атмосфере критическая концентрация",
    "отравило продакшн окружение",
    "расширилось до размеров дата-центра",
    "технический долг испаряется в воздух",
]);

// ── ShitOrchestrator ──────────────────────────────────────────────────────────

async fn shit_orchestrator(
    mut rx:       mpsc::Receiver<WorkerShit>,
    broadcast_tx: broadcast::Sender<ShitMessage>,
    metrics:      Arc<ShitMetrics>,
) {
    info!("🎭 ShitOrchestrator запущен — координирую потоки говна");
    let mut seq: u64 = 0;

    while let Some(ws) = rx.recv().await {
        seq += 1;
        metrics.inc_topic(&ws.topic, ws.payload.len() as u64);
        let msg = ShitMessage { topic: ws.topic, seq, payload: ws.payload };
        let _ = broadcast_tx.send(msg); // Err = нет подписчиков, не фатально
    }

    warn!("🎭 ShitOrchestrator завершил работу");
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
    info!("💩 +1. Очков: {}", state.metrics.connected_assholes.load(Ordering::Relaxed));

    send_json(&mut socket, &ServerMsg::Welcome {
        msg: "Добро пожаловать в говно-стрим! Подпишитесь на топик командой subscribe.",
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
                    Some(Ok(Message::Close(_))) | None => {
                        info!("🚪 Клиент закрыл соединение");
                        break;
                    }
                    Some(Err(e)) => { warn!("💥 WS: {e}"); break; }
                    _ => {}
                }
            }

            bcast = bcast_rx.recv() => {
                match bcast {
                    Ok(msg) => {
                        if sub.as_ref().is_some_and(|t| t == &msg.topic) {
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
    info!("💩 -1. Осталось: {}", state.metrics.connected_assholes.load(Ordering::Relaxed));
}

async fn handle_cmd(cmd: ClientCmd, socket: &mut WebSocket, sub: &mut Option<ShitTopic>) {
    match cmd {
        ClientCmd::Subscribe { topic } => {
            let label = topic.label();
            info!("📬 Подписка на '{label}'");
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
        .with_env_filter("govno_server=debug,tower_http=debug")
        .init();

    let (broadcast_tx, _) = broadcast::channel::<ShitMessage>(BROADCAST_CAP);
    let metrics = Arc::new(ShitMetrics::default());

    {
        let (worker_tx, worker_rx) = mpsc::channel::<WorkerShit>(64);
        tokio::spawn(liquid_shit_worker(worker_tx.clone()));
        tokio::spawn(solid_shit_worker(worker_tx.clone()));
        tokio::spawn(gas_shit_worker(worker_tx));
        tokio::spawn(shit_orchestrator(worker_rx, broadcast_tx.clone(), Arc::clone(&metrics)));
    }

    let state = AppState { broadcast_tx, metrics };

    let app = Router::new()
        .route("/ws",           get(ws_handler))
        .route("/health",       get(|| async { "💩 живой" }))
        .route("/metrics",      get(metrics_handler))
        .route("/metrics/json", get(metrics_json_handler))
        .layer(CorsLayer::permissive())
        .with_state(state);

    let addr = "0.0.0.0:3000";
    info!("╔══════════════════════════════════════════════╗");
    info!("║  💩 ГОВНО-СЕРВЕР v0.2.0 на {}         ║", addr);
    info!("║  /ws            WebSocket (pub/sub)         ║");
    info!("║  /metrics       Prometheus text             ║");
    info!("║  /metrics/json  JSON метрики                ║");
    info!("╚══════════════════════════════════════════════╝");

    axum::serve(
        tokio::net::TcpListener::bind(addr).await.unwrap(),
        app,
    )
    .await
    .unwrap();
}
