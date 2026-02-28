//! ChargeOps Simulator Library
//!
//! Core game logic, components, systems, and resources.
//!
//! # Architecture
//!
//! The game is organized into several modules:
//! - [`states`]: Game state machine (MainMenu, Loading, Playing, Paused, GameOver)
//! - [`components`]: ECS components for game entities
//! - [`systems`]: Game logic systems
//! - [`resources`]: Shared game resources
//! - [`events`]: Game events and messages
//! - [`errors`]: Error types and result handling utilities
//! - [`observers`]: Entity-targeted observer handlers
//! - [`helpers`]: Reusable utility modules (camera, debug, UI builders)
//! - [`ui`]: User interface systems
//! - [`data`]: Data loading and configuration
//!
//! ## Level Rendering Architecture
//!
//! Levels are designed in Tiled and rendered using a hybrid approach:
//!
//! - **bevy_ecs_tiled** - Renders all base tiles (terrain, walls, parking, decorations) from TMX files
//! - **Entity overlays** - Infrastructure with dynamic state (transformers, solar, batteries, chargers)
//! - **SiteGrid** - Maintains gameplay state (pathfinding, placement validation) initialized from TMX
//!
//! TMX files are the single source of truth containing:
//! - Tile layer for visuals + layout
//! - Object layer for gameplay zones
//! - Map properties for configuration (capacity, popularity, rent cost, etc.)
//!
//! # Error Handling
//!
//! The game uses a consistent error handling pattern:
//! - Fallible systems return `Result<(), bevy::ecs::error::BevyError>`
//! - The global error handler logs warnings instead of panicking
//! - Custom [`ChargeOpsError`](errors::ChargeOpsError) provides domain-specific errors
//! - Extension traits like [`ResultExt`](errors::ResultExt) simplify error logging

// Bevy systems commonly have complex query types and many system parameters.
// These are structural patterns, not code smells in this context.
#![allow(clippy::type_complexity)]
#![allow(clippy::too_many_arguments)]

pub mod api;
pub mod audio;
pub mod components;
pub mod data;
pub mod errors;
pub mod events;
pub mod helpers;
pub mod hooks;
pub mod observers;
#[cfg(feature = "ocpi")]
pub mod ocpi;
#[cfg(feature = "ocpp")]
pub mod ocpp;
#[cfg(feature = "openadr")]
pub mod openadr;
pub mod resources;
pub mod states;
pub mod systems;
pub mod ui;

use bevy::prelude::*;
use bevy_ecs_tiled::prelude::*;
use bevy_northstar::prelude::*;

use api::ApiPlugin;
use audio::AudioPlugin;
use components::ComponentsPlugin;
use data::DataPlugin;
use errors::ErrorsPlugin;
use events::EventsPlugin;
use helpers::HelpersPlugin;
use hooks::HooksPlugin;
use observers::ObserversPlugin;
#[cfg(feature = "ocpi")]
use ocpi::OcpiPlugin;
#[cfg(feature = "ocpp")]
use ocpp::OcppPlugin;
#[cfg(feature = "openadr")]
use openadr::OpenAdrPlugin;
use resources::ResourcesPlugin;
use states::StatesPlugin;
use systems::SystemsPlugin;
use ui::UiPlugin;

/// Main game plugin that bundles all subsystems
pub struct ChargeOpsPlugin;

impl Plugin for ChargeOpsPlugin {
    fn build(&self, app: &mut App) {
        // Split into two add_plugins calls to stay within Bevy's tuple limit (15)
        app.add_plugins((
            // Error handling should be configured first
            ErrorsPlugin,
            StatesPlugin,
            AudioPlugin,
            ResourcesPlugin,
            ComponentsPlugin,
            EventsPlugin,
            ObserversPlugin,
            HooksPlugin,
            HelpersPlugin,
            DataPlugin,
            SystemsPlugin,
            UiPlugin,
        ));
        app.add_plugins((
            ApiPlugin,
            #[cfg(feature = "ocpi")]
            OcpiPlugin,
            #[cfg(feature = "ocpp")]
            OcppPlugin,
            #[cfg(feature = "openadr")]
            OpenAdrPlugin,
            NorthstarPlugin::<CardinalNeighborhood>::default(),
            TiledPlugin::default(),
        ));
    }
}
