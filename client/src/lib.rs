// ╔══════════════════════════════════════════════════════════════════════╗
// ║   💩  ГОВНО-КЛИЕНТ  v0.3.0  —  100% Rust → WASM                    ║
// ║   Auth flow, pub/sub, serde_json протокол, ноль JS бизнес-логики    ║
// ╚══════════════════════════════════════════════════════════════════════╝

use std::cell::RefCell;

use js_sys::Date;
use serde::{Deserialize, Serialize};
use wasm_bindgen::prelude::*;
use wasm_bindgen::JsCast;
use web_sys::{CloseEvent, ErrorEvent, MessageEvent, WebSocket};

// ── Протокол ──────────────────────────────────────────────────────────────────

#[derive(Serialize)]
#[serde(tag = "cmd", rename_all = "snake_case")]
enum ClientCmd<'a> {
    Auth        { token: &'a str },
    Subscribe   { topic: &'a str },
    Unsubscribe,
    Echo        { text: &'a str },
    Ping,
}

#[derive(Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
enum ServerMsg {
    AuthRequired { msg: String },
    Authorized   { puk: String, session_id: String },
    Unauthorized { msg: String },
    Welcome      { msg: String },
    Subscribed   { topic: String },
    Unsubscribed,
    Shit         { topic: String, seq: u64, payload: String, priority: String },
    Echo         { payload: String },
    Pong,
    Error        { msg: String },
}

// ── Глобальное состояние ──────────────────────────────────────────────────────

thread_local! {
    static WS: RefCell<Option<WebSocket>> = const { RefCell::new(None) };
}

// ── DOM хелперы ───────────────────────────────────────────────────────────────

fn document() -> web_sys::Document {
    web_sys::window().unwrap().document().unwrap()
}

fn now_hms() -> String {
    let d = Date::new_0();
    format!("{:02}:{:02}:{:02}", d.get_hours(), d.get_minutes(), d.get_seconds())
}

fn log(text: &str, class: &str) {
    let doc = document();
    let Some(el) = doc.get_element_by_id("log") else { return };
    let p = doc.create_element("p").unwrap();
    p.set_class_name(class);
    p.set_text_content(Some(&format!("[{}] {}", now_hms(), text)));
    el.append_child(&p).unwrap();
    el.set_scroll_top(el.scroll_height());
}

fn set_status(icon: &str, text: &str, class: &str) {
    if let Some(el) = document().get_element_by_id("status") {
        el.set_class_name(class);
        el.set_text_content(Some(&format!("{icon} {text}")));
    }
}

fn set_sub_label(topic: Option<&str>) {
    if let Some(el) = document().get_element_by_id("sub-label") {
        el.set_text_content(Some(match topic {
            Some(t) => &format!("подписан: {t}"),
            None    => "не подписан",
        }));
    }
}

fn set_btn(id: &str, enabled: bool) {
    let doc = document();
    if let Some(btn) = doc.get_element_by_id(id) {
        if enabled { btn.remove_attribute("disabled").unwrap(); }
        else       { btn.set_attribute("disabled", "").unwrap(); }
    }
}

fn show_el(id: &str, visible: bool) {
    let doc = document();
    if let Some(el) = doc.get_element_by_id(id) {
        let _ = el.set_attribute("style", if visible { "" } else { "display:none" });
    }
}

fn set_text(id: &str, text: &str) {
    if let Some(el) = document().get_element_by_id(id) {
        el.set_text_content(Some(text));
    }
}

fn enable_main_ui(session_id: &str) {
    show_el("auth-overlay", false);
    show_el("main-ui", true);
    set_text("session-id", session_id);
    set_btn("btn-send",   true);
    set_btn("btn-liquid", true);
    set_btn("btn-solid",  true);
    set_btn("btn-gas",    true);
    set_btn("btn-critical", true);
    set_btn("btn-unsub",  true);
}

// ── Входящие сообщения ────────────────────────────────────────────────────────

fn handle_server_msg(text: &str) {
    match serde_json::from_str::<ServerMsg>(text) {
        Ok(ServerMsg::AuthRequired { msg }) => {
            log(&msg, "msg-system");
        }
        Ok(ServerMsg::Authorized { puk, session_id }) => {
            log(&puk, "msg-puk");
            set_status("🟢", "авторизован", "status-ok");
            enable_main_ui(&session_id);
        }
        Ok(ServerMsg::Unauthorized { msg }) => {
            log(&msg, "msg-error");
            set_status("🔴", "отказано", "status-error");
            set_btn("btn-auth", true);
        }
        Ok(ServerMsg::Welcome { msg }) => {
            log(&msg, "msg-system");
        }
        Ok(ServerMsg::Subscribed { topic }) => {
            log(&format!("📬 Подписка на «{}» оформлена", topic), "msg-system");
            set_sub_label(Some(&topic));
        }
        Ok(ServerMsg::Unsubscribed) => {
            log("📭 Отписались", "msg-system");
            set_sub_label(None);
        }
        Ok(ServerMsg::Shit { topic, seq, payload, priority }) => {
            let class = if priority == "critical" {
                "msg-critical"
            } else {
                match topic.as_str() {
                    "жидкое"       => "msg-liquid",
                    "твёрдое"      => "msg-solid",
                    "газообразное" => "msg-gas",
                    _              => "msg-server",
                }
            };
            log(&format!("[#{seq}] {payload}"), class);
        }
        Ok(ServerMsg::Echo { payload })   => log(&payload,          "msg-echo"),
        Ok(ServerMsg::Pong)               => log("🏓 pong",         "msg-system"),
        Ok(ServerMsg::Error { msg })      => log(&format!("❌ {msg}"), "msg-error"),
        Err(e) => {
            log(&format!("⚠️ Неизвестное сообщение: {e}"), "msg-error");
        }
    }
}

// ── Публичный API ─────────────────────────────────────────────────────────────

#[wasm_bindgen(start)]
pub fn start() {
    // Не подключаемся автоматически — URL задаётся из index.html
}

#[wasm_bindgen]
pub fn connect(url: &str) -> Result<(), JsValue> {
    web_sys::console::log_1(&format!("💩 Коннект: {url}").into());

    let ws = WebSocket::new(url)?;
    set_status("⏳", "подключаемся...", "status-connecting");

    {
        let cb = Closure::wrap(Box::new(move |_: JsValue| {
            set_status("⏳", "ожидаем токен...", "status-connecting");
            log("🔗 Соединение установлено, ожидаем auth challenge...", "msg-system");
        }) as Box<dyn FnMut(JsValue)>);
        ws.set_onopen(Some(cb.as_ref().unchecked_ref()));
        cb.forget();
    }

    {
        let cb = Closure::wrap(Box::new(move |e: MessageEvent| {
            if let Some(text) = e.data().as_string() {
                handle_server_msg(&text);
            }
        }) as Box<dyn FnMut(MessageEvent)>);
        ws.set_onmessage(Some(cb.as_ref().unchecked_ref()));
        cb.forget();
    }

    {
        let cb = Closure::wrap(Box::new(move |e: ErrorEvent| {
            set_status("🔴", "ошибка", "status-error");
            log(&format!("❌ WS ошибка: {}", e.message()), "msg-error");
        }) as Box<dyn FnMut(ErrorEvent)>);
        ws.set_onerror(Some(cb.as_ref().unchecked_ref()));
        cb.forget();
    }

    {
        let cb = Closure::wrap(Box::new(move |e: CloseEvent| {
            set_status("🔴", "отключено", "status-error");
            log(&format!("🚪 Отключено (code={})", e.code()), "msg-system");
            show_el("auth-overlay", true);
            show_el("main-ui", false);
            for id in &["btn-send","btn-liquid","btn-solid","btn-gas","btn-critical","btn-unsub"] {
                set_btn(id, false);
            }
            set_sub_label(None);
        }) as Box<dyn FnMut(CloseEvent)>);
        ws.set_onclose(Some(cb.as_ref().unchecked_ref()));
        cb.forget();
    }

    WS.with(|cell| *cell.borrow_mut() = Some(ws));
    Ok(())
}

fn ws_send_json(cmd: &ClientCmd) {
    if let Ok(json) = serde_json::to_string(cmd) {
        WS.with(|cell| {
            if let Some(ws) = cell.borrow().as_ref() {
                let _ = ws.send_with_str(&json);
            }
        });
    }
}

#[wasm_bindgen]
pub fn auth(token: &str) {
    ws_send_json(&ClientCmd::Auth { token });
}

#[wasm_bindgen]
pub fn subscribe(topic: &str) {
    log(&format!("➡️  подписываемся на «{topic}»..."), "msg-system");
    ws_send_json(&ClientCmd::Subscribe { topic });
}

#[wasm_bindgen]
pub fn unsubscribe() {
    ws_send_json(&ClientCmd::Unsubscribe);
}

#[wasm_bindgen]
pub fn send_echo(text: &str) {
    if text.trim().is_empty() { return; }
    log(&format!("➡️  ты: {text}"), "msg-client");
    ws_send_json(&ClientCmd::Echo { text });
}

#[wasm_bindgen]
pub fn ping() {
    ws_send_json(&ClientCmd::Ping);
}

#[wasm_bindgen]
pub fn disconnect() -> Result<(), JsValue> {
    WS.with(|cell| -> Result<(), JsValue> {
        if let Some(ws) = cell.borrow().as_ref() { ws.close()?; }
        Ok(())
    })
}
