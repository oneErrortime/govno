// ╔══════════════════════════════════════════════════════════════════════╗
// ║   💩  ГОВНО-КЛИЕНТ  v0.2.0  —  100% Rust → WASM                    ║
// ║   Pub/Sub по топикам, JSON протокол, ни строчки JS бизнес-логики     ║
// ╚══════════════════════════════════════════════════════════════════════╝

use std::cell::RefCell;

use js_sys::Date;
use serde::{Deserialize, Serialize};
use wasm_bindgen::prelude::*;
use wasm_bindgen::JsCast;
use web_sys::{CloseEvent, ErrorEvent, MessageEvent, WebSocket};

// ── Протокол (зеркало серверных типов) ───────────────────────────────────────

#[derive(Serialize)]
#[serde(tag = "cmd", rename_all = "snake_case")]
enum ClientCmd<'a> {
    Subscribe   { topic: &'a str },
    Unsubscribe,
    Echo        { text: &'a str },
    Ping,
}

#[derive(Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
enum ServerMsg {
    Welcome     { msg: String },
    Subscribed  { topic: String },
    Unsubscribed,
    // ShitMessage инлайнится в тег
    Shit        { topic: String, seq: u64, payload: String },
    Echo        { payload: String },
    Pong,
    Error       { msg: String },
}

// ── Глобальное состояние ──────────────────────────────────────────────────────

thread_local! {
    static WS: RefCell<Option<WebSocket>> = const { RefCell::new(None) };
}

// ── Хелперы DOM ───────────────────────────────────────────────────────────────

fn document() -> web_sys::Document {
    web_sys::window().unwrap().document().unwrap()
}

fn now_str() -> String {
    let d = Date::new_0();
    format!(
        "{:02}:{:02}:{:02}",
        d.get_hours(),
        d.get_minutes(),
        d.get_seconds()
    )
}

fn log(text: &str, class: &str) {
    let doc = document();
    let Some(log_el) = doc.get_element_by_id("log") else { return };
    let p = doc.create_element("p").unwrap();
    p.set_class_name(class);
    p.set_text_content(Some(&format!("[{}] {}", now_str(), text)));
    log_el.append_child(&p).unwrap();
    log_el.set_scroll_top(log_el.scroll_height());
}

fn set_status(icon: &str, text: &str, class: &str) {
    if let Some(el) = document().get_element_by_id("status") {
        el.set_class_name(class);
        el.set_text_content(Some(&format!("{icon} {text}")));
    }
}

fn set_sub_label(topic: Option<&str>) {
    if let Some(el) = document().get_element_by_id("sub-label") {
        match topic {
            Some(t) => el.set_text_content(Some(&format!("подписан: {t}"))),
            None    => el.set_text_content(Some("не подписан")),
        }
    }
}

fn set_btn_enabled(id: &str, enabled: bool) {
    let doc = document();
    if let Some(btn) = doc.get_element_by_id(id) {
        if enabled {
            btn.remove_attribute("disabled").unwrap();
        } else {
            btn.set_attribute("disabled", "").unwrap();
        }
    }
}

// ── Обработка входящих сообщений ──────────────────────────────────────────────

fn handle_server_msg(text: &str) {
    match serde_json::from_str::<ServerMsg>(text) {
        Ok(ServerMsg::Welcome { msg }) => {
            log(&msg, "msg-system");
        }
        Ok(ServerMsg::Subscribed { topic }) => {
            log(&format!("📬 Подписка на «{}» оформлена", topic), "msg-system");
            set_sub_label(Some(&topic));
        }
        Ok(ServerMsg::Unsubscribed) => {
            log("📭 Отписались от топика", "msg-system");
            set_sub_label(None);
        }
        Ok(ServerMsg::Shit { topic, seq, payload }) => {
            let class = match topic.as_str() {
                "жидкое"       => "msg-liquid",
                "твёрдое"      => "msg-solid",
                "газообразное" => "msg-gas",
                _              => "msg-server",
            };
            log(&format!("[#{seq}] {payload}"), class);
        }
        Ok(ServerMsg::Echo { payload }) => {
            log(&payload, "msg-echo");
        }
        Ok(ServerMsg::Pong) => {
            log("🏓 pong", "msg-system");
        }
        Ok(ServerMsg::Error { msg }) => {
            log(&format!("❌ Ошибка сервера: {msg}"), "msg-error");
        }
        Err(e) => {
            log(&format!("⚠️ Неизвестное сообщение: {e}"), "msg-error");
            web_sys::console::warn_1(&format!("raw: {text}").into());
        }
    }
}

// ── Публичный API (экспортируется в JS) ───────────────────────────────────────

#[wasm_bindgen(start)]
pub fn start() -> Result<(), JsValue> {
    connect("ws://127.0.0.1:3000/ws")
}

#[wasm_bindgen]
pub fn connect(url: &str) -> Result<(), JsValue> {
    web_sys::console::log_1(&format!("💩 Коннект: {url}").into());

    let ws = WebSocket::new(url)?;
    set_status("⏳", "подключаемся...", "status-connecting");

    // onopen
    {
        let cb = Closure::wrap(Box::new(move |_: JsValue| {
            set_status("🟢", "подключено", "status-ok");
            log("✅ Соединение установлено", "msg-system");
            set_btn_enabled("btn-send", true);
            // Топик-кнопки
            set_btn_enabled("btn-liquid", true);
            set_btn_enabled("btn-solid",  true);
            set_btn_enabled("btn-gas",    true);
            set_btn_enabled("btn-unsub",  true);
        }) as Box<dyn FnMut(JsValue)>);
        ws.set_onopen(Some(cb.as_ref().unchecked_ref()));
        cb.forget();
    }

    // onmessage
    {
        let cb = Closure::wrap(Box::new(move |e: MessageEvent| {
            if let Some(text) = e.data().as_string() {
                handle_server_msg(&text);
            }
        }) as Box<dyn FnMut(MessageEvent)>);
        ws.set_onmessage(Some(cb.as_ref().unchecked_ref()));
        cb.forget();
    }

    // onerror
    {
        let cb = Closure::wrap(Box::new(move |e: ErrorEvent| {
            set_status("🔴", "ошибка", "status-error");
            log(&format!("❌ WS ошибка: {}", e.message()), "msg-error");
        }) as Box<dyn FnMut(ErrorEvent)>);
        ws.set_onerror(Some(cb.as_ref().unchecked_ref()));
        cb.forget();
    }

    // onclose
    {
        let cb = Closure::wrap(Box::new(move |e: CloseEvent| {
            set_status("🔴", "отключено", "status-error");
            log(&format!("🚪 Отключено (code={})", e.code()), "msg-system");
            set_btn_enabled("btn-send",   false);
            set_btn_enabled("btn-liquid", false);
            set_btn_enabled("btn-solid",  false);
            set_btn_enabled("btn-gas",    false);
            set_btn_enabled("btn-unsub",  false);
            set_sub_label(None);
        }) as Box<dyn FnMut(CloseEvent)>);
        ws.set_onclose(Some(cb.as_ref().unchecked_ref()));
        cb.forget();
    }

    WS.with(|cell| *cell.borrow_mut() = Some(ws));
    Ok(())
}

fn ws_send_json(cmd: &ClientCmd) {
    let json = serde_json::to_string(cmd).unwrap_or_default();
    WS.with(|cell| {
        if let Some(ws) = cell.borrow().as_ref() {
            let _ = ws.send_with_str(&json);
        }
    });
}

#[wasm_bindgen]
pub fn subscribe(topic: &str) {
    log(&format!("➡️  подписываемся на «{topic}»..."), "msg-system");
    ws_send_json(&ClientCmd::Subscribe { topic });
}

#[wasm_bindgen]
pub fn unsubscribe() {
    log("➡️  отписываемся...", "msg-system");
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
        if let Some(ws) = cell.borrow().as_ref() {
            ws.close()?;
        }
        Ok(())
    })
}
