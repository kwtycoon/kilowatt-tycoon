//! Global game resources

pub mod achievements;
pub mod asset_handles;
pub mod build_state;
pub mod charger_queue;
pub mod demand;
pub mod fleet;
pub mod game_clock;
pub mod game_data;
pub mod game_state;
pub mod leaderboard;
pub mod ledger;
pub mod lot_templates;
pub mod multi_site;
pub mod northstar_grid;
pub mod player_profile;
pub mod site_config;
pub mod site_energy;
pub mod site_grid;
pub mod site_upgrades;
pub mod sprite_metadata;
pub mod strategy;
pub mod technician;
pub mod tiled_bridge;
pub mod tutorial;
pub mod unit_system;

use bevy::prelude::*;

pub use achievements::*;
pub use asset_handles::*;
pub use build_state::*;
pub use charger_queue::*;
pub use demand::*;
pub use fleet::*;
pub use game_clock::*;
pub use game_data::*;
pub use game_state::*;
pub use leaderboard::*;
pub use ledger::*;
pub use lot_templates::*;
pub use multi_site::*;
pub use northstar_grid::*;
pub use player_profile::*;
pub use site_config::*;
pub use site_energy::*;
pub use site_grid::*;
pub use site_upgrades::*;
pub use sprite_metadata::*;
pub use strategy::*;
pub use technician::*;
pub use tiled_bridge::*;
pub use tutorial::*;
pub use unit_system::*;

/// Plugin that registers all resources
pub struct ResourcesPlugin;

impl Plugin for ResourcesPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<GameClock>()
            .init_resource::<GameState>()
            .init_resource::<MultiSiteManager>()
            .init_resource::<SiteConfig>()
            .init_resource::<DriverSchedule>()
            .init_resource::<SelectedChargerEntity>()
            .init_resource::<TicketCounter>()
            .init_resource::<BuildState>()
            // SiteGrid is now per-site in SiteState, no global resource
            .init_resource::<crate::systems::AmbientTrafficTimer>()
            // Environment (global - affects all sites)
            .init_resource::<EnvironmentState>()
            // Technician (global - travels between sites)
            .init_resource::<TechnicianState>()
            // Note: SiteUpgrades is per-site, stored in SiteState.site_upgrades
            // Note: ServiceStrategy is per-site, stored in SiteState.service_strategy
            // Carbon credit market (global - fluctuates daily)
            .init_resource::<CarbonCreditMarket>()
            // Game data assets (handles to loaded JSON)
            .init_resource::<GameDataAssets>()
            // Site template cache (parsed template data)
            .init_resource::<SiteTemplateCache>()
            // Tiled map registry
            .init_resource::<TiledMapRegistry>()
            // Tutorial state
            .init_resource::<TutorialState>()
            // Achievement tracking
            .init_resource::<AchievementState>()
            // Player profile (character selection, name, and Supabase ID)
            .insert_resource(PlayerProfile::new())
            // Leaderboard data
            .init_resource::<LeaderboardData>()
            .init_resource::<UnitSystem>()
            .init_resource::<FleetContractManager>()
            .init_resource::<crate::resources::fleet::FleetDebugMode>()
            .init_resource::<crate::systems::robber::DailyRobberyTracker>()
            .init_resource::<crate::systems::hacker::DailyHackerTracker>()
            .add_systems(Startup, (load_image_assets, load_audio_assets));
    }
}
