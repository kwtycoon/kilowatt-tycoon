//! OCPI live feed bridge to JavaScript.
//!
//! Streams new OCPI log entries to `window.__kwtycoon_ocpi_feed`
//! so the HTML overlay can consume them alongside the OCPP and OpenADR feeds.
//!
//! Uses the same `js_sys::Reflect::set()` pattern as the other feed bridges.
//!
//! Properties written:
//! - `window.__kwtycoon_ocpi_feed` -- array of new [`OcpiLogEntry`] objects (incremental)

use bevy::prelude::*;
use wasm_bindgen::JsValue;

use crate::ocpi::queue::OcpiMessageQueue;

/// Tracks what we've already pushed to JS so we only send incremental updates.
#[derive(Resource, Default)]
pub struct OcpiFeedState {
    last_pushed_index: usize,
}

fn push_to_window(key: &str, json: &str) {
    let Some(window) = web_sys::window() else {
        return;
    };
    let Ok(parsed) = js_sys::JSON::parse(json) else {
        return;
    };
    let _ = js_sys::Reflect::set(&window, &JsValue::from_str(key), &parsed);
}

/// Bevy system that pushes new OCPI log entries to JS.
///
/// Runs each frame on WASM during the `Playing` state.
pub fn ocpi_feed_system(queue: Res<OcpiMessageQueue>, mut state: ResMut<OcpiFeedState>) {
    if !queue.event_log_enabled {
        return;
    }

    let absolute_len = queue.event_log.len() + queue.total_drained;
    if absolute_len > state.last_pushed_index {
        let start = state.last_pushed_index.saturating_sub(queue.total_drained);
        let new_entries = &queue.event_log[start..];
        if let Ok(json) = serde_json::to_string(new_entries) {
            push_to_window("__kwtycoon_ocpi_feed", &json);
        }
        state.last_pushed_index = absolute_len;
    }
}
