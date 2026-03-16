# 💩 ГОВНО v0.3.0

> Microservice WebSocket инфраструктура на Rust + WASM

[![CI](https://github.com/oneErrortime/govno/actions/workflows/ci.yml/badge.svg)](https://github.com/oneErrortime/govno/actions/workflows/ci.yml)
![Rust](https://img.shields.io/badge/rust-%23000000.svg?style=flat&logo=rust)
![WebAssembly](https://img.shields.io/badge/WebAssembly-654FF0?style=flat&logo=WebAssembly&logoColor=white)
![Lines of JS](https://img.shields.io/badge/JS%20business%20logic-0%20lines-brightgreen)
![Docker](https://img.shields.io/badge/docker-%230db7ed.svg?style=flat&logo=docker&logoColor=white)

**[🌐 GitHub Pages Demo](https://oneerrortime.github.io/govno/)**

## Архитектура

```
workers/liquid  ──ws /producer──┐
workers/solid   ──ws /producer──┤
workers/gas     ──ws /producer──┤──► orchestrator ──broadcast──► WASM клиент (/ws)
workers/critical──ws /producer──┘         │
                                     /api/services   ← live registry
                                     /metrics        ← Prometheus
                                     /metrics/json   ← JSON
```

Каждый воркер — отдельный **микросервис-бинарь**. Оркестратор — динамический
брокер: воркеры регистрируются через `ProducerMsg::Hello`, оркестратор выдаёт
`assigned_id`. При дисконнекте — авто-дерегистрация. Перезапуск оркестратора
не требуется при добавлении/удалении воркеров.

## Workspace

```
govno/
├── proto/                  — shared protocol types (serde, без зависимостей)
├── orchestrator/           — Axum WS broker, registry, broadcast, auth, metrics
├── workers/
│   ├── common/             — reconnect loop, WS client, WorkerConfig trait
│   ├── liquid/             — 💧 жидкое говно (1.8s)
│   ├── solid/              — 🧱 твёрдое говно (2.5s)
│   ├── gas/                — 💨 газообразное говно (1.2s)
│   └── critical/           — 🚨 критические инциденты (15s)
├── client/src/             — Rust → WASM
│   ├── lib.rs              — public API (#[wasm_bindgen] exports)
│   ├── protocol.rs         — serde types (зеркало proto)
│   ├── state.rs            — AppState, ConnectionState machine, RateTracker
│   ├── dom.rs              — DOM helpers, toast, service panel, install_tick
│   ├── canvas.rs           — sparkline charts (CanvasRenderingContext2d)
│   ├── audio.rs            — AudioContext alerts для critical
│   └── ws.rs               — WebSocket lifecycle, auto-reconnect, dispatch
├── www/
│   ├── index.html          — UI: auth, topics, log, sparklines, services, metrics
│   └── pkg/                ← wasm-pack output (gitignored)
├── Dockerfile              — multistage: dep-cache stage + builder + runtime
├── docker-compose.yml      — orchestrator + 4 workers + nginx + wasm-builder
└── .github/workflows/ci.yml
```

## Как добавить новую какашку

1. Создай `workers/my_new_shit/`:
   ```
   workers/my_new_shit/
   ├── Cargo.toml
   └── src/main.rs
   ```

2. `Cargo.toml`:
   ```toml
   [package]
   name = "govno-worker-my-new-shit"
   version = "0.1.0"
   edition = "2021"
   [[bin]]
   name = "govno-worker-my-new-shit"
   path = "src/main.rs"
   [dependencies]
   workers_common = { path = "../common" }
   proto          = { path = "../../proto" }
   ```

3. `src/main.rs`:
   ```rust
   fn main() {
       workers_common::run(workers_common::WorkerConfig {
           service_id:  "my-new-shit-worker".into(),
           topic:       proto::ShitTopic::Gas,    // or any existing topic
           version:     env!("CARGO_PKG_VERSION").into(),
           interval_ms: 3000,
           description: "Описание нового говна".into(),
           priority:    proto::Priority::Normal,
           tags:        &["new", "shit"],
           phrases:     &["фраза 1", "фраза 2"],
       });
   }
   ```

4. Добавь в корневой `Cargo.toml`:
   ```toml
   members = [..., "workers/my_new_shit"]
   ```

5. Добавь в `docker-compose.yml` (скопировать секцию `liquid-worker`).

6. `docker compose up --build my-new-shit-worker`

Оркестратор подхватит новый воркер **без перезапуска** — он просто подключится
через `/producer`, отправит `Hello`, и сразу начнёт отдавать сообщения.

## Запуск

### Docker

```bash
# Сначала один раз — собрать WASM клиент
docker compose run --rm wasm-builder

# Полный стек: оркестратор + 4 воркера + nginx
docker compose up --build

# Открыть: http://localhost:8080
# Токен: говно  (или GOVNO_TOKEN=... docker compose up)
```

### Вручную

```bash
rustup target add wasm32-unknown-unknown
curl https://rustwasm.github.io/wasm-pack/installer/init.sh -sSf | sh

# Terminal 1: оркестратор
GOVNO_TOKEN=говно cargo run -p govno-orchestrator

# Terminal 2-5: воркеры (каждый в своём терминале)
cargo run -p govno-worker-liquid
cargo run -p govno-worker-solid
cargo run -p govno-worker-gas
cargo run -p govno-worker-critical

# Terminal 6: WASM + статика
wasm-pack build client --target web --out-dir ../www/pkg
cd www && python3 -m http.server 8080
```

## Протокол

### Worker → Orchestrator

```jsonc
// Регистрация (первое сообщение)
{"type":"hello","service_id":"liquid-shit-worker","topic":"liquid",
 "version":"0.1.0","interval_ms":1800,"description":"..."}

// Продукт
{"type":"emit","payload":"💧 говно #42: ...","priority":"normal","tags":["liquid"]}

// Отключение
{"type":"bye"}
```

### Orchestrator → Worker

```jsonc
{"type":"welcome","assigned_id":"💩-1a2b3c4d"}
{"type":"ack","seq":42}
{"type":"reject","reason":"first message must be Hello"}
```

### Consumer → Orchestrator

```jsonc
{"cmd":"auth","token":"говно"}
{"cmd":"subscribe","topic":"liquid"}
{"cmd":"unsubscribe","topic":"liquid"}
{"cmd":"unsubscribe_all"}
{"cmd":"ping"}
{"cmd":"echo","text":"тест"}
```

## WASM клиент

Состоит из 7 Rust-модулей:

| Модуль | Что делает |
|--------|-----------|
| `lib.rs` | Публичный API, `#[wasm_bindgen]` экспорты |
| `protocol.rs` | serde-типы протокола |
| `state.rs` | `AppState`, `ConnectionState` machine, `RateTracker` (30-bucket) |
| `dom.rs` | DOM helpers, toast, service panel, 1s tick |
| `canvas.rs` | Sparkline charts via `CanvasRenderingContext2d` |
| `audio.rs` | `AudioContext` alerts (критические — beep) |
| `ws.rs` | WS lifecycle, auto-reconnect с exponential backoff |

Клавиатурные шорткаты: `/` — фокус фильтра, `a` — подписаться на всё,
`x` — отписаться, `c` — очистить лог, `p` — ping.

## GitHub Pages

CI после каждого push в `main`:
1. Собирает WASM через `wasm-pack build --release`
2. Деплоит `www/` → ветка `gh-pages`

**Активировать**: Settings → Pages → Source: `gh-pages` / root.

На Pages: WASM клиент работает полностью, нужно только ввести URL своего сервера.
Chrome разрешает `ws://localhost` с `https://` страниц.

---

*«любой достаточно продвинутый говнокод неотличим от архитектуры»*
