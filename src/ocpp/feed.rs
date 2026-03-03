//! OCPP live feed bridge to JavaScript.
//!
//! Streams new OCPP log entries and port registry data to `window` properties
//! so an HTML overlay (or future DuckDB-WASM integration) can consume them.
//!
//! Uses the same `js_sys::Reflect::set()` pattern as the test bridge
//! in `src/systems/test_bridge.rs`.
//!
//! Properties written:
//! - `window.__kwtycoon_ocpp_feed` — array of new [`OcppLogEntry`] objects (incremental)
//! - `window.__kwtycoon_ocpp_ports` — full array of [`PortEntry`] objects (replaced when changed)

use bevy::prelude::*;
use wasm_bindgen::JsValue;

use crate::ocpp::ports_registry::PortsRegistry;
use crate::ocpp::queue::OcppMessageQueue;

/// Tracks what we've already pushed to JS so we only send incremental updates.
#[derive(Resource, Default)]
pub struct OcppFeedState {
    last_pushed_index: usize,
    last_ports_count: usize,
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

/// Bevy system that pushes new OCPP log entries and port registry changes to JS.
///
/// Runs each frame on WASM when the `ocpp` feature is enabled.
pub fn ocpp_feed_system(
    queue: Res<OcppMessageQueue>,
    ports: Res<PortsRegistry>,
    mut state: ResMut<OcppFeedState>,
) {
    if !queue.event_log_enabled {
        return;
    }

    let absolute_len = queue.event_log.len() + queue.total_drained;
    if absolute_len > state.last_pushed_index {
        let start = state.last_pushed_index.saturating_sub(queue.total_drained);
        let new_entries = &queue.event_log[start..];
        if let Ok(json) = serde_json::to_string(new_entries) {
            push_to_window("__kwtycoon_ocpp_feed", &json);
        }
        state.last_pushed_index = absolute_len;
    }

    let ports_len = ports.entries.len();
    if ports_len != state.last_ports_count {
        if let Ok(json) = serde_json::to_string(&ports.entries) {
            push_to_window("__kwtycoon_ocpp_ports", &json);
        }
        state.last_ports_count = ports_len;
    }
}
