//! Site auto-initialization - rents the free starter site on Day 1
//!
//! This initializes the first site when the game starts (Day 1 only).

use crate::resources::{BuildState, GameState, LotTemplate, MultiSiteManager, SiteTemplateCache};
use bevy::prelude::*;

/// Auto-rent the free starter site on Day 1 when entering Playing state.
/// On Day 2+, the site is already rented so this is a no-op.
pub fn initialize_game_on_first_play(
    mut multi_site: ResMut<MultiSiteManager>,
    mut game_state: ResMut<GameState>,
    mut build_state: ResMut<BuildState>,
    game_clock: Res<crate::resources::GameClock>,
    template_cache: Res<SiteTemplateCache>,
    tiled_assets: Res<bevy::asset::Assets<bevy_ecs_tiled::prelude::TiledMapAsset>>,
    game_data: Res<crate::resources::GameDataAssets>,
) {
    // Only initialize on Day 1
    if game_clock.day > 1 {
        return;
    }

    // Check if we already have a site (shouldn't happen, but defensive)
    if !multi_site.owned_sites.is_empty() {
        return;
    }

    // Rent the free starter site (index 0 = "First Street Station")
    let available_sites = multi_site.available_sites_list();
    if available_sites.is_empty() {
        error!("No sites available to rent!");
        return;
    }

    let starter_listing = available_sites[0].clone();

    match multi_site.rent_site(&starter_listing, &template_cache, &tiled_assets, &game_data) {
        Ok(site_id) => {
            info!(
                "Rented starter site: {} ({:?})",
                starter_listing.name, site_id
            );

            // Template is now automatically applied by rent_site()
            // Set starting budget based on traditional Large template
            game_state.reset_with_cash(LotTemplate::Large.starting_budget() as f32);

            // Initialize current day tracker for Day 1
            game_state.daily_history.current_day =
                crate::resources::game_state::CurrentDayTracker {
                    site_id: Some(site_id),
                    starting_reputation: game_state.reputation,
                    ..Default::default()
                };

            // Set build mode to charger L2
            build_state.selected_tool = crate::resources::BuildTool::ChargerL2;
        }
        Err(e) => {
            error!("Failed to rent starter site: {}", e);
        }
    }
}
