//! Game data resources for loaded JSON assets
//!
//! These resources hold handles to loaded assets and provide
//! synchronous access to parsed data after loading completes.

use bevy::prelude::*;
use bevy_ecs_tiled::prelude::TiledMapAsset;
use std::collections::HashMap;

use crate::data::json_assets::{ChargersAsset, DriverScheduleAsset};
use crate::data::loader::SiteTemplateData;
use crate::resources::SiteArchetype;

/// Resource holding handles to all game data assets
#[derive(Resource, Default)]
pub struct GameDataAssets {
    /// Handle to the chargers configuration
    pub chargers: Handle<ChargersAsset>,
    /// Handle to the driver schedule
    pub driver_schedule: Handle<DriverScheduleAsset>,
    /// Handles to Tiled maps (TMX), keyed by archetype
    pub tiled_maps: HashMap<SiteArchetype, Handle<TiledMapAsset>>,
}

/// Cache of parsed site template data for synchronous access
///
/// This is populated after assets finish loading and provides
/// immediate access to template data without async lookups.
#[derive(Resource, Default)]
pub struct SiteTemplateCache {
    pub templates: HashMap<SiteArchetype, SiteTemplateData>,
    /// Flag indicating all templates have been loaded
    pub loaded: bool,
}

impl SiteTemplateCache {
    /// Get a template by archetype
    pub fn get(&self, archetype: SiteArchetype) -> Option<&SiteTemplateData> {
        self.templates.get(&archetype)
    }

    /// Check if a template exists
    pub fn contains(&self, archetype: SiteArchetype) -> bool {
        self.templates.contains_key(&archetype)
    }

    /// Get all available archetypes
    pub fn archetypes(&self) -> impl Iterator<Item = &SiteArchetype> {
        self.templates.keys()
    }
}
