//! JSON asset types and loaders for Bevy's AssetServer
//!
//! This module provides custom asset types for loading game data JSON files
//! through Bevy's asset system, enabling WASM compatibility and hot-reloading.

use bevy::asset::io::Reader;
use bevy::asset::{Asset, AssetLoader, LoadContext};
use bevy::prelude::*;
use thiserror::Error;

use crate::resources::{ChargerData, DriverSchedule};

// ============================================================================
// Error Types
// ============================================================================

/// Errors that can occur when loading JSON assets
#[derive(Debug, Error)]
pub enum JsonLoaderError {
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
    #[error("JSON parse error: {0}")]
    Json(#[from] serde_json::Error),
}

// ============================================================================
// Asset Types
// ============================================================================

/// Chargers configuration asset (list of charger definitions)
#[derive(Asset, TypePath, Debug, Clone)]
pub struct ChargersAsset(pub Vec<ChargerData>);

/// Driver schedule asset
#[derive(Asset, TypePath, Debug, Clone)]
pub struct DriverScheduleAsset(pub DriverSchedule);

// ============================================================================
// Asset Loaders
// ============================================================================

/// Loader for chargers JSON files (.chargers.json)
#[derive(Default)]
pub struct ChargersLoader;

impl AssetLoader for ChargersLoader {
    type Asset = ChargersAsset;
    type Settings = ();
    type Error = JsonLoaderError;

    async fn load(
        &self,
        reader: &mut dyn Reader,
        _settings: &Self::Settings,
        _load_context: &mut LoadContext<'_>,
    ) -> Result<Self::Asset, Self::Error> {
        let mut bytes = Vec::new();
        reader.read_to_end(&mut bytes).await?;
        let data: Vec<ChargerData> = serde_json::from_slice(&bytes)?;
        Ok(ChargersAsset(data))
    }

    fn extensions(&self) -> &[&str] {
        &["chargers.json"]
    }
}

/// Loader for driver schedule JSON files (.scenario.json)
#[derive(Default)]
pub struct DriverScheduleLoader;

impl AssetLoader for DriverScheduleLoader {
    type Asset = DriverScheduleAsset;
    type Settings = ();
    type Error = JsonLoaderError;

    async fn load(
        &self,
        reader: &mut dyn Reader,
        _settings: &Self::Settings,
        _load_context: &mut LoadContext<'_>,
    ) -> Result<Self::Asset, Self::Error> {
        let mut bytes = Vec::new();
        reader.read_to_end(&mut bytes).await?;
        let data: DriverSchedule = serde_json::from_slice(&bytes)?;
        Ok(DriverScheduleAsset(data))
    }

    fn extensions(&self) -> &[&str] {
        &["scenario.json"]
    }
}

// ============================================================================
// Plugin
// ============================================================================

/// Plugin that registers JSON asset loaders
pub struct JsonAssetPlugin;

impl Plugin for JsonAssetPlugin {
    fn build(&self, app: &mut App) {
        app.init_asset::<ChargersAsset>()
            .init_asset::<DriverScheduleAsset>()
            .init_asset_loader::<ChargersLoader>()
            .init_asset_loader::<DriverScheduleLoader>();
        // Note: GameDataAssets and SiteTemplateCache are registered by ResourcesPlugin
    }
}
