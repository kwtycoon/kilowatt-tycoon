//! Shared photovoltaic canopy layout helpers.
//!
//! These helpers keep canopy render geometry in one place so the build preview
//! and world render stay aligned when assets or offsets change.

use bevy::prelude::*;

use crate::resources::{PhotovoltaicCanopyPlacement, sprite_metadata};

/// Resolved render/layout data for a photovoltaic canopy.
#[derive(Debug, Clone, Copy)]
pub struct CanopyRenderLayout {
    pub sprite_bottom_center_world: Vec2,
    pub sprite_size: Vec2,
    pub sprite_scale_sign: Vec2,
}

pub fn canopy_sprite_size() -> Vec2 {
    let size = sprite_metadata::photovoltaic_canopy_world_size();
    Vec2::new(size.width, size.height.unwrap_or(size.width))
}

pub fn canopy_layout(canopy: &PhotovoltaicCanopyPlacement) -> CanopyRenderLayout {
    CanopyRenderLayout {
        sprite_bottom_center_world: canopy.sprite_bottom_center_world(),
        sprite_size: canopy.sprite_world_size(),
        sprite_scale_sign: canopy.sprite_scale_sign(),
    }
}

pub fn scale_to_size(image: &Image, target_size: Vec2) -> Vec3 {
    let size = image.size().as_vec2();
    Vec3::new(target_size.x / size.x, target_size.y / size.y, 1.0)
}

pub fn canopy_scale(image: Option<&Image>, layout: &CanopyRenderLayout) -> Vec3 {
    let base_scale = image
        .map(|image| scale_to_size(image, layout.sprite_size))
        .unwrap_or(Vec3::splat(0.25));
    Vec3::new(
        base_scale.x * layout.sprite_scale_sign.x,
        base_scale.y * layout.sprite_scale_sign.y,
        1.0,
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::resources::{CanopyOrientation, SiteGrid, TILE_SIZE};

    fn placement(orientation: CanopyOrientation) -> PhotovoltaicCanopyPlacement {
        let charger_pos = match orientation {
            CanopyOrientation::ChargersNorthOfBay => (5, 6),
            CanopyOrientation::ChargersSouthOfBay => (5, 4),
        };
        PhotovoltaicCanopyPlacement {
            anchor_bay_pos: (5, 5),
            covered_bay_positions: vec![(5, 5)],
            covered_charger_positions: vec![charger_pos],
            orientation,
            span_chargers: 1,
            installed_kw_peak: 6.0,
        }
    }

    #[test]
    fn north_of_bay_layout_bottom_center_tracks_charger_tile() {
        let layout = canopy_layout(&placement(CanopyOrientation::ChargersNorthOfBay));
        assert_eq!(
            layout.sprite_bottom_center_world,
            SiteGrid::grid_to_world(5, 6) + Vec2::new(0.0, -TILE_SIZE * 0.5)
        );
        assert_eq!(layout.sprite_size, Vec2::splat(128.0));
        assert!(layout.sprite_scale_sign.y > 0.0);
    }

    #[test]
    fn south_of_bay_layout_bottom_center_tracks_charger_tile() {
        let layout = canopy_layout(&placement(CanopyOrientation::ChargersSouthOfBay));
        assert_eq!(
            layout.sprite_bottom_center_world,
            SiteGrid::grid_to_world(5, 4) + Vec2::new(0.0, TILE_SIZE * 0.5)
        );
        assert_eq!(layout.sprite_size, Vec2::splat(128.0));
        assert!(layout.sprite_scale_sign.y < 0.0);
    }
}
