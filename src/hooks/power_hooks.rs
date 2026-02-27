//! Power system component lifecycle hooks.
//!
//! These hooks ensure power infrastructure is properly tracked.

use bevy::ecs::lifecycle::HookContext;
use bevy::ecs::world::DeferredWorld;
use bevy::prelude::*;

use crate::components::power::Transformer;

/// Hook called when a Transformer component is added.
///
/// Logs the addition and can be used to validate transformer configuration.
pub fn on_transformer_added(world: DeferredWorld, HookContext { entity, .. }: HookContext) {
    if let Some(transformer) = world.get::<Transformer>(entity) {
        info!(
            "Transformer added: {:?} - Rating: {}kVA, Thermal limit: {}\u{00BA}C",
            entity, transformer.rating_kva, transformer.thermal_limit_c
        );

        // Validate transformer configuration
        if transformer.rating_kva <= 0.0 {
            warn!(
                "Transformer {:?} has invalid rating: {}kVA",
                entity, transformer.rating_kva
            );
        }

        if transformer.thermal_limit_c <= transformer.ambient_temp_c {
            warn!(
                "Transformer {:?} thermal limit ({}) <= ambient temp ({})",
                entity, transformer.thermal_limit_c, transformer.ambient_temp_c
            );
        }
    }
}
