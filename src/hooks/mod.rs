//! Component lifecycle hooks for maintaining data integrity.
//!
//! Hooks are callbacks that fire when components are added, modified, or removed.
//! They're useful for:
//! - Maintaining indices (e.g., spatial grids, lookups)
//! - Enforcing structural rules
//! - Automatic cleanup
//!
//! # Example
//!
//! ```rust,ignore
//! // Register hooks in your plugin
//! fn build(&self, app: &mut App) {
//!     app.world_mut()
//!         .register_component_hooks::<Charger>()
//!         .on_add(on_charger_added)
//!         .on_remove(on_charger_removed);
//! }
//! ```

pub mod charger_hooks;
pub mod power_hooks;

use bevy::prelude::*;

pub use charger_hooks::*;
pub use power_hooks::*;

/// Plugin that registers all component hooks
pub struct HooksPlugin;

impl Plugin for HooksPlugin {
    fn build(&self, app: &mut App) {
        // Initialize the charger index resource
        app.init_resource::<ChargerIndex>();

        // Register charger hooks
        app.world_mut()
            .register_component_hooks::<crate::components::charger::Charger>()
            .on_add(on_charger_added)
            .on_remove(on_charger_removed);

        // Register power tracking hooks
        app.world_mut()
            .register_component_hooks::<crate::components::power::Transformer>()
            .on_add(on_transformer_added);
    }
}
