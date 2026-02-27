//! Data loading functionality

pub mod json_assets;
pub mod loader;
pub mod tiled_loader;

use bevy::prelude::*;

pub use json_assets::*;
pub use loader::*;
pub use tiled_loader::*;

/// Plugin for data loading
pub struct DataPlugin;

impl Plugin for DataPlugin {
    fn build(&self, app: &mut App) {
        // Register JSON asset loaders
        app.add_plugins(JsonAssetPlugin);
        // Note: load_scenario_data is removed - now handled via AssetServer in loading state
    }
}
