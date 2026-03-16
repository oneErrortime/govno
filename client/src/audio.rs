//! AudioContext-based alerts
//!
//! Created lazily on first use (browser requires prior user interaction).

use std::cell::RefCell;
use web_sys::{AudioContext, OscillatorType};
use crate::state;

thread_local! {
    static AUDIO_CTX: RefCell<Option<AudioContext>> = const { RefCell::new(None) };
}

fn get_ctx() -> Option<AudioContext> {
    AUDIO_CTX.with(|cell| {
        let mut b = cell.borrow_mut();
        if b.is_none() {
            *b = AudioContext::new().ok();
        }
        b.clone()
    })
}

pub fn beep_critical() {
    if !state::get(|s| s.sound_enabled) { return; }
    let Some(ctx) = get_ctx() else { return };
    for &(freq, offset, dur) in &[(880.0f32, 0.0f64, 0.08f64), (660.0, 0.1, 0.08), (880.0, 0.2, 0.15)] {
        if let (Ok(osc), Ok(gain)) = (ctx.create_oscillator(), ctx.create_gain()) {
            let _ = osc.connect_with_audio_node(&gain);
            let _ = gain.connect_with_audio_node(&ctx.destination());
            osc.set_type(OscillatorType::Square);
            osc.frequency().set_value(freq);
            gain.gain().set_value(0.06);
            let t = ctx.current_time() + offset;
            let _ = osc.start_with_when(t);
            let _ = osc.stop_with_when(t + dur);
        }
    }
}

pub fn beep_soft() {
    if !state::get(|s| s.sound_enabled) { return; }
    let Some(ctx) = get_ctx() else { return };
    if let (Ok(osc), Ok(gain)) = (ctx.create_oscillator(), ctx.create_gain()) {
        let _ = osc.connect_with_audio_node(&gain);
        let _ = gain.connect_with_audio_node(&ctx.destination());
        osc.set_type(OscillatorType::Sine);
        osc.frequency().set_value(440.0);
        gain.gain().set_value(0.025);
        let t = ctx.current_time();
        let _ = osc.start_with_when(t);
        let _ = osc.stop_with_when(t + 0.05);
    }
}
