//! Sparkline charts via CanvasRenderingContext2d
//!
//! Renders a 30-second rolling message-rate histogram for each topic.
//! Uses `#[allow(deprecated)]` on set_fill_style/set_stroke_style because
//! web-sys 0.3.x has them deprecated in favour of the Canvas2dContext API
//! (CanvasRenderingContext2d::fill_style_str) which isn't stable in 0.3.91.

use wasm_bindgen::JsCast;
use web_sys::CanvasRenderingContext2d;
use crate::state;

/// (topic_key, canvas_id, bar_color, bg_color)
const CANVAS_CONFIGS: &[(&str, &str, &str, &str)] = &[
    ("liquid",    "canvas-liquid",   "#60b0e8", "#0a1a28"),
    ("solid",     "canvas-solid",    "#d8a040", "#1a1000"),
    ("gas",       "canvas-gas",      "#80d050", "#0a1400"),
    ("critical",  "canvas-critical", "#e05050", "#280000"),
    ("__total__", "canvas-total",    "#c97a1a", "#1a0d00"),
];

fn get_ctx(canvas_id: &str) -> Option<CanvasRenderingContext2d> {
    let doc = web_sys::window()?.document()?;
    let canvas = doc.get_element_by_id(canvas_id)?
        .dyn_into::<web_sys::HtmlCanvasElement>().ok()?;
    canvas.get_context("2d").ok()??.dyn_into::<CanvasRenderingContext2d>().ok()
}

pub fn redraw_all() {
    for &(topic, canvas_id, bar_color, bg_color) in CANVAS_CONFIGS {
        draw_sparkline(topic, canvas_id, bar_color, bg_color);
    }
}

#[allow(deprecated)]   // set_fill_style / set_stroke_style: stable in web-sys 0.3.91
fn draw_sparkline(topic: &str, canvas_id: &str, bar_color: &str, bg_color: &str) {
    let Some(ctx) = get_ctx(canvas_id) else { return };

    let canvas = ctx.canvas().unwrap()
        .dyn_into::<web_sys::HtmlCanvasElement>().unwrap();
    let w = canvas.width() as f64;
    let h = canvas.height() as f64;

    let (buckets, max_val) = state::get(|s| {
        if let Some(t) = s.rates.get(topic) {
            (t.buckets.iter().copied().collect::<Vec<_>>(), t.max_in_window() as f64)
        } else {
            (vec![], 1.0)
        }
    });

    // ── Background ────────────────────────────────────────────────────────────
    ctx.set_fill_style(&bg_color.into());
    ctx.fill_rect(0.0, 0.0, w, h);

    if buckets.is_empty() { return; }

    let bar_w   = (w / state::RATE_BUCKETS as f64).max(1.0);
    let gap     = 1.0_f64;
    let max_val = max_val.max(1.0);

    // ── Bars ──────────────────────────────────────────────────────────────────
    ctx.set_fill_style(&bar_color.into());
    for (i, &count) in buckets.iter().enumerate() {
        if count == 0 { continue; }
        let ratio = count as f64 / max_val;
        let bar_h = (ratio * (h - 4.0)).max(1.0);
        let x     = i as f64 / state::RATE_BUCKETS as f64 * w;
        ctx.fill_rect(x + gap * 0.5, h - bar_h, bar_w - gap, bar_h);
    }

    // ── Mid-point grid line ────────────────────────────────────────────────────
    ctx.set_stroke_style(&format!("{}40", bar_color).into());
    ctx.set_line_width(0.5);
    ctx.begin_path();
    ctx.move_to(0.0, h * 0.5);
    ctx.line_to(w, h * 0.5);
    ctx.stroke();

    // ── Sparkline overlay (connect bar tops) ────────────────────────────────
    if buckets.len() > 1 {
        ctx.set_stroke_style(&bar_color.into());
        ctx.set_line_width(1.0);
        ctx.begin_path();
        let mut first = true;
        for (i, &count) in buckets.iter().enumerate() {
            let ratio = count as f64 / max_val;
            let x     = i as f64 / state::RATE_BUCKETS as f64 * w + bar_w * 0.5;
            let y     = h - ratio * (h - 4.0);
            if first { ctx.move_to(x, y); first = false; } else { ctx.line_to(x, y); }
        }
        ctx.stroke();
    }

    // ── Rate label ──────────────────────────────────────────────────────────
    let rate = state::get(|s| {
        s.rates.get(topic).map(|r| r.rate_per_sec()).unwrap_or(0.0)
    });
    if rate > 0.01 {
        ctx.set_fill_style(&bar_color.into());
        ctx.set_font("10px monospace");
        let _ = ctx.fill_text(&format!("{rate:.1}/s"), 3.0, 12.0);
    }
}
