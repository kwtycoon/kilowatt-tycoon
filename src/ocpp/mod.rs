//! OCPP 1.6J integration module.
//!
//! Generates real-time OCPP messages from the game simulation and streams
//! them over WebSocket to a Central System (e.g. Relion).
//!
//! # Configuration
//!
//! Set the `OCPP_ENDPOINT` environment variable to enable OCPP streaming:
//!
//! ```bash
//! OCPP_ENDPOINT=ws://relion.example.com/ocpp cargo run
//! ```
//!
//! Each charger will connect to `{OCPP_ENDPOINT}/{charger_id}`.
//!
//! # Architecture
//!
//! ```text
//! Game ECS Events / Charger State
//!          │
//!          ▼
//!   message_gen systems  (Bevy Update, gated on Playing state)
//!          │
//!          ▼
//!   OcppMessageQueue     (Resource: outbound buffer)
//!          │
//!          ▼
//!   ocpp_send_system     (drains queue → WebSocket connections)
//!          │
//!          ▼
//!   OcppConnectionManager (per-charger WS connections)
//!          │
//!          ▼
//!   Relion CSMS          (ws://host/ocpp/{charger_id})
//! ```

pub mod connection;
#[cfg(not(target_arch = "wasm32"))]
pub mod disk_writer;
#[cfg(target_arch = "wasm32")]
pub mod feed;
pub mod message_gen;
pub mod ports_registry;
pub mod queue;
pub mod types;

use bevy::prelude::*;

use crate::states::AppState;
use connection::{OcppConnectionManager, WsStatus, ocpp_send_system};
use message_gen::*;
use queue::OcppMessageQueue;

/// Plugin that wires up OCPP message generation + WebSocket streaming.
pub struct OcppPlugin;

impl Plugin for OcppPlugin {
    fn build(&self, app: &mut App) {
        // Resources
        app.init_resource::<OcppMessageQueue>()
            .init_resource::<OcppConnectionManager>()
            .init_resource::<ports_registry::PortsRegistry>();

        // Native-only: disk writer resource + startup + flush system
        #[cfg(not(target_arch = "wasm32"))]
        {
            app.init_resource::<disk_writer::OcppDiskWriter>();
            app.add_systems(Startup, disk_writer::ocpp_disk_writer_init);
            app.add_systems(Update, disk_writer::ocpp_disk_write_system);
        }

        // Startup: read OCPP_ENDPOINT env var
        app.add_systems(Startup, ocpp_config_from_env);

        // Message generation systems — run during Playing state
        app.add_systems(
            Update,
            (
                ocpp_boot_system,
                ocpp_status_system,
                ocpp_start_transaction_system,
                ocpp_stop_transaction_system,
                ocpp_meter_values_system,
                ocpp_heartbeat_system,
            )
                .run_if(in_state(AppState::Playing)),
        );

        // Connection send system — always runs (flushes queued messages even during pause)
        app.add_systems(Update, ocpp_send_system);

        // WASM-only: live feed bridge to JS overlay
        #[cfg(target_arch = "wasm32")]
        {
            app.init_resource::<feed::OcppFeedState>();
            app.add_systems(
                PostUpdate,
                feed::ocpp_feed_system.run_if(in_state(AppState::Playing)),
            );
        }

        // Status logging — periodic summary
        app.add_systems(
            Update,
            ocpp_status_log_system.run_if(in_state(AppState::Playing)),
        );
    }
}

// ─────────────────────────────────────────────────────
//  Startup: environment variable config
// ─────────────────────────────────────────────────────

/// Read `OCPP_ENDPOINT` from the environment at startup.
/// If set, OCPP streaming is automatically enabled.
fn ocpp_config_from_env(mut queue: ResMut<OcppMessageQueue>) {
    // On WASM, std::env::var won't work — could use URL query params instead
    #[cfg(not(target_arch = "wasm32"))]
    {
        if let Ok(url) = std::env::var("OCPP_ENDPOINT")
            && !url.is_empty()
        {
            info!("OCPP: Streaming enabled — endpoint: {}", url);
            queue.endpoint_url = url;
            queue.enabled = true;
        }

        if !queue.enabled {
            info!("OCPP: Streaming disabled (set OCPP_ENDPOINT env var to enable)");
        }
    }

    #[cfg(target_arch = "wasm32")]
    {
        // On WASM, check URL query parameter: ?ocpp_endpoint=ws://...
        if let Some(window) = web_sys::window() {
            if let Ok(search) = window.location().search() {
                if let Some(url) = search.trim_start_matches('?').split('&').find_map(|pair| {
                    let mut parts = pair.splitn(2, '=');
                    let key = parts.next()?;
                    let val = parts.next()?;
                    if key == "ocpp_endpoint" {
                        Some(val.to_string())
                    } else {
                        None
                    }
                }) {
                    info!("OCPP: Streaming enabled (WASM) — endpoint: {}", url);
                    queue.endpoint_url = url;
                    queue.enabled = true;
                }
            }
        }

        if !queue.enabled {
            info!("OCPP: Streaming disabled (add ?ocpp_endpoint=ws://... to URL to enable)");
        }
    }
}

// ─────────────────────────────────────────────────────
//  Status logging system
// ─────────────────────────────────────────────────────

/// Tracks when we last logged OCPP status.
#[derive(Default)]
struct OcppStatusLogTimer {
    last_log_time: f32,
}

/// Periodically log OCPP connection status (every 30 real seconds).
fn ocpp_status_log_system(
    queue: Res<OcppMessageQueue>,
    conn_mgr: Res<OcppConnectionManager>,
    time: Res<Time>,
    mut timer: Local<OcppStatusLogTimer>,
) {
    if !queue.is_active() {
        return;
    }

    timer.last_log_time += time.delta_secs();
    if timer.last_log_time < 30.0 {
        return;
    }
    timer.last_log_time = 0.0;

    let total = conn_mgr.connections.len();
    let connected = conn_mgr
        .connections
        .values()
        .filter(|c| c.status == WsStatus::Connected)
        .count();
    let pending_msgs = queue.messages.len();

    info!(
        "OCPP status: {}/{} chargers connected, {} messages queued, endpoint: {}",
        connected, total, pending_msgs, queue.endpoint_url
    );
}
