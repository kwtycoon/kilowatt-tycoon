//! OpenADR 3.0 DER message feed module.
//!
//! Generates real-time OpenADR 3.0 messages from the game's DER simulation
//! (solar arrays, battery storage, grid import) and streams them to an
//! in-browser overlay via the JS feed bridge.
//!
//! # Architecture
//!
//! ```text
//! SiteState (solar, BESS, grid)
//!          │
//!          ▼
//!   message_gen systems  (Bevy Update, gated on Playing state)
//!          │
//!          ▼
//!   OpenAdrMessageQueue  (Resource: in-memory log)
//!          │
//!          ▼
//!   openadr_feed_system  (WASM bridge → window.__kwtycoon_openadr_feed)
//!          │
//!          ▼
//!   protocol_feed.js     (tabbed overlay, OpenADR tab)
//! ```

#[cfg(target_arch = "wasm32")]
pub mod feed;
pub mod message_gen;
pub mod queue;
pub mod types;

use bevy::prelude::*;

use crate::states::AppState;
use message_gen::*;
use queue::OpenAdrMessageQueue;

/// Plugin that wires up OpenADR 3.0 message generation + JS feed.
pub struct OpenAdrPlugin;

impl Plugin for OpenAdrPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<OpenAdrMessageQueue>();

        // Message generation systems -- run during Playing state.
        // Program registration runs first, then VEN/resource registration,
        // then telemetry (grid last to update shared timestamps), then events.
        app.add_systems(
            Update,
            (
                openadr_program_system,
                openadr_register_system,
                openadr_solar_telemetry_system,
                openadr_bess_telemetry_system,
                openadr_grid_telemetry_system,
                openadr_event_system,
                openadr_event_response_system,
                openadr_export_event_system,
                openadr_customer_price_system,
                openadr_ghg_signal_system,
                openadr_grid_alert_system,
            )
                .chain()
                .run_if(in_state(AppState::Playing)),
        );

        // WASM-only: live feed bridge to JS overlay
        #[cfg(target_arch = "wasm32")]
        {
            app.init_resource::<feed::OpenAdrFeedState>();
            app.add_systems(
                PostUpdate,
                feed::openadr_feed_system.run_if(in_state(AppState::Playing)),
            );
        }
    }
}
