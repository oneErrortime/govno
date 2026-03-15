# 💩 ГОВНО

> enterprise-grade WebSocket инфраструктура на Rust

[![CI](https://github.com/oneErrortime/govno/actions/workflows/ci.yml/badge.svg)](https://github.com/oneErrortime/govno/actions/workflows/ci.yml)
![Rust](https://img.shields.io/badge/rust-%23000000.svg?style=flat&logo=rust&logoColor=white)
![WebAssembly](https://img.shields.io/badge/WebAssembly-654FF0?style=flat&logo=WebAssembly&logoColor=white)
![Lines of JS](https://img.shields.io/badge/lines%20of%20JS%20business%20logic-0-brightgreen)
![Docker](https://img.shields.io/badge/docker-%230db7ed.svg?style=flat&logo=docker&logoColor=white)

**[🌐 GitHub Pages Demo](https://oneerrortime.github.io/govno/)**

## Архитектура v0.3.0

```
 CriticalShitWorker (15s) ──critical_tx──┐
 LiquidShitWorker   (1.8s) ──────────────┤──mpsc──► ShitOrchestrator ──broadcast──► WS-клиенты
 SolidShitWorker    (2.5s) ─── normal_tx─┤          (biased select:                  (фильтр по
 GasShitWorker      (1.2s) ──────────────┘          critical > normal)                топику)
                                                     └── /metrics (Prometheus)
                                          Auth gate: GOVNO_TOKEN → 💨 ПУУК или welcome
```

| Компонент | Stack |
|-----------|-------|
| **Сервер** | Rust + Axum + Tokio |
| **Клиент** | Rust → **WebAssembly** (wasm-bindgen + web-sys) |
| **Протокол** | WebSocket + JSON (serde) |
| **Оркестрация** | mpsc (biased priority) → broadcast |
| **Метрики** | `/metrics` Prometheus + `/metrics/json` |
| **Auth** | Token-based, `GOVNO_TOKEN` env, пук при отказе |
| **Deploy** | Docker multistage + GitHub Pages (WASM) |

## Запуск

### Docker (рекомендуется)

```bash
# Один раз: собрать WASM клиент
docker compose run --rm wasm-builder

# Поднять сервер + nginx с клиентом
docker compose up

# Открыть: http://localhost:8080
# Токен по умолчанию: говно
```

### Вручную

```bash
# Зависимости
rustup target add wasm32-unknown-unknown
curl https://rustwasm.github.io/wasm-pack/installer/init.sh -sSf | sh

# Terminal 1: сервер
GOVNO_TOKEN=говно cargo run -p govno-server

# Terminal 2: WASM + статика
wasm-pack build client --target web --out-dir ../www/pkg
cd www && python3 -m http.server 8080
```

## GitHub Pages

После push в `main` Actions:
1. Собирает WASM через `wasm-pack`
2. Деплоит `www/` (+ `www/pkg/`) в ветку `gh-pages`

**Активировать Pages**: Settings → Pages → Source: `gh-pages` branch → `/` root.

На GitHub Pages клиент предлагает ввести URL сервера. Запусти сервер локально
(Docker или cargo) и введи `ws://localhost:3000/ws`. Chrome разрешает
ws://localhost с HTTPS-страниц.

## Метрики

```
GET /metrics        → Prometheus text format
GET /metrics/json   → JSON

Метрики:
  shit_messages_total      — всего оркестрировано говна
  shit_bytes_sent          — байт отправлено
  connected_assholes       — активных WS-соединений
  shit_auth_success        — успешных авторизаций
  shit_auth_failures       — провалов авторизации
  shit_{liquid,solid,gas,critical}_total — по топикам
```

## Структура

```
govno/
├── server/src/main.rs    — Axum WS сервер, ShitOrchestrator, auth, metrics
├── client/src/lib.rs     — Rust → WASM, auth flow, pub/sub, serde_json
├── www/
│   ├── index.html        — UI (auth overlay, smart URL, topics, metrics)
│   └── pkg/              — ← wasm-pack output (gitignored, CI собирает)
├── Dockerfile            — multistage: builder (rust:slim) + runtime (debian:slim)
├── docker-compose.yml    — server + nginx + wasm-builder profile
├── nginx.conf
└── .github/workflows/ci.yml   — CI + gh-pages deploy
```

---

*«любой достаточно продвинутый говнокод неотличим от архитектуры»*
