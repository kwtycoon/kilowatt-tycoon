//! Site visibility system - show/hide entities based on active site

use bevy::prelude::*;

use crate::components::SiteRoot;
use crate::resources::MultiSiteManager;

/// Update visibility of site root entities based on which site is active
///
/// By setting visibility on the root entity, all children automatically inherit
/// the visibility state. This is much more efficient than updating every entity.
pub fn update_site_entity_visibility(
    multi_site: Res<MultiSiteManager>,
    mut site_roots: Query<(&SiteRoot, &mut Visibility)>,
) {
    // Only update when viewed site changes
    if !multi_site.is_changed() {
        return;
    }

    let viewed_site_id = multi_site.viewed_site_id;

    for (site_root, mut visibility) in &mut site_roots {
        *visibility = if Some(site_root.site_id) == viewed_site_id {
            Visibility::Visible
        } else {
            Visibility::Hidden
        };
    }
}
