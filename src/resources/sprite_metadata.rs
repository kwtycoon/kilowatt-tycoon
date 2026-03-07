//! Sprite display size metadata
//!
//! This module defines the intended world sizes for sprites.
//! The actual PNG dimensions are read at runtime and scales are calculated
//! automatically, making the system maintainable and flexible.
//!
//! Benefits:
//! - Artists can change PNG resolution without code changes
//! - No magic scale constants scattered across files
//! - Clear intent: "this sprite should be X pixels wide in-game"
//! - Data-driven: scale = intended_size / actual_png_size

use crate::components::charger::ChargerType;
use bevy::prelude::*;

/// Intended display size for a sprite in world pixels
#[derive(Debug, Clone, Copy)]
pub struct SpriteSize {
    /// Intended width in world pixels
    pub width: f32,
    /// Intended height in world pixels (or None to preserve aspect ratio)
    pub height: Option<f32>,
}

impl SpriteSize {
    /// Create a sprite size with just width (preserves aspect ratio)
    pub const fn width(width: f32) -> Self {
        Self {
            width,
            height: None,
        }
    }

    /// Create a sprite size with explicit width and height
    pub const fn size(width: f32, height: f32) -> Self {
        Self {
            width,
            height: Some(height),
        }
    }

    /// Calculate uniform scale to fit the intended width
    ///
    /// This divides intended width by actual PNG width to get the scale factor.
    /// Example: If we want 32px wide and PNG is 256px, scale = 32/256 = 0.125
    pub fn scale_for_image(&self, image: &Image) -> f32 {
        let actual_size = image.size().as_vec2();
        self.width / actual_size.x
    }
}

/// Get intended world size for charger sprites
///
/// These sizes define how large chargers should appear in-game.
/// The actual scale is calculated from the PNG dimensions at runtime.
///
/// Size guidelines:
/// - L2 chargers are smallest (wall-mounted, compact)
/// - DCFC chargers scale with power rating (larger = more powerful)
/// - Sizes chosen to fit in parking spaces while remaining visible
pub fn charger_world_size(charger_type: ChargerType, power_kw: f32) -> SpriteSize {
    match charger_type {
        ChargerType::AcLevel2 => SpriteSize::width(32.0),
        ChargerType::DcFast if power_kw <= 75.0 => SpriteSize::width(35.0), // DCFC 50kW
        ChargerType::DcFast if power_kw <= 125.0 => SpriteSize::width(38.0), // DCFC 100kW
        ChargerType::DcFast if power_kw <= 200.0 => SpriteSize::width(38.0), // DCFC 150kW
        ChargerType::DcFast => SpriteSize::width(42.0),                     // DCFC 350kW
    }
}

/// Get intended world size for prop sprites (transformers, solar, battery, amenities)
///
/// Props are sized to match their tile footprint (2x2, 3x2, etc.)
pub fn prop_world_size(tile_width: f32, tile_height: f32) -> SpriteSize {
    let tile_size = crate::resources::TILE_SIZE;
    SpriteSize::size(tile_width * tile_size, tile_height * tile_size)
}

/// Get the intended footprint for a single photovoltaic canopy overlay.
pub fn photovoltaic_canopy_world_size() -> SpriteSize {
    let tile_size = crate::resources::TILE_SIZE;
    SpriteSize::size(tile_size * 2.0, tile_size * 2.0)
}

/// Get intended world size for vehicle sprites
///
/// These sizes define how large vehicles should appear in-game.
/// Based on the existing `vehicle_dimensions()` function, which defines
/// the intended display size for each vehicle type.
pub fn vehicle_world_size(vehicle_type: crate::components::driver::VehicleType) -> SpriteSize {
    use crate::components::driver::VehicleType;

    match vehicle_type {
        VehicleType::Compact => SpriteSize::width(35.0),
        VehicleType::Sedan => SpriteSize::width(40.0),
        VehicleType::Suv => SpriteSize::width(45.0),
        VehicleType::Crossover => SpriteSize::width(42.0),
        VehicleType::Pickup => SpriteSize::width(48.0),
        VehicleType::Bus => SpriteSize::width(55.0),
        VehicleType::Semi => SpriteSize::width(58.0),
        VehicleType::Tractor => SpriteSize::width(50.0),
        VehicleType::Scooter => SpriteSize::width(22.0),
        VehicleType::Motorcycle => SpriteSize::width(26.0),
        VehicleType::Firetruck => SpriteSize::width(60.0),
    }
}

/// Get intended world size for UI icon sprites
///
/// These are small icons that appear above entities (mood, warnings, etc.)
pub fn icon_world_size() -> SpriteSize {
    SpriteSize::width(19.2) // 0.3 scale for typical 64px icons
}

/// Get intended world size for VFX sprites
///
/// Visual effects like floating money, wrenches, pulse effects
#[derive(Debug, Clone, Copy)]
pub enum VfxType {
    // Floating indicators
    FloatingMoney,
    FloatingWrench,
    FloatingRepLoss,

    // Pulses and alerts
    UrgentPulse,
    LightPulseGreen,
    LightPulseBlue,
    LightPulseYellow,
    LightPulseRed,

    // UI elements
    Selection,
    PlacementCursor,

    // Legacy (TODO: verify if still needed)
    FaultPulse,
    BrokenChargerIcon,
    FrustrationIcon,
    PowerThrottleIcon,
}

pub fn vfx_world_size(vfx_type: VfxType) -> SpriteSize {
    match vfx_type {
        // Floating indicators
        VfxType::FloatingMoney => SpriteSize::width(51.2), // 0.8 scale
        VfxType::FloatingWrench => SpriteSize::width(51.2), // 0.8 scale
        VfxType::FloatingRepLoss => SpriteSize::width(51.2), // 0.8 scale

        // Pulses and alerts
        VfxType::UrgentPulse => SpriteSize::width(64.0), // 1.0 scale
        VfxType::LightPulseGreen => SpriteSize::width(16.0), // 0.25 scale
        VfxType::LightPulseBlue => SpriteSize::width(16.0), // 0.25 scale
        VfxType::LightPulseYellow => SpriteSize::width(16.0), // 0.25 scale
        VfxType::LightPulseRed => SpriteSize::width(16.0), // 0.25 scale

        // UI elements
        VfxType::Selection => SpriteSize::width(64.0), // 1.0 scale
        VfxType::PlacementCursor => SpriteSize::width(64.0), // 1.0 scale

        // Legacy (TODO: verify if still needed)
        VfxType::FaultPulse => SpriteSize::width(25.6), // 0.4 scale
        VfxType::BrokenChargerIcon => SpriteSize::width(38.4), // 0.6 scale
        VfxType::FrustrationIcon => SpriteSize::width(32.0), // 0.5 scale
        VfxType::PowerThrottleIcon => SpriteSize::width(32.0), // 0.5 scale
    }
}

/// Get intended world size for technician sprites
pub fn technician_world_size() -> SpriteSize {
    SpriteSize::width(45.0) // Similar to vehicle size for realistic proportions
}

/// Get intended world size for robber sprites (10% smaller than technician)
pub fn robber_world_size() -> SpriteSize {
    SpriteSize::width(40.5) // 45 * 0.9 — slightly smaller than technician
}

/// Get intended world size for hacker sprites (same proportions as robber)
pub fn hacker_world_size() -> SpriteSize {
    SpriteSize::width(40.5)
}

/// Get intended world size for tile sprites (roads, parking, etc.)
pub fn tile_world_size() -> SpriteSize {
    let tile_size = crate::resources::TILE_SIZE;
    SpriteSize::width(tile_size) // Tiles are rendered at TILE_SIZE (64px)
}

/// Helper to calculate prop scale from tile multiplier
///
/// Legacy props use multipliers of TILE_SCALE (which = 1.0 now).
/// This allows gradual migration without breaking existing code.
pub fn tile_scale_multiplier(multiplier: f32) -> f32 {
    multiplier // With TILE_SCALE = 1.0, this is just the multiplier
}
