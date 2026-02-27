//! API integration module for external services

pub mod leaderboard;
pub mod supabase;

use bevy::prelude::*;

pub use leaderboard::*;
pub use supabase::*;

/// Plugin that sets up API integrations
pub struct ApiPlugin;

impl Plugin for ApiPlugin {
    fn build(&self, app: &mut App) {
        if let Some(config) = SupabaseConfig::from_env() {
            info!("Supabase configured: {}", config.url);
            app.insert_resource(config);
        } else {
            info!("Supabase not configured -- leaderboard disabled");
        }
    }
}
