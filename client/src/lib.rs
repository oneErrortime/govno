//! 💩 ГОВНО-КЛИЕНТ v0.3.0 — 100% Rust → WebAssembly

mod audio;
mod canvas;
mod dom;
mod protocol;
mod state;
mod ws;

use wasm_bindgen::prelude::*;

#[wasm_bindgen(start)]
pub fn start() {
    std::panic::set_hook(Box::new(|info| {
        web_sys::console::error_1(&format!("💩 PANIC: {info}").into());
    }));
    dom::install_tick();
}

#[wasm_bindgen]
pub fn connect(url: &str) { ws::connect(url); }

#[wasm_bindgen]
pub fn auth(token: &str) {
    state::get(|s| s.token = token.into());
    ws::send_cmd(&protocol::ClientCmd::Auth { token: token.into() });
    state::get(|s| s.pending_auth = true);
}

#[wasm_bindgen]
pub fn toggle_topic(topic: &str) -> bool {
    let subscribed = state::get(|s| s.subscriptions.contains(topic));
    if subscribed {
        ws::send_cmd(&protocol::ClientCmd::Unsubscribe { topic: topic.into() });
        state::get(|s| { s.subscriptions.remove(topic); });
    } else {
        ws::send_cmd(&protocol::ClientCmd::Subscribe { topic: topic.into() });
        state::get(|s| { s.subscriptions.insert(topic.into()); });
    }
    dom::update_topic_buttons();
    !subscribed
}

#[wasm_bindgen]
pub fn subscribe_all() {
    for t in &["liquid", "solid", "gas", "critical"] {
        let already = state::get(|s| s.subscriptions.contains(*t));
        if !already {
            ws::send_cmd(&protocol::ClientCmd::Subscribe { topic: t.to_string() });
            state::get(|s| { s.subscriptions.insert(t.to_string()); });
        }
    }
    dom::update_topic_buttons();
}

#[wasm_bindgen]
pub fn unsubscribe_all() {
    ws::send_cmd(&protocol::ClientCmd::UnsubscribeAll);
    state::get(|s| s.subscriptions.clear());
    dom::update_topic_buttons();
}

#[wasm_bindgen]
pub fn send_echo(text: &str) {
    if text.trim().is_empty() { return; }
    dom::log_msg(&format!("➡️  ты: {text}"), "msg-client");
    ws::send_cmd(&protocol::ClientCmd::Echo { text: text.into() });
}

#[wasm_bindgen]
pub fn ping() { ws::send_cmd(&protocol::ClientCmd::Ping); }

#[wasm_bindgen]
pub fn disconnect() {
    ws::disconnect();
    state::get(|s| s.connection_state = state::ConnectionState::Disconnected);
    dom::set_status("🔴", "отключено", "status-error");
}

#[wasm_bindgen]
pub fn clear_log() {
    state::get(|s| s.message_history.clear());
    if let Some(doc) = web_sys::window().and_then(|w| w.document()) {
        if let Some(el) = doc.get_element_by_id("log") {
            el.set_inner_html("");
        }
    }
    dom::log_msg("── лог очищен ──", "msg-system");
}

#[wasm_bindgen]
pub fn export_log() {
    let history = state::get(|s| s.message_history.clone());
    let json = serde_json::to_string_pretty(&history).unwrap_or_default();

    let parts = js_sys::Array::new();
    parts.push(&js_sys::JsString::from(json.as_str()).into());

    // BlobPropertyBag::set_type() is the non-deprecated replacement for type_()
    let mut opts = web_sys::BlobPropertyBag::new();
    opts.set_type("application/json");

    let blob = web_sys::Blob::new_with_str_sequence_and_options(&parts, &opts).ok();
    if let Some(blob) = blob {
        if let Some(url) = web_sys::Url::create_object_url_with_blob(&blob).ok() {
            if let Some(doc) = web_sys::window().and_then(|w| w.document()) {
                if let Ok(el) = doc.create_element("a") {
                    if let Ok(a) = el.dyn_into::<web_sys::HtmlAnchorElement>() {
                        a.set_href(&url);
                        a.set_download("govno-log.json");
                        a.click();
                        let _ = web_sys::Url::revoke_object_url(&url);
                    }
                }
            }
        }
    }
}

#[wasm_bindgen]
pub fn tick() {
    state::get(|s| s.tick());
    canvas::redraw_all();
    dom::update_rate_display();
    dom::update_session_stats();
}

#[wasm_bindgen]
pub fn set_log_filter(query: &str) {
    let q = query.to_lowercase();
    if let Some(doc) = web_sys::window().and_then(|w| w.document()) {
        if let Some(log) = doc.get_element_by_id("log") {
            // Use HtmlCollection (requires "HtmlCollection" feature in web-sys)
            let children = log.children();
            for i in 0..children.length() {
                if let Some(child) = children.item(i) {
                    let text = child.text_content().unwrap_or_default().to_lowercase();
                    let vis = q.is_empty() || text.contains(&q);
                    let _ = child.set_attribute("style", if vis { "" } else { "display:none" });
                }
            }
        }
    }
}

#[wasm_bindgen]
pub fn toggle_sound() -> bool {
    state::get(|s| { s.sound_enabled = !s.sound_enabled; s.sound_enabled })
}
