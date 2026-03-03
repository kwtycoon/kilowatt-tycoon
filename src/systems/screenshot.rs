//! Screenshot automation system for capturing level images.
//!
//! This module provides a `--screenshot` mode that:
//! 1. Bypasses the main menu
//! 2. Loads each site archetype in sequence
//! 3. Takes a screenshot of each
//! 4. Saves to `spec/levels/` with descriptive names
//! 5. Exits when complete

use bevy::app::AppExit;
use bevy::ecs::message::MessageWriter;
use bevy::prelude::*;
use bevy::render::view::screenshot::{Screenshot, save_to_disk};

use crate::resources::{
    CharacterKind, MultiSiteManager, PlayerProfile, SiteArchetype, SiteTemplateCache,
};
use crate::states::AppState;

/// Resource that controls screenshot automation mode.
#[derive(Resource)]
pub struct ScreenshotMode {
    /// Whether screenshot mode is enabled
    pub enabled: bool,
    /// Current archetype index being processed
    pub current_index: usize,
    /// Current state in the screenshot process
    pub state: ScreenshotState,
    /// Frame counter for waiting
    pub wait_frames: u32,
    /// Whether we've initialized (rented the first site)
    pub initialized: bool,
}

impl Default for ScreenshotMode {
    fn default() -> Self {
        Self {
            enabled: false,
            current_index: 0,
            state: ScreenshotState::Idle,
            wait_frames: 0,
            initialized: false,
        }
    }
}

/// State machine for screenshot capture process
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ScreenshotState {
    /// Waiting for setup
    #[default]
    Idle,
    /// Waiting for scene to render
    WaitingForRender,
    /// Taking the screenshot
    TakingScreenshot,
    /// Screenshot saved, advancing to next
    Advancing,
    /// All screenshots done
    Done,
}

/// Get the filename for a given archetype index
fn get_screenshot_filename(index: usize, archetype: SiteArchetype) -> String {
    let level_num = index + 1;
    let name = match archetype {
        SiteArchetype::ParkingLot => "first_street",
        SiteArchetype::GasStation => "quick_charge_express",
        SiteArchetype::FleetDepot => "central_fleet_plaza",
        SiteArchetype::ScooterHub => "scooter_alley",
    };
    format!("spec/levels/level_{:02}_{}.png", level_num, name)
}

/// Run condition: screenshot mode is enabled
pub fn screenshot_mode_enabled(mode: Res<ScreenshotMode>) -> bool {
    mode.enabled
}

/// System that initializes screenshot mode on entering Playing state
pub fn screenshot_init_system(
    mut mode: ResMut<ScreenshotMode>,
    mut multi_site: ResMut<MultiSiteManager>,
    template_cache: Res<SiteTemplateCache>,
    mut build_state: ResMut<crate::resources::BuildState>,
    tiled_assets: Res<bevy::asset::Assets<bevy_ecs_tiled::prelude::TiledMapAsset>>,
    game_data: Res<crate::resources::GameDataAssets>,
) {
    if !mode.enabled || mode.initialized {
        return;
    }

    info!("Screenshot mode: Initializing...");

    // Get the first archetype
    let archetypes = SiteArchetype::all_variants();
    if archetypes.is_empty() {
        error!("No archetypes available!");
        mode.state = ScreenshotState::Done;
        return;
    }

    let first_archetype = archetypes[0];

    // Find the listing for this archetype
    let listing = multi_site
        .available_sites_list()
        .iter()
        .find(|l| l.archetype == first_archetype)
        .cloned();

    if let Some(listing) = listing {
        match multi_site.rent_site(&listing, &template_cache, &tiled_assets, &game_data) {
            Ok(site_id) => {
                info!(
                    "Screenshot mode: Rented {} (site {})",
                    listing.name, site_id.0
                );
                mode.initialized = true;
                mode.current_index = 0;
                mode.state = ScreenshotState::WaitingForRender;
                mode.wait_frames = 60; // Wait 60 frames for scene to fully render

                // Open the station so the scene renders properly
                build_state.is_open = true;
            }
            Err(e) => {
                error!("Screenshot mode: Failed to rent site: {}", e);
                mode.state = ScreenshotState::Done;
            }
        }
    } else {
        error!(
            "Screenshot mode: No listing found for {:?}",
            first_archetype
        );
        mode.state = ScreenshotState::Done;
    }
}

/// Main screenshot automation system
pub fn screenshot_automation_system(
    mut commands: Commands,
    mut mode: ResMut<ScreenshotMode>,
    mut multi_site: ResMut<MultiSiteManager>,
    template_cache: Res<SiteTemplateCache>,
    mut app_exit: MessageWriter<AppExit>,
    tiled_assets: Res<bevy::asset::Assets<bevy_ecs_tiled::prelude::TiledMapAsset>>,
    game_data: Res<crate::resources::GameDataAssets>,
) {
    if !mode.enabled || !mode.initialized {
        return;
    }

    let archetypes = SiteArchetype::all_variants();

    match mode.state {
        ScreenshotState::Idle => {
            // Should not happen after initialization
        }
        ScreenshotState::WaitingForRender => {
            // Count down frames
            if mode.wait_frames > 0 {
                mode.wait_frames -= 1;
            } else {
                // Ready to take screenshot
                mode.state = ScreenshotState::TakingScreenshot;
            }
        }
        ScreenshotState::TakingScreenshot => {
            let current_archetype = archetypes[mode.current_index];
            let filename = get_screenshot_filename(mode.current_index, current_archetype);

            info!(
                "Screenshot mode: Capturing {} -> {}",
                current_archetype.display_name(),
                filename
            );

            // Spawn screenshot capture
            commands
                .spawn(Screenshot::primary_window())
                .observe(save_to_disk(filename));

            mode.state = ScreenshotState::Advancing;
            mode.wait_frames = 10; // Wait a bit for screenshot to complete
        }
        ScreenshotState::Advancing => {
            // Wait for screenshot to be saved
            if mode.wait_frames > 0 {
                mode.wait_frames -= 1;
                return;
            }

            // Move to next archetype
            mode.current_index += 1;

            if mode.current_index >= archetypes.len() {
                info!(
                    "Screenshot mode: All {} screenshots captured!",
                    archetypes.len()
                );
                mode.state = ScreenshotState::Done;
                return;
            }

            let next_archetype = archetypes[mode.current_index];

            // Sell current site and rent next one
            // First, find the current site ID
            let current_site_id = multi_site.viewed_site_id;

            // Rent the new site
            let listing = multi_site
                .available_sites_list()
                .iter()
                .find(|l| l.archetype == next_archetype)
                .cloned();

            if let Some(listing) = listing {
                match multi_site.rent_site(&listing, &template_cache, &tiled_assets, &game_data) {
                    Ok(new_site_id) => {
                        info!(
                            "Screenshot mode: Rented {} (site {})",
                            listing.name, new_site_id.0
                        );

                        // Switch to new site
                        let _ = multi_site.switch_to_site(new_site_id);

                        // Sell the old site to clean up
                        if let Some(old_id) = current_site_id {
                            let _ = multi_site.sell_site(old_id);
                        }

                        mode.state = ScreenshotState::WaitingForRender;
                        mode.wait_frames = 60; // Wait for scene to render
                    }
                    Err(e) => {
                        error!("Screenshot mode: Failed to rent site: {}", e);
                        mode.state = ScreenshotState::Done;
                    }
                }
            } else {
                error!("Screenshot mode: No listing found for {:?}", next_archetype);
                mode.state = ScreenshotState::Done;
            }
        }
        ScreenshotState::Done => {
            // Exit the application
            info!("Screenshot mode: Exiting...");
            app_exit.write(AppExit::Success);
        }
    }
}

/// System to skip main menu in screenshot mode
pub fn screenshot_skip_menu_system(
    mode: Res<ScreenshotMode>,
    current_state: Res<State<AppState>>,
    mut next_state: ResMut<NextState<AppState>>,
) {
    if mode.enabled && *current_state.get() == AppState::MainMenu {
        info!("Screenshot mode: Skipping main menu, going to Loading...");
        next_state.set(AppState::Loading);
    }
}

/// Pre-populate the player profile during Loading so the character selection
/// overlay never spawns when we enter Playing in screenshot mode.
pub fn screenshot_skip_character_setup(
    mode: Res<ScreenshotMode>,
    mut profile: ResMut<PlayerProfile>,
) {
    if mode.enabled && profile.character.is_none() {
        profile.character = Some(CharacterKind::Raccoon);
        profile.name = "Screenshot".to_string();
        info!("Screenshot mode: Skipping character selection");
    }
}
