// ╔══════════════════════════════════════════════╗
// ║   💩  ГОВНО-КЛИЕНТ  —  100% Rust → WASM      ║
// ║   ни одной строчки JS/TS, только хардкор      ║
// ╚══════════════════════════════════════════════╝

use std::cell::RefCell;

use js_sys::Date;
use wasm_bindgen::prelude::*;
use wasm_bindgen::JsCast;
use web_sys::{CloseEvent, ErrorEvent, MessageEvent, WebSocket};

// Глобальное состояние WebSocket.
// В WASM всё однопоточное — RefCell достаточно.
thread_local! {
    static WS: RefCell<Option<WebSocket>> = const { RefCell::new(None) };
}

// ── Хелперы DOM ──────────────────────────────────────────────────────────────

fn document() -> web_sys::Document {
    web_sys::window().unwrap().document().unwrap()
}

/// Добавляет строчку в #log с меткой времени
fn log_message(text: &str, class: &str) {
    let doc = document();

    let Some(log) = doc.get_element_by_id("log") else {
        return;
    };

    let p = doc.create_element("p").unwrap();
    p.set_class_name(class);

    let time = {
        let d = Date::new_0();
        format!(
            "{:02}:{:02}:{:02}",
            d.get_hours(),
            d.get_minutes(),
            d.get_seconds()
        )
    };

    p.set_text_content(Some(&format!("[{}] {}", time, text)));

    // scroll вниз
    log.append_child(&p).unwrap();
    log.set_scroll_top(log.scroll_height());
}

fn set_status(icon: &str, text: &str, class: &str) {
    let doc = document();
    if let Some(el) = doc.get_element_by_id("status") {
        el.set_class_name(class);
        el.set_text_content(Some(&format!("{} {}", icon, text)));
    }
}

// ── Точка входа — вызывается при загрузке WASM ───────────────────────────────

#[wasm_bindgen(start)]
pub fn start() -> Result<(), JsValue> {
    connect("ws://127.0.0.1:3000/ws")
}

/// Устанавливает WebSocket-соединение с указанным URL.
/// Экспортируется для возможности переподключения из HTML.
#[wasm_bindgen]
pub fn connect(url: &str) -> Result<(), JsValue> {
    web_sys::console::log_1(&format!("💩 Коннектимся к {}", url).into());

    let ws = WebSocket::new(url)?;
    ws.set_binary_type(web_sys::BinaryType::Arraybuffer);

    set_status("⏳", "подключаемся...", "status-connecting");

    // ── onopen ────────────────────────────────────────────────────────────────
    {
        let cb = Closure::wrap(Box::new(move |_: JsValue| {
            web_sys::console::log_1(&"💩 WS открыт".into());
            set_status("🟢", "подключено", "status-ok");
            log_message("✅ Соединение с говно-сервером установлено!", "msg-system");

            // Включаем кнопку
            let doc = document();
            if let Some(btn) = doc.get_element_by_id("btn-send") {
                btn.remove_attribute("disabled").unwrap();
            }
        }) as Box<dyn FnMut(JsValue)>);
        ws.set_onopen(Some(cb.as_ref().unchecked_ref()));
        cb.forget(); // отдаём владение JS GC
    }

    // ── onmessage ─────────────────────────────────────────────────────────────
    {
        let cb = Closure::wrap(Box::new(move |e: MessageEvent| {
            if let Some(text) = e.data().as_string() {
                web_sys::console::log_1(&format!("📨 {}", text).into());
                log_message(&text, "msg-server");
            }
        }) as Box<dyn FnMut(MessageEvent)>);
        ws.set_onmessage(Some(cb.as_ref().unchecked_ref()));
        cb.forget();
    }

    // ── onerror ───────────────────────────────────────────────────────────────
    {
        let cb = Closure::wrap(Box::new(move |e: ErrorEvent| {
            let msg = e.message();
            web_sys::console::error_1(&format!("💥 WS error: {}", msg).into());
            set_status("🔴", "ошибка", "status-error");
            log_message(&format!("❌ Ошибка: {}", msg), "msg-error");
        }) as Box<dyn FnMut(ErrorEvent)>);
        ws.set_onerror(Some(cb.as_ref().unchecked_ref()));
        cb.forget();
    }

    // ── onclose ───────────────────────────────────────────────────────────────
    {
        let cb = Closure::wrap(Box::new(move |e: CloseEvent| {
            web_sys::console::log_1(&"🚪 WS закрыт".into());
            set_status("🔴", "отключено", "status-error");
            log_message(
                &format!("🚪 Соединение закрыто (code={})", e.code()),
                "msg-system",
            );
            // Дизейблим кнопку
            let doc = document();
            if let Some(btn) = doc.get_element_by_id("btn-send") {
                btn.set_attribute("disabled", "").unwrap();
            }
        }) as Box<dyn FnMut(CloseEvent)>);
        ws.set_onclose(Some(cb.as_ref().unchecked_ref()));
        cb.forget();
    }

    // Сохраняем WS в thread-local
    WS.with(|cell| {
        *cell.borrow_mut() = Some(ws);
    });

    Ok(())
}

/// Отправляет текстовое сообщение на сервер.
/// Вызывается из `<script type="module">` в index.html.
#[wasm_bindgen]
pub fn send_message(msg: &str) -> Result<(), JsValue> {
    if msg.trim().is_empty() {
        return Ok(());
    }

    WS.with(|cell| -> Result<(), JsValue> {
        match cell.borrow().as_ref() {
            Some(ws) => {
                log_message(&format!("➡️  ты: {}", msg), "msg-client");
                ws.send_with_str(msg)?;
                Ok(())
            }
            None => {
                log_message("⚠️ Нет соединения!", "msg-error");
                Ok(())
            }
        }
    })
}

/// Закрывает соединение — для кнопки «Отключиться»
#[wasm_bindgen]
pub fn disconnect() -> Result<(), JsValue> {
    WS.with(|cell| -> Result<(), JsValue> {
        if let Some(ws) = cell.borrow().as_ref() {
            ws.close()?;
        }
        Ok(())
    })
}
