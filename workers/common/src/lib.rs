//! 💩 Говно-воркер: общая логика для всех worker-микросервисов
//!
//! # Как добавить новый воркер (новую какашку):
//!
//! 1. Создай папку `workers/my_new_shit/`
//! 2. Добавь `workers/my_new_shit` в workspace members в корневом `Cargo.toml`
//! 3. Создай `workers/my_new_shit/Cargo.toml` (см. пример liquid)
//! 4. Создай `workers/my_new_shit/src/main.rs`:
//!    ```rust
//!    fn main() {
//!        workers_common::run(workers_common::WorkerConfig {
//!            service_id:  "my-new-shit-worker".into(),
//!            topic:       proto::ShitTopic::Solid,   // или любой топик
//!            version:     env!("CARGO_PKG_VERSION").into(),
//!            interval_ms: 3000,
//!            description: "Описание нового говна".into(),
//!            phrases: &["фраза 1", "фраза 2"],
//!            priority: proto::Priority::Normal,
//!            tags: &["new", "shit"],
//!        });
//!    }
//!    ```
//! 5. Добавь сервис в `docker-compose.yml` по образцу liquid-worker
//! 6. Всё. Новая какашка готова.

use std::{env, time::Duration};

use futures_util::{SinkExt, StreamExt};
use rand::Rng;
use tokio_tungstenite::{connect_async, tungstenite};
use tracing::{info, warn, error};

use proto::{OrchestratorMsg, Priority, ProducerMsg, ShitTopic};

/// Configuration for a shit worker microservice.
/// Fill this in and call `run()` from main.
pub struct WorkerConfig {
    pub service_id:  String,
    pub topic:       ShitTopic,
    pub version:     String,
    pub interval_ms: u64,
    pub description: String,
    pub phrases:     &'static [&'static str],
    pub priority:    Priority,
    pub tags:        &'static [&'static str],
}

/// Entry point for every worker binary.
/// Blocks forever; handles reconnections automatically.
pub fn run(cfg: WorkerConfig) {
    tracing_subscriber::fmt()
        .with_env_filter(
            env::var("RUST_LOG")
                .unwrap_or_else(|_| format!("{}=debug", cfg.service_id.replace('-', "_")))
        )
        .init();

    let rt = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .expect("tokio runtime");

    rt.block_on(reconnect_loop(cfg));
}

/// Reconnect loop with exponential backoff.
/// Max delay: 60s. Resets after successful connection of >30s.
async fn reconnect_loop(cfg: WorkerConfig) {
    let orchestrator_url = env::var("ORCHESTRATOR_URL")
        .unwrap_or_else(|_| "ws://127.0.0.1:3000/producer".into());

    info!(
        "💩 {} v{} starting — target: {}",
        cfg.service_id, cfg.version, orchestrator_url
    );

    let mut attempt: u32 = 0;

    loop {
        let connect_ts = std::time::Instant::now();

        match connect_and_run(&cfg, &orchestrator_url).await {
            Ok(_) => {
                info!("✅ {} disconnected cleanly", cfg.service_id);
            }
            Err(e) => {
                warn!("❌ {} error: {}", cfg.service_id, e);
            }
        }

        // Reset backoff if we were connected for a while
        if connect_ts.elapsed() > Duration::from_secs(30) {
            attempt = 0;
        } else {
            attempt += 1;
        }

        let delay_secs = backoff_secs(attempt);
        warn!(
            "🔁 {} reconnecting in {}s (attempt {})",
            cfg.service_id, delay_secs, attempt
        );
        tokio::time::sleep(Duration::from_secs(delay_secs)).await;
    }
}

/// Exponential backoff: 1s, 2s, 4s, 8s, 16s, 32s, 60s (capped)
fn backoff_secs(attempt: u32) -> u64 {
    let base: u64 = 2u64.pow(attempt.min(6));
    base.min(60)
}

async fn connect_and_run(
    cfg: &WorkerConfig,
    url: &str,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    info!("🔌 {} connecting to {}", cfg.service_id, url);

    let (ws_stream, response) = connect_async(url).await?;

    info!(
        "🟢 {} connected — HTTP {} {}",
        cfg.service_id,
        response.status().as_u16(),
        response.status().canonical_reason().unwrap_or("")
    );

    let (mut write, mut read) = ws_stream.split();

    // Send Hello — register with orchestrator
    let hello = serde_json::to_string(&ProducerMsg::Hello {
        service_id:  cfg.service_id.clone(),
        topic:       cfg.topic.clone(),
        version:     cfg.version.clone(),
        interval_ms: cfg.interval_ms,
        description: cfg.description.clone(),
    })?;
    write.send(tungstenite::Message::Text(hello)).await?;

    // Expect Welcome
    match read.next().await {
        Some(Ok(tungstenite::Message::Text(t))) => {
            match serde_json::from_str::<OrchestratorMsg>(&t)? {
                OrchestratorMsg::Welcome { assigned_id } => {
                    info!("🎫 {} registered as {}", cfg.service_id, assigned_id);
                }
                OrchestratorMsg::Reject { reason } => {
                    error!("🚫 {} rejected by orchestrator: {}", cfg.service_id, reason);
                    return Err(format!("rejected: {reason}").into());
                }
                OrchestratorMsg::Ack { .. } => {
                    // Unexpected but harmless
                }
            }
        }
        other => {
            return Err(format!("unexpected welcome: {other:?}").into());
        }
    }

    // Main emit loop
    let mut rng = rand::thread_rng();
    let mut counter: u64 = 0;
    let mut interval = tokio::time::interval(Duration::from_millis(cfg.interval_ms));

    loop {
        tokio::select! {
            _ = interval.tick() => {
                let phrase = cfg.phrases[rng.gen_range(0..cfg.phrases.len())];
                let payload = format!(
                    "{} {} #{}: {}",
                    cfg.topic.emoji(), cfg.service_id, counter, phrase
                );

                let msg = serde_json::to_string(&ProducerMsg::Emit {
                    payload,
                    priority: cfg.priority.clone(),
                    tags: cfg.tags.iter().map(|s| s.to_string()).collect(),
                })?;

                if let Err(e) = write.send(tungstenite::Message::Text(msg)).await {
                    warn!("{} send error: {}", cfg.service_id, e);
                    break;
                }

                counter += 1;
            }

            msg = read.next() => {
                match msg {
                    Some(Ok(tungstenite::Message::Text(t))) => {
                        // Ack from orchestrator — just log at trace level
                        if let Ok(OrchestratorMsg::Ack { seq }) = serde_json::from_str(&t) {
                            tracing::trace!("{} ack seq={}", cfg.service_id, seq);
                        }
                    }
                    Some(Ok(tungstenite::Message::Close(_))) | None => {
                        info!("{} received close", cfg.service_id);
                        break;
                    }
                    Some(Err(e)) => {
                        warn!("{} read error: {}", cfg.service_id, e);
                        break;
                    }
                    _ => {}
                }
            }
        }
    }

    Ok(())
}
