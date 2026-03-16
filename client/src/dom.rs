//! DOM manipulation helpers

use wasm_bindgen::{closure::Closure, JsCast};
use web_sys::{Element};
use crate::state;

pub fn document() -> web_sys::Document {
    web_sys::window().unwrap().document().unwrap()
}

pub fn window() -> web_sys::Window {
    web_sys::window().unwrap()
}

// ── Status bar ────────────────────────────────────────────────────────────────

pub fn set_status(icon: &str, text: &str, class: &str) {
    let doc = document();
    if let Some(el) = doc.get_element_by_id("status") {
        el.set_class_name(class);
        el.set_text_content(Some(&format!("{icon} {text}")));
    }
}

pub fn update_status_from_state() {
    let (icon, label, class) = state::get(|s| (
        s.connection_state.icon(),
        s.connection_state.label(),
        s.connection_state.css_class(),
    ));
    set_status(icon, label, class);
}

// ── Button helpers ────────────────────────────────────────────────────────────

pub fn set_btn(id: &str, enabled: bool) {
    let doc = document();
    if let Some(el) = doc.get_element_by_id(id) {
        if enabled { let _ = el.remove_attribute("disabled"); }
        else       { let _ = el.set_attribute("disabled", ""); }
    }
}

pub fn show_el(id: &str, visible: bool) {
    let doc = document();
    if let Some(el) = doc.get_element_by_id(id) {
        let _ = el.set_attribute("style", if visible { "" } else { "display:none" });
    }
}

pub fn set_text(id: &str, text: &str) {
    let doc = document();
    if let Some(el) = doc.get_element_by_id(id) {
        el.set_text_content(Some(text));
    }
}

// ── Log ───────────────────────────────────────────────────────────────────────

pub fn log_msg(text: &str, class: &str) {
    let doc = document();
    let Some(log_el) = doc.get_element_by_id("log") else { return };

    let p = doc.create_element("p").unwrap();
    p.set_class_name(class);

    // Timestamp
    let now = js_sys::Date::new_0();
    let ts = format!("{:02}:{:02}:{:02}",
        now.get_hours(), now.get_minutes(), now.get_seconds());
    p.set_text_content(Some(&format!("[{ts}] {text}")));

    log_el.append_child(&p).unwrap();
    log_el.set_scroll_top(log_el.scroll_height());

    // Trim DOM to 300 visible entries
    let children = log_el.children();
    if children.length() > 300 {
        if let Some(first) = children.item(0) {
            let _ = log_el.remove_child(&first);
        }
    }
}

// ── Topic buttons ─────────────────────────────────────────────────────────────

pub fn update_topic_buttons() {
    let subs = state::get(|s| s.subscriptions.clone());
    let topics = [("liquid", "btn-topic-liquid"),
                  ("solid",  "btn-topic-solid"),
                  ("gas",    "btn-topic-gas"),
                  ("critical", "btn-topic-critical")];
    let doc = document();
    for (topic, btn_id) in &topics {
        if let Some(el) = doc.get_element_by_id(btn_id) {
            if subs.contains(*topic) {
                el.set_attribute("data-active", "1").unwrap();
            } else {
                let _ = el.remove_attribute("data-active");
            }
        }
    }
}

// ── UI panels ─────────────────────────────────────────────────────────────────

pub fn enable_main_ui(session_id: &str) {
    show_el("auth-overlay", false);
    show_el("main-ui", true);
    set_text("session-id", session_id);
    for id in &["btn-send", "btn-topic-liquid", "btn-topic-solid",
                "btn-topic-gas", "btn-topic-critical",
                "btn-sub-all", "btn-unsub-all", "btn-ping"] {
        set_btn(id, true);
    }
}

pub fn disable_main_ui() {
    show_el("auth-overlay", true);
    show_el("main-ui", false);
    for id in &["btn-send", "btn-topic-liquid", "btn-topic-solid",
                "btn-topic-gas", "btn-topic-critical",
                "btn-sub-all", "btn-unsub-all", "btn-ping"] {
        set_btn(id, false);
    }
}

// ── Service registry ──────────────────────────────────────────────────────────

pub fn update_services_panel() {
    let services = state::get(|s| s.services.clone());
    let doc = document();
    let Some(container) = doc.get_element_by_id("services-panel") else { return };

    if services.is_empty() {
        container.set_inner_html(
            r#"<p class="msg-system">— нет подключённых воркеров —</p>"#
        );
        return;
    }

    let mut html = String::new();
    for svc in &services {
        let topic_emoji = match svc.topic.as_str() {
            "liquid"   => "💧",
            "solid"    => "🧱",
            "gas"      => "💨",
            "critical" => "🚨",
            _          => "💩",
        };
        let rate = format!("{:.1}/s", 1000.0 / svc.interval_ms as f64);
        html.push_str(&format!(
            r#"<div class="service-card topic-{topic}">
                <div class="svc-name">{emoji} {sid}</div>
                <div class="svc-meta">{topic} · {rate} · {msgs} sent · v{ver}</div>
               </div>"#,
            topic  = svc.topic,
            emoji  = topic_emoji,
            sid    = svc.service_id,
            rate   = rate,
            msgs   = svc.messages_sent,
            ver    = svc.version,
        ));
    }
    container.set_inner_html(&html);
}

// ── Rate display ──────────────────────────────────────────────────────────────

pub fn update_rate_display() {
    let (total_rate, topic_rates) = state::get(|s| {
        let total = s.rates.get("__total__").map(|r| r.rate_per_sec()).unwrap_or(0.0);
        let topics: Vec<(String, f64)> = ["liquid","solid","gas","critical"]
            .iter()
            .map(|t| (t.to_string(), s.rates.get(*t).map(|r| r.rate_per_sec()).unwrap_or(0.0)))
            .collect();
        (total, topics)
    });

    set_text("rate-total", &format!("{total_rate:.1}/s"));
    for (topic, rate) in &topic_rates {
        set_text(&format!("rate-{topic}"), &format!("{rate:.1}/s"));
    }
}

pub fn update_session_stats() {
    let (total, uptime) = state::get(|s| (s.total_received, s.session_uptime_secs()));
    set_text("stat-total", &total.to_string());
    let h = uptime / 3600;
    let m = (uptime % 3600) / 60;
    let s = uptime % 60;
    set_text("stat-uptime", &format!("{h:02}:{m:02}:{s:02}"));
}

// ── Toast (critical alerts) ───────────────────────────────────────────────────

pub fn show_toast(text: &str) {
    let doc = document();
    let Some(toast) = doc.get_element_by_id("toast") else { return };
    toast.set_text_content(Some(text));
    let _ = toast.set_attribute("class", "toast toast-visible");

    // Auto-hide after 4s
    let toast_el = toast.clone();
    let cb = Closure::once(move || {
        let _ = toast_el.set_attribute("class", "toast");
    });
    let _ = window().set_timeout_with_callback_and_timeout_and_arguments_0(
        cb.as_ref().unchecked_ref(),
        4000,
    );
    cb.forget();
}

// ── 1-second tick install ─────────────────────────────────────────────────────

pub fn install_tick() {
    let cb = Closure::wrap(Box::new(|| {
        crate::tick();
    }) as Box<dyn Fn()>);

    let _ = window().set_interval_with_callback_and_timeout_and_arguments_0(
        cb.as_ref().unchecked_ref(),
        1000,
    );
    cb.forget();
}
