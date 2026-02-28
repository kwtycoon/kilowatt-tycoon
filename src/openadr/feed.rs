//! OpenADR live feed bridge to JavaScript.
//!
//! Streams new OpenADR log entries to `window.__kwtycoon_openadr_feed`
//! so the HTML overlay can consume them alongside the OCPP feed.
//!
//! Uses the same `js_sys::Reflect::set()` pattern as the OCPP feed bridge
//! in `crate::ocpp::feed`.
//!
//! Properties written:
//! - `window.__kwtycoon_openadr_feed` -- array of new [`OpenAdrLogEntry`] objects (incremental)

use bevy::prelude::*;
use wasm_bindgen::JsValue;

use crate::openadr::queue::OpenAdrMessageQueue;

/// Tracks what we've already pushed to JS so we only send incremental updates.
#[derive(Resource, Default)]
pub struct OpenAdrFeedState {
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

/// Bevy system that pushes new OpenADR log entries to JS.
///
/// Runs each frame on WASM during the `Playing` state.
pub fn openadr_feed_system(queue: Res<OpenAdrMessageQueue>, mut state: ResMut<OpenAdrFeedState>) {
    if !queue.event_log_enabled {
        return;
    }

    let log_len = queue.event_log.len();
    if log_len > state.last_pushed_index {
        let new_entries = &queue.event_log[state.last_pushed_index..];
        if let Ok(json) = serde_json::to_string(new_entries) {
            push_to_window("__kwtycoon_openadr_feed", &json);
        }
        state.last_pushed_index = log_len;
    }
}
