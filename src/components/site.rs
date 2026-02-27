//! Site-related components

use crate::resources::SiteId;
use bevy::prelude::*;

/// Component marking an entity as a site root
///
/// The site root entity acts as a parent for all entities belonging to that site.
/// Its Transform is positioned at the site's world_offset, and all child entities
/// use coordinates relative to the site root (eliminating the need for manual
/// world_offset calculations throughout the codebase).
#[derive(Component, Debug, Clone, Copy, PartialEq, Eq)]
pub struct SiteRoot {
    pub site_id: SiteId,
}

impl SiteRoot {
    /// Create a new site root marker
    pub fn new(site_id: SiteId) -> Self {
        Self { site_id }
    }
}

/// Component that tags an entity as belonging to a specific site
///
/// This enables concurrent operation of multiple sites - all entities exist
/// in the ECS simultaneously, and systems filter by site ID to process each
/// site independently.
///
/// Note: Entities are also children of their site's SiteRoot entity for
/// automatic transform propagation.
#[derive(Component, Debug, Clone, Copy, PartialEq, Eq)]
pub struct BelongsToSite {
    pub site_id: SiteId,
}

impl BelongsToSite {
    /// Create a new site tag
    pub fn new(site_id: SiteId) -> Self {
        Self { site_id }
    }
}
