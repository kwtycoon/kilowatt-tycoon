//! OCPI 2.3.0 roaming message feed module.
//!
//! Generates OCPI 2.3.0 messages from the game's charger/driver simulation
//! (Locations, Sessions, CDRs, EVSE status) and streams them to an
//! in-browser overlay via the JS feed bridge.
//!
//! # Architecture
//!
//! ```text
//! Charger + Driver ECS state
//!          │
//!          ▼
//!   message_gen systems  (Bevy Update, gated on Playing state)
//!          │
//!          ▼
//!   OcpiMessageQueue     (Resource: in-memory log)
//!          │
//!          ▼
//!   ocpi_feed_system     (WASM bridge → window.__kwtycoon_ocpi_feed)
//!          │
//!          ▼
//!   protocol_feed.js     (tabbed overlay, OCPI tab)
//! ```

#[cfg(target_arch = "wasm32")]
pub mod feed;
pub mod message_gen;
pub mod queue;
pub mod types;

use bevy::prelude::*;

use crate::states::AppState;
use message_gen::*;
use queue::OcpiMessageQueue;

/// Plugin that wires up OCPI 2.3.0 message generation + JS feed.
pub struct OcpiPlugin;

impl Plugin for OcpiPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<OcpiMessageQueue>();

        app.add_systems(
            Update,
            (
                ocpi_location_system,
                ocpi_status_system,
                ocpi_session_start_system,
                ocpi_session_update_system,
                ocpi_cdr_system,
            )
                .chain()
                .run_if(in_state(AppState::Playing)),
        );

        #[cfg(target_arch = "wasm32")]
        {
            app.init_resource::<feed::OcpiFeedState>();
            app.add_systems(
                PostUpdate,
                feed::ocpi_feed_system.run_if(in_state(AppState::Playing)),
            );
        }
    }
}
