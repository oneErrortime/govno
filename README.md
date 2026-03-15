# 💩 ГОВНО

> **enterprise-grade WebSocket инфраструктура на Rust**

[![CI](https://github.com/oneErrortime/govno/actions/workflows/ci.yml/badge.svg)](https://github.com/oneErrortime/govno/actions/workflows/ci.yml)
![Rust](https://img.shields.io/badge/rust-%23000000.svg?style=flat&logo=rust&logoColor=white)
![WebAssembly](https://img.shields.io/badge/WebAssembly-654FF0?style=flat&logo=WebAssembly&logoColor=white)
![Lines of JS](https://img.shields.io/badge/lines%20of%20JS-0-brightgreen)

## Что это

Минимальный пример WebSocket-коммуникации между:

| Компонент | Технологии |
|-----------|-----------|
| **Сервер** | Rust + Axum + Tokio |
| **Клиент** | Rust → **WebAssembly** (wasm-bindgen + web-sys) |
| **Протокол** | WebSocket (`ws://`) |
| **JavaScript** | 0 строк бизнес-логики |

Весь клиентский код — это Rust, скомпилированный в `.wasm`.  
Единственный «JS» в проекте — клей-загрузчик, сгенерированный `wasm-bindgen`.

## Структура

```
govno/
├── server/            # 🦀 Rust WebSocket сервер (Axum)
│   └── src/main.rs
├── client/            # 🦀 Rust WASM клиент
│   └── src/lib.rs
├── www/               # 🌐 Статика
│   ├── index.html     # UI (загружает WASM)
│   └── pkg/           # ← генерируется wasm-pack (gitignored)
├── Makefile
└── .github/workflows/ci.yml
```

## Быстрый старт

### 1. Зависимости

```bash
# Rust (если нет)
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh

# wasm-pack
curl https://rustwasm.github.io/wasm-pack/installer/init.sh -sSf | sh

# wasm32 таргет
rustup target add wasm32-unknown-unknown
```

### 2. Собрать клиент

```bash
make client
# или вручную:
wasm-pack build client --target web --out-dir ../www/pkg
```

### 3. Запустить сервер

```bash
# в отдельном терминале
make server
# 💩 ГОВНО-СЕРВЕР СТАРТУЕТ НА 0.0.0.0:3000
```

### 4. Запустить статику

```bash
# в другом терминале
make www
# http://localhost:8080
```

Открой `http://localhost:8080` — наблюдай говно в браузере.

## Как работает

```
 Browser                          Server
   │                                │
   │── init WASM ──────────────────>│
   │   (Rust код в .wasm)           │
   │                                │
   │── ws://localhost:3000/ws ─────>│  WebSocket handshake
   │<─ "💩 ДОБРО ПОЖАЛОВАТЬ..." ───│  приветствие
   │                                │
   │── "привет" ──────────────────>│  send_message() из Rust/WASM
   │<─ "💩 ЭХО: привет" ──────────│  echo с префиксом
```

Функции, доступные из HTML (экспортированы `#[wasm_bindgen]`):

```rust
pub fn connect(url: &str)          // подключиться к WS
pub fn send_message(msg: &str)     // отправить сообщение
pub fn disconnect()                // закрыть соединение
```

## CI

GitHub Actions собирает и сервер, и WASM-клиент на каждый push.  
WASM артефакт (`www/pkg/`) загружается как artifact сборки.

---

*«любой достаточно продвинутый говнокод неотличим от архитектуры»*
