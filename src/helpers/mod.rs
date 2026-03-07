//! Reusable helper modules for common patterns.
//!
//! This module contains utility systems and components that can be reused
//! across different parts of the game.
//!
//! # Modules
//!
//! - [`camera_controller`]: Pan and zoom camera controls for top-down view
//! - [`canopy_layout`]: Shared photovoltaic canopy preview/render geometry
//! - [`debug_overlay`]: Toggle-able debug information display
//! - [`pointer`]: Unified mouse/touch pointer abstraction
//! - [`ui_builders`]: Common UI widget construction utilities

pub mod camera_controller;
pub mod canopy_layout;
pub mod debug_overlay;
pub mod pointer;
pub mod ui_builders;

use bevy::prelude::*;
use bevy_northstar::prelude::{CardinalNeighborhood, NorthstarDebugPlugin};

pub use camera_controller::*;
pub use canopy_layout::*;
pub use debug_overlay::*;
pub use pointer::{GamePointer, PointerPlugin};
pub use ui_builders::*;

/// Plugin that adds all helper systems
pub struct HelpersPlugin;

impl Plugin for HelpersPlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins((
            CameraControllerPlugin,
            DebugOverlayPlugin,
            PointerPlugin,
            NorthstarDebugPlugin::<CardinalNeighborhood>::default(),
        ));
    }
}
