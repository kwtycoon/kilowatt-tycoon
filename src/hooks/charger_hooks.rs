//! Charger component lifecycle hooks.
//!
//! These hooks maintain a charger index for quick lookups by ID.

use bevy::ecs::lifecycle::HookContext;
use bevy::ecs::world::DeferredWorld;
use bevy::prelude::*;
use std::collections::HashMap;

use crate::components::charger::Charger;

/// Resource that maintains an index of charger IDs to entities.
///
/// This index is automatically updated when chargers are added or removed.
#[derive(Resource, Default, Debug)]
pub struct ChargerIndex {
    /// Map from charger ID to entity
    pub by_id: HashMap<String, Entity>,
    /// Map from grid position to entity (if set)
    pub by_position: HashMap<(i32, i32), Entity>,
}

impl ChargerIndex {
    /// Get a charger entity by its ID.
    pub fn get_by_id(&self, id: &str) -> Option<Entity> {
        self.by_id.get(id).copied()
    }

    /// Get a charger entity by its grid position.
    pub fn get_by_position(&self, x: i32, y: i32) -> Option<Entity> {
        self.by_position.get(&(x, y)).copied()
    }

    /// Get all charger entities.
    pub fn all_entities(&self) -> impl Iterator<Item = Entity> + '_ {
        self.by_id.values().copied()
    }

    /// Get the number of registered chargers.
    pub fn count(&self) -> usize {
        self.by_id.len()
    }

    /// Check if a charger ID exists.
    pub fn contains_id(&self, id: &str) -> bool {
        self.by_id.contains_key(id)
    }

    /// Check if a grid position has a charger.
    pub fn contains_position(&self, x: i32, y: i32) -> bool {
        self.by_position.contains_key(&(x, y))
    }
}

/// Hook called when a Charger component is added to an entity.
pub fn on_charger_added(mut world: DeferredWorld, HookContext { entity, .. }: HookContext) {
    // Get the charger data
    let charger = world.get::<Charger>(entity);
    let charger_id = charger.map(|c| c.id.clone());
    let grid_pos = charger.and_then(|c| c.grid_position);

    // Update the index
    if let Some(id) = charger_id
        && let Some(mut index) = world.get_resource_mut::<ChargerIndex>()
    {
        if !id.is_empty() {
            index.by_id.insert(id.clone(), entity);
            debug!("Charger index: added {} -> {:?}", id, entity);
        }

        if let Some(pos) = grid_pos {
            index.by_position.insert(pos, entity);
            debug!("Charger index: position {:?} -> {:?}", pos, entity);
        }
    }
}

/// Hook called when a Charger component is removed from an entity.
pub fn on_charger_removed(mut world: DeferredWorld, HookContext { entity, .. }: HookContext) {
    // Get the charger data before it's removed
    let charger = world.get::<Charger>(entity);
    let charger_id = charger.map(|c| c.id.clone());
    let grid_pos = charger.and_then(|c| c.grid_position);

    // Update the index
    if let Some(id) = charger_id
        && let Some(mut index) = world.get_resource_mut::<ChargerIndex>()
    {
        if !id.is_empty() {
            index.by_id.remove(&id);
            debug!("Charger index: removed {}", id);
        }

        if let Some(pos) = grid_pos {
            index.by_position.remove(&pos);
            debug!("Charger index: removed position {:?}", pos);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_charger_index_operations() {
        let mut index = ChargerIndex::default();

        // Add entries - use from_bits to create a test entity
        let entity = Entity::from_bits(1);
        index.by_id.insert("CHG-001".to_string(), entity);
        index.by_position.insert((5, 10), entity);

        // Test lookups
        assert_eq!(index.get_by_id("CHG-001"), Some(entity));
        assert_eq!(index.get_by_position(5, 10), Some(entity));
        assert_eq!(index.get_by_id("CHG-999"), None);

        // Test contains
        assert!(index.contains_id("CHG-001"));
        assert!(!index.contains_id("CHG-999"));
        assert!(index.contains_position(5, 10));
        assert!(!index.contains_position(0, 0));

        // Test count
        assert_eq!(index.count(), 1);
    }
}
