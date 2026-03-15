use axum::{
    extract::ws::{Message, WebSocket, WebSocketUpgrade},
    response::IntoResponse,
    routing::get,
    Router,
};
use tower_http::cors::CorsLayer;
use tracing::{info, warn};

// ╔══════════════════════════════════════════════╗
// ║        💩  ГОВНО-СЕРВЕР  v0.0.1  💩          ║
// ║   enterprise-grade WebSocket infrastructure  ║
// ╚══════════════════════════════════════════════╝

#[tokio::main]
async fn main() {
    tracing_subscriber::fmt()
        .with_env_filter("govno_server=debug,tower_http=debug")
        .init();

    let app = Router::new()
        .route("/ws", get(ws_handler))
        // health-check для тех, кто серьёзный
        .route("/health", get(|| async { "💩 живой" }))
        .layer(CorsLayer::permissive());

    let addr = "0.0.0.0:3000";
    info!("╔═══════════════════════════════════════╗");
    info!("║  💩  ГОВНО-СЕРВЕР СТАРТУЕТ НА {}  ║", addr);
    info!("╚═══════════════════════════════════════╝");

    let listener = tokio::net::TcpListener::bind(addr).await.unwrap();
    axum::serve(listener, app).await.unwrap();
}

async fn ws_handler(ws: WebSocketUpgrade) -> impl IntoResponse {
    ws.on_upgrade(handle_socket)
}

async fn handle_socket(mut socket: WebSocket) {
    info!("💩 Новое подключение — добро пожаловать в говно!");

    let welcome = "💩 ГОВНО-СЕРВЕР ПРИВЕТСТВУЕТ ВАС. Пишите что угодно — получите обратно в 💩-формате.";
    if socket
        .send(Message::Text(welcome.to_string()))
        .await
        .is_err()
    {
        warn!("Клиент исчез раньше, чем прочитал приветствие 💔");
        return;
    }

    while let Some(msg) = socket.recv().await {
        match msg {
            Ok(Message::Text(text)) => {
                info!("📨 Получено: {}", text);
                let reply = format!("💩 ЭХО: {}", text);
                if socket.send(Message::Text(reply)).await.is_err() {
                    warn!("Не смогли отправить ответ — клиент уже ушёл");
                    break;
                }
            }
            Ok(Message::Ping(data)) => {
                // axum по умолчанию отвечает на Ping, но можно и вручную
                let _ = socket.send(Message::Pong(data)).await;
            }
            Ok(Message::Close(_)) => {
                info!("🚪 Клиент вежливо закрыл соединение");
                break;
            }
            Err(e) => {
                warn!("💥 Ошибка WebSocket: {}", e);
                break;
            }
            _ => {}
        }
    }

    info!("💩 Соединение завершено");
}
