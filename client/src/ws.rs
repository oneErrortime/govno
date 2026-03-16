//! WebSocket lifecycle — connect, send, dispatch incoming messages

use std::cell::RefCell;
use wasm_bindgen::{closure::Closure, JsCast, JsValue};
use web_sys::{CloseEvent, ErrorEvent, MessageEvent, WebSocket};
use serde::Serialize;

use crate::{audio, dom, protocol::{self, ServerMsg}, state};

// ── Global WebSocket handle ───────────────────────────────────────────────────

thread_local! {
    static WS: RefCell<Option<WebSocket>> = const { RefCell::new(None) };
}

fn ws_send_text(text: &str) {
    WS.with(|cell| {
        if let Some(ws) = cell.borrow().as_ref() {
            let _ = ws.send_with_str(text);
        }
    });
}

pub fn send_cmd(cmd: &protocol::ClientCmd) {
    if let Ok(json) = serde_json::to_string(cmd) {
        ws_send_text(&json);
    }
}

pub fn disconnect() {
    WS.with(|cell| {
        if let Some(ws) = cell.borrow().as_ref() {
            let _ = ws.close();
        }
    });
}

// ── Connect ───────────────────────────────────────────────────────────────────

pub fn connect(url: &str) {
    web_sys::console::log_1(&format!("💩 WS connect: {url}").into());

    let ws = match WebSocket::new(url) {
        Ok(ws) => ws,
        Err(e) => {
            dom::log_msg(&format!("❌ Не могу создать WS: {:?}", e), "msg-error");
            dom::set_status("🔴", "ошибка", "status-error");
            return;
        }
    };

    state::get(|s| {
        s.connection_state = state::ConnectionState::Connecting { url: url.into() };
        s.ws_url = url.into();
    });
    dom::update_status_from_state();

    setup_callbacks(&ws, url.to_string());

    WS.with(|cell| *cell.borrow_mut() = Some(ws));
}

fn setup_callbacks(ws: &WebSocket, url: String) {
    // ── onopen ────────────────────────────────────────────────────────────────
    {
        let cb = Closure::wrap(Box::new(move |_: JsValue| {
            web_sys::console::log_1(&"💩 WS open".into());
            state::get(|s| {
                s.connection_state = state::ConnectionState::Authenticating;
                s.reconnect_attempt = 0;
            });
            dom::update_status_from_state();
            dom::log_msg("🔗 Соединение установлено, ждём auth challenge...", "msg-system");
        }) as Box<dyn FnMut(JsValue)>);
        ws.set_onopen(Some(cb.as_ref().unchecked_ref()));
        cb.forget();
    }

    // ── onmessage ─────────────────────────────────────────────────────────────
    {
        let cb = Closure::wrap(Box::new(move |e: MessageEvent| {
            if let Some(text) = e.data().as_string() {
                dispatch_message(&text);
            }
        }) as Box<dyn FnMut(MessageEvent)>);
        ws.set_onmessage(Some(cb.as_ref().unchecked_ref()));
        cb.forget();
    }

    // ── onerror ───────────────────────────────────────────────────────────────
    {
        let cb = Closure::wrap(Box::new(move |e: ErrorEvent| {
            let msg = e.message();
            web_sys::console::error_1(&format!("💥 WS error: {msg}").into());
            dom::log_msg(&format!("❌ WS ошибка: {msg}"), "msg-error");
            dom::set_status("🔴", "ошибка", "status-error");
        }) as Box<dyn FnMut(ErrorEvent)>);
        ws.set_onerror(Some(cb.as_ref().unchecked_ref()));
        cb.forget();
    }

    // ── onclose ───────────────────────────────────────────────────────────────
    {
        let url_clone = url.clone();
        let cb = Closure::wrap(Box::new(move |e: CloseEvent| {
            let code = e.code();
            web_sys::console::log_1(&format!("🚪 WS closed code={code}").into());
            dom::log_msg(&format!("🚪 Отключено (code={code})"), "msg-system");
            dom::disable_main_ui();
            state::get(|s| {
                s.connection_state = state::ConnectionState::Disconnected;
                s.subscriptions.clear();
            });
            dom::update_status_from_state();
            dom::update_topic_buttons();

            // Auto-reconnect for non-normal closes (1000 = normal)
            if code != 1000 {
                schedule_reconnect(url_clone.clone());
            }
        }) as Box<dyn FnMut(CloseEvent)>);
        ws.set_onclose(Some(cb.as_ref().unchecked_ref()));
        cb.forget();
    }
}

// ── Auto-reconnect ────────────────────────────────────────────────────────────

fn schedule_reconnect(url: String) {
    let attempt = state::get(|s| {
        s.reconnect_attempt += 1;
        s.reconnect_attempt
    });

    let delay_ms = (2u64.pow(attempt.min(6)) * 1000).min(60_000);
    let delay_secs = delay_ms / 1000;

    state::get(|s| {
        s.connection_state = state::ConnectionState::Reconnecting {
            attempt,
            delay_secs,
        };
    });
    dom::update_status_from_state();
    dom::log_msg(
        &format!("🔁 Переподключение через {delay_secs}s (попытка {attempt})..."),
        "msg-system",
    );

    let cb = Closure::once(move || {
        let url_clone = url.clone();
        connect(&url_clone);
        // If we have a stored token, send auth automatically
        let token = state::get(|s| s.token.clone());
        if !token.is_empty() {
            // Small delay to let WS handshake complete
            let cb2 = Closure::once(move || {
                send_cmd(&protocol::ClientCmd::Auth { token });
            });
            let _ = web_sys::window().unwrap()
                .set_timeout_with_callback_and_timeout_and_arguments_0(
                    cb2.as_ref().unchecked_ref(), 800,
                );
            cb2.forget();
        }
    });

    let _ = web_sys::window().unwrap()
        .set_timeout_with_callback_and_timeout_and_arguments_0(
            cb.as_ref().unchecked_ref(),
            delay_ms as i32,
        );
    cb.forget();
}

// ── Message dispatch ──────────────────────────────────────────────────────────

fn dispatch_message(text: &str) {
    match serde_json::from_str::<ServerMsg>(text) {

        Ok(ServerMsg::AuthRequired { msg }) => {
            dom::log_msg(&msg, "msg-system");
        }

        Ok(ServerMsg::Authorized { puk, session_id }) => {
            dom::log_msg(&puk, "msg-puk");
            state::get(|s| {
                s.connection_state = state::ConnectionState::Connected {
                    session_id: session_id.clone(),
                };
                s.pending_auth = false;
            });
            dom::update_status_from_state();
            dom::enable_main_ui(&session_id);
        }

        Ok(ServerMsg::Unauthorized { msg }) => {
            dom::log_msg(&msg, "msg-error");
            dom::set_status("🔴", "отказано", "status-error");
            dom::set_btn("btn-auth", true);
            state::get(|s| {
                s.connection_state = state::ConnectionState::Disconnected;
            });
        }

        Ok(ServerMsg::Welcome { msg }) => {
            dom::log_msg(&msg, "msg-system");
        }

        Ok(ServerMsg::Subscribed { topic }) => {
            dom::log_msg(&format!("📬 Подписка на «{}» активна", topic), "msg-system");
            dom::update_topic_buttons();
        }

        Ok(ServerMsg::Unsubscribed { topic }) => {
            dom::log_msg(&format!("📭 Отписались от «{}»", topic), "msg-system");
            dom::update_topic_buttons();
        }

        Ok(ServerMsg::UnsubscribedAll) => {
            dom::log_msg("📭 Отписались от всех топиков", "msg-system");
            dom::update_topic_buttons();
        }

        Ok(ServerMsg::Shit(payload)) => {
            handle_shit_message(payload);
        }

        Ok(ServerMsg::ServiceList { services }) => {
            state::get(|s| s.services = services);
            dom::update_services_panel();
        }

        Ok(ServerMsg::Echo { payload }) => {
            dom::log_msg(&payload, "msg-echo");
        }

        Ok(ServerMsg::Pong) => {
            dom::log_msg("🏓 pong", "msg-system");
        }

        Ok(ServerMsg::Error { msg }) => {
            dom::log_msg(&format!("❌ Ошибка сервера: {msg}"), "msg-error");
        }

        Err(e) => {
            dom::log_msg(&format!("⚠️ Неизвестное сообщение: {e}"), "msg-error");
            web_sys::console::warn_1(&format!("raw: {text}").into());
        }
    }
}

fn handle_shit_message(payload: protocol::ShitPayload) {
    let is_critical = payload.priority == "critical";
    let topic       = payload.topic.clone();

    // Record in state
    let entry = payload.to_history_entry();
    state::get(|s| s.record_message(&entry));

    // CSS class for log entry
    let class = if is_critical {
        "msg-critical"
    } else {
        match topic.as_str() {
            "liquid"   => "msg-liquid",
            "solid"    => "msg-solid",
            "gas"      => "msg-gas",
            "critical" => "msg-critical",
            _          => "msg-server",
        }
    };

    // Tags formatted
    let tags_str = if payload.tags.is_empty() {
        String::new()
    } else {
        format!(" [{}]", payload.tags.join(","))
    };

    let text = format!(
        "[#{seq}] {payload}{tags} ← {svc}",
        seq     = payload.seq,
        payload = payload.payload,
        tags    = tags_str,
        svc     = payload.service_id,
    );

    dom::log_msg(&text, class);

    // Critical: toast + audio
    if is_critical {
        dom::show_toast(&format!("🚨 {}", payload.payload));
        audio::beep_critical();
    }
}
