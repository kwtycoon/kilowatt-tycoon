//! Build mode input handling - click to place, drag to place, sell
//!
//! Uses PNG assets for the placement cursor (generated from SVG source files).

use crate::components::power::Transformer;
use crate::helpers::GamePointer;
use crate::resources::{
    AmenityType, BuildState, BuildTool, ChargerPadType, GameState, ImageAssets, MultiSiteManager,
    SellResult, SiteGrid, TileContent,
};
use crate::systems::WorldCamera;
use bevy::prelude::*;

/// Marker for the placement cursor preview
#[derive(Component)]
pub struct PlacementCursor;

/// Marker for the sell cursor highlight overlay
#[derive(Component)]
pub struct SellCursor;

/// Get the image handle for a build tool
fn get_tool_image(tool: BuildTool, assets: &ImageAssets) -> Option<Handle<Image>> {
    match tool {
        BuildTool::Select | BuildTool::Road | BuildTool::ParkingBay | BuildTool::Sell => None,
        BuildTool::ChargerL2 => Some(assets.charger_l2_available.clone()),
        BuildTool::ChargerDCFC50 => Some(assets.charger_dcfc50_available.clone()),
        BuildTool::ChargerDCFC100 => Some(assets.charger_dcfc100_available.clone()),
        BuildTool::ChargerDCFC150 => Some(assets.charger_dcfc150_available.clone()),
        BuildTool::ChargerDCFC350 => Some(assets.charger_dcfc350_available.clone()),
        BuildTool::Transformer100kVA
        | BuildTool::Transformer500kVA
        | BuildTool::Transformer1000kVA
        | BuildTool::Transformer2500kVA => Some(assets.prop_transformer.clone()),
        BuildTool::SolarCanopy => Some(assets.prop_solar_array_ground.clone()),
        BuildTool::BatteryStorage => Some(assets.prop_battery_container.clone()),
        BuildTool::SecuritySystem => Some(assets.prop_security_system.clone()),
        BuildTool::AmenityWifiRestrooms => Some(assets.prop_amenity_wifi_restrooms.clone()),
        BuildTool::AmenityLoungeSnacks => Some(assets.prop_amenity_lounge_snacks.clone()),
        BuildTool::AmenityRestaurant => Some(assets.prop_amenity_restaurant_premium.clone()),
        BuildTool::AmenityDriverRestLounge => Some(assets.prop_amenity_driver_rest_lounge.clone()),
    }
}

/// Get the visual scale for a build tool's preview sprite using actual PNG dimensions
fn get_tool_visual_scale(tool: BuildTool, image: &Image) -> f32 {
    use crate::components::charger::ChargerType;
    use crate::resources::sprite_metadata;

    let intended_size = match tool {
        BuildTool::Select | BuildTool::Road | BuildTool::ParkingBay | BuildTool::Sell => {
            // These don't have visual previews
            return 1.0;
        }
        // Chargers: use sprite metadata for intended world size
        BuildTool::ChargerL2 => sprite_metadata::charger_world_size(ChargerType::AcLevel2, 7.0),
        BuildTool::ChargerDCFC50 => sprite_metadata::charger_world_size(ChargerType::DcFast, 50.0),
        BuildTool::ChargerDCFC100 => {
            sprite_metadata::charger_world_size(ChargerType::DcFast, 100.0)
        }
        BuildTool::ChargerDCFC150 => {
            sprite_metadata::charger_world_size(ChargerType::DcFast, 150.0)
        }
        BuildTool::ChargerDCFC350 => {
            sprite_metadata::charger_world_size(ChargerType::DcFast, 350.0)
        }
        // Props: use tile-based sizing
        BuildTool::Transformer100kVA
        | BuildTool::Transformer500kVA
        | BuildTool::Transformer1000kVA
        | BuildTool::Transformer2500kVA => sprite_metadata::prop_world_size(2.0, 2.0),
        BuildTool::SolarCanopy => sprite_metadata::prop_world_size(3.0, 2.0),
        BuildTool::BatteryStorage => sprite_metadata::prop_world_size(2.0, 2.0),
        BuildTool::SecuritySystem => sprite_metadata::prop_world_size(2.0, 2.0),
        BuildTool::AmenityWifiRestrooms => sprite_metadata::prop_world_size(3.0, 3.0),
        BuildTool::AmenityLoungeSnacks => sprite_metadata::prop_world_size(4.0, 4.0),
        BuildTool::AmenityRestaurant => sprite_metadata::prop_world_size(5.0, 4.0),
        BuildTool::AmenityDriverRestLounge => sprite_metadata::prop_world_size(3.0, 3.0),
    };

    intended_size.scale_for_image(image)
}

/// System to handle mouse clicks for tile placement
pub fn build_placement_system(
    mut build_state: ResMut<BuildState>,
    mut game_state: ResMut<GameState>,
    mut multi_site: ResMut<MultiSiteManager>,
    pointer: Res<GamePointer>,
    camera_query: Query<(&Camera, &GlobalTransform), With<WorldCamera>>,
    ui_interactions: Query<&Interaction>,
    transformers: Query<&Transformer>,
) {
    // Block placement when pointer is over any UI element (buttons, panels, etc.)
    if pointer.just_pressed || pointer.pressed {
        for interaction in &ui_interactions {
            if *interaction != Interaction::None {
                return;
            }
        }
    }

    // Get active site's world offset first (before mutable borrow)
    let world_offset = {
        let Some(site) = multi_site.active_site() else {
            return;
        };
        site.world_offset()
    };

    // Get active site grid and pending video ad chargers or return early
    let Some(site) = multi_site.active_site_mut() else {
        return;
    };
    let site_id = site.id;
    let grid = &mut site.grid;
    let pending_video_ad_chargers = &mut site.pending_video_ad_chargers;

    // Building is allowed anytime in Playing state

    // Get camera
    let Ok((camera, camera_transform)) = camera_query.single() else {
        return;
    };

    let Some(cursor_pos) = pointer.screen_position else {
        return;
    };

    // Convert to world coordinates
    let Ok(world_pos) = camera.viewport_to_world_2d(camera_transform, cursor_pos) else {
        return;
    };

    // Subtract site world offset before converting to grid coordinates
    let grid_relative_pos = world_pos - world_offset;
    let (grid_x, grid_y) = SiteGrid::world_to_grid(grid_relative_pos);

    // Check if valid grid position
    if !grid.is_valid(grid_x, grid_y) {
        return;
    }

    // Handle press
    if pointer.just_pressed {
        build_state.is_dragging = true;
        build_state.last_placed_tile = None;
        try_place_tile(
            &mut build_state,
            &mut game_state,
            grid,
            pending_video_ad_chargers,
            site_id,
            &transformers,
            grid_x,
            grid_y,
        );
    }

    // Handle dragging (for roads especially)
    if pointer.pressed && build_state.is_dragging {
        // Only place if we moved to a new tile
        if build_state.last_placed_tile != Some((grid_x, grid_y)) {
            try_place_tile(
                &mut build_state,
                &mut game_state,
                grid,
                pending_video_ad_chargers,
                site_id,
                &transformers,
                grid_x,
                grid_y,
            );
        }
    }

    // Handle release
    if pointer.just_released {
        build_state.is_dragging = false;
        build_state.last_placed_tile = None;
    }
}

/// Try to place the currently selected tool at the given grid position
fn try_place_tile(
    build_state: &mut BuildState,
    game_state: &mut GameState,
    grid: &mut crate::resources::SiteGrid,
    pending_video_ad_chargers: &mut std::collections::HashSet<(i32, i32)>,
    site_id: crate::resources::SiteId,
    transformers: &Query<&Transformer>,
    x: i32,
    y: i32,
) {
    let tool = build_state.selected_tool;
    let cost = tool.cost();

    match tool {
        BuildTool::Select | BuildTool::Road | BuildTool::ParkingBay => {
            // Select mode or deprecated tools - don't place anything
        }
        BuildTool::ChargerL2
        | BuildTool::ChargerDCFC50
        | BuildTool::ChargerDCFC100
        | BuildTool::ChargerDCFC150
        | BuildTool::ChargerDCFC350 => {
            // User clicked on pad position - find the adjacent parking bay
            let bay_pos = if grid.get_content(x, y - 1) == TileContent::ParkingBaySouth {
                Some((x, y - 1))
            } else if grid.get_content(x, y + 1) == TileContent::ParkingBayNorth {
                Some((x, y + 1))
            } else {
                None
            };

            if let Some((bay_x, bay_y)) = bay_pos {
                let charger_type = match tool {
                    BuildTool::ChargerL2 => ChargerPadType::L2,
                    BuildTool::ChargerDCFC50 => ChargerPadType::DCFC50,
                    BuildTool::ChargerDCFC100 => ChargerPadType::DCFC100,
                    BuildTool::ChargerDCFC150 => ChargerPadType::DCFC150,
                    BuildTool::ChargerDCFC350 => ChargerPadType::DCFC350,
                    _ => unreachable!(),
                };

                if game_state.can_afford_build(cost)
                    && grid.place_charger(bay_x, bay_y, charger_type).is_ok()
                {
                    game_state.try_spend_build(cost);
                    build_state.last_placed_tile = Some((x, y));

                    // DC 100kW has built-in video ads - auto-enable
                    if charger_type.has_built_in_video_ads() {
                        pending_video_ad_chargers.insert((x, y));
                        info!(
                            "Placed {:?} charger with built-in video ads at pad ({}, {}) for bay ({}, {})",
                            charger_type, x, y, bay_x, bay_y
                        );
                    } else {
                        info!(
                            "Placed {:?} charger at pad ({}, {}) for bay ({}, {})",
                            charger_type, x, y, bay_x, bay_y
                        );
                    }
                }
            }
        }
        BuildTool::Transformer100kVA
        | BuildTool::Transformer500kVA
        | BuildTool::Transformer1000kVA
        | BuildTool::Transformer2500kVA => {
            let kva = tool.transformer_kva().unwrap_or(500.0);
            if game_state.can_afford_build(cost) && grid.place_transformer(x, y, kva).is_ok() {
                game_state.try_spend_build(cost);
                build_state.last_placed_tile = Some((x, y));
                info!("Placed {} kVA transformer at ({}, {})", kva as i32, x, y);
            }
        }
        BuildTool::SolarCanopy => {
            if game_state.can_afford_build(cost) && grid.place_solar(x, y).is_ok() {
                game_state.try_spend_build(cost);
                build_state.last_placed_tile = Some((x, y));
                info!("Placed solar array at ({}, {})", x, y);
            }
        }
        BuildTool::BatteryStorage => {
            if game_state.can_afford_build(cost) && grid.place_battery(x, y).is_ok() {
                game_state.try_spend_build(cost);
                build_state.last_placed_tile = Some((x, y));
                info!("Placed battery storage (2x2) at ({}, {})", x, y);
            }
        }
        BuildTool::SecuritySystem => {
            if game_state.can_afford_build(cost) && grid.place_security_system(x, y).is_ok() {
                game_state.try_spend_build(cost);
                build_state.last_placed_tile = Some((x, y));
                info!("Placed security system (2x2) at ({}, {})", x, y);
            }
        }
        BuildTool::AmenityWifiRestrooms => {
            if game_state.can_afford_build(cost)
                && grid.place_amenity(x, y, AmenityType::WifiRestrooms).is_ok()
            {
                game_state.try_spend_build(cost);
                build_state.last_placed_tile = Some((x, y));
                info!("Placed WiFi+Restrooms (3x3) at ({}, {})", x, y);
            }
        }
        BuildTool::AmenityLoungeSnacks => {
            if game_state.can_afford_build(cost)
                && grid.place_amenity(x, y, AmenityType::LoungeSnacks).is_ok()
            {
                game_state.try_spend_build(cost);
                build_state.last_placed_tile = Some((x, y));
                info!("Placed Lounge+Snacks (4x4) at ({}, {})", x, y);
            }
        }
        BuildTool::AmenityRestaurant => {
            if game_state.can_afford_build(cost)
                && grid.place_amenity(x, y, AmenityType::Restaurant).is_ok()
            {
                game_state.try_spend_build(cost);
                build_state.last_placed_tile = Some((x, y));
                info!("Placed Restaurant (5x4) at ({}, {})", x, y);
            }
        }
        BuildTool::AmenityDriverRestLounge => {
            if game_state.can_afford_build(cost)
                && grid
                    .place_amenity(x, y, AmenityType::DriverRestLounge)
                    .is_ok()
            {
                game_state.try_spend_build(cost);
                build_state.last_placed_tile = Some((x, y));
                info!("Placed Driver Rest Lounge (3x3) at ({}, {})", x, y);
            }
        }
        BuildTool::Sell => {
            // Burned transformers are forced demolition: $0 resale.
            let destroyed_transformer_target = {
                let content = grid.get_content(x, y);
                if matches!(
                    content,
                    TileContent::TransformerPad | TileContent::TransformerOccupied
                ) {
                    let anchor = if content == TileContent::TransformerPad {
                        Some((x, y))
                    } else {
                        grid.get_tile(x, y).and_then(|t| t.anchor_pos)
                    };
                    if let Some((ax, ay)) = anchor {
                        transformers
                            .iter()
                            .any(|t| t.site_id == site_id && t.grid_pos == (ax, ay) && t.destroyed)
                    } else {
                        false
                    }
                } else {
                    false
                }
            };

            if let Ok(sold) = grid.sell(x, y) {
                // Refund based on what was sold
                let refund = match sold {
                    SellResult::SoldCharger(charger_type) => {
                        // Refund charger costs (50% via refund function)
                        match charger_type {
                            ChargerPadType::L2 => 3000,
                            ChargerPadType::DCFC50 => 40000,
                            ChargerPadType::DCFC100 => 60000,
                            ChargerPadType::DCFC150 => 80000,
                            ChargerPadType::DCFC350 => 150000,
                        }
                    }
                    SellResult::SoldEquipment(content) => match content {
                        TileContent::TransformerPad => {
                            if destroyed_transformer_target {
                                0
                            } else {
                                50000
                            }
                        } // 2x2 transformer
                        TileContent::SolarPad => 24000,   // 3x2 solar
                        TileContent::BatteryPad => 50000, // 2x2 battery
                        TileContent::SecurityPad => 80000, // 2x2 security system
                        TileContent::AmenityWifiRestrooms => 15000, // 3x3
                        TileContent::AmenityLoungeSnacks => 50000, // 4x4
                        TileContent::AmenityRestaurant => 150000, // 5x4
                        TileContent::AmenityDriverRestLounge => 25000, // 3x3
                        _ => 0,
                    },
                };
                if refund > 0 {
                    game_state.refund_build(refund); // refund function gives 50% of the passed cost
                    let actual_refund = refund / 2;
                    info!("Sold at ({}, {}), refunded ${}", x, y, actual_refund);
                }
                build_state.last_placed_tile = Some((x, y));
            }
        }
    }
}

/// Spawn/update placement cursor preview
pub fn update_placement_cursor(
    mut commands: Commands,
    build_state: Res<BuildState>,
    multi_site: Res<MultiSiteManager>,
    game_state: Res<crate::resources::GameState>,
    image_assets: Res<ImageAssets>,
    images: Res<Assets<Image>>,
    pointer: Res<GamePointer>,
    camera_query: Query<(&Camera, &GlobalTransform), With<WorldCamera>>,
    mut cursor_query: Query<(Entity, &mut Transform, &mut Sprite), With<PlacementCursor>>,
) {
    // Get active site grid and world offset
    let (world_offset, grid) = {
        let Some(site) = multi_site.active_site() else {
            return;
        };
        (site.world_offset(), &site.grid)
    };

    let tool = build_state.selected_tool;

    // Get the tool's image and scale
    let Some(tool_image) = get_tool_image(tool, &image_assets) else {
        // No visual preview for Select, Road, ParkingBay, Sell - despawn cursor
        for (entity, _, _) in &cursor_query {
            commands.entity(entity).try_despawn();
        }
        return;
    };

    // Get camera
    let Ok((camera, camera_transform)) = camera_query.single() else {
        return;
    };

    let Some(cursor_pos) = pointer.screen_position else {
        // Hide cursor if no pointer position
        for (entity, _, _) in &cursor_query {
            commands.entity(entity).try_despawn();
        }
        return;
    };

    // Convert to world coordinates
    let Ok(world_pos) = camera.viewport_to_world_2d(camera_transform, cursor_pos) else {
        return;
    };

    // Subtract site world offset before converting to grid coordinates
    let grid_relative_pos = world_pos - world_offset;
    let (grid_x, grid_y) = SiteGrid::world_to_grid(grid_relative_pos);

    // Validate placement and determine color
    let is_valid = validate_placement(&tool, grid, &game_state, grid_x, grid_y);
    let color = if is_valid {
        Color::srgba(0.2, 1.0, 0.2, 0.5) // Semi-transparent green
    } else {
        Color::srgba(1.0, 0.2, 0.2, 0.5) // Semi-transparent red
    };

    // Determine position based on structure size (in grid space)
    let grid_preview_pos = if let Some(size) = tool.structure_size() {
        // Multi-tile structure: center on the footprint
        SiteGrid::multi_tile_center(grid_x, grid_y, size)
    } else {
        // Single tile (including chargers): center on clicked tile
        // Chargers are now placed by clicking on the ChargerPad location directly
        SiteGrid::grid_to_world(grid_x, grid_y)
    };

    // Add world offset to position cursor in world space
    let world_preview_pos = grid_preview_pos + world_offset;
    let (preview_x, preview_y) = (world_preview_pos.x, world_preview_pos.y);

    // Calculate scale from actual PNG dimensions
    let scale = if let Some(image) = images.get(&tool_image) {
        get_tool_visual_scale(tool, image)
    } else {
        // Fallback if image not loaded yet
        0.125
    };

    // Update or spawn cursor
    if let Some((_, mut transform, mut sprite)) = cursor_query.iter_mut().next() {
        transform.translation = Vec3::new(preview_x, preview_y, 100.0);
        transform.scale = Vec3::splat(scale);
        sprite.image = tool_image;
        sprite.color = color;
    } else {
        // Spawn sprite cursor with the actual tool image
        commands.spawn((
            Sprite {
                image: tool_image,
                color,
                ..default()
            },
            Transform::from_xyz(preview_x, preview_y, 100.0).with_scale(Vec3::splat(scale)),
            PlacementCursor,
        ));
    }
}

/// Validate if a tool can be placed at the given position
fn validate_placement(
    tool: &BuildTool,
    grid: &SiteGrid,
    game_state: &crate::resources::GameState,
    x: i32,
    y: i32,
) -> bool {
    // Check if we can afford it
    if !game_state.can_afford_build(tool.cost()) {
        return false;
    }

    // Check valid grid coordinates
    if !grid.is_valid(x, y) {
        return false;
    }

    match tool {
        BuildTool::Select | BuildTool::Road | BuildTool::ParkingBay | BuildTool::Sell => false,
        BuildTool::ChargerL2
        | BuildTool::ChargerDCFC50
        | BuildTool::ChargerDCFC100
        | BuildTool::ChargerDCFC150
        | BuildTool::ChargerDCFC350 => {
            // User clicks on the ChargerPad location (Lot/Grass/Empty tile)
            // We need to find the adjacent parking bay that uses this spot
            let pad_content = grid.get_content(x, y);

            // Check if this is a valid charger pad location
            // ChargerPad tiles from TMX are the primary placement spots
            if !matches!(
                pad_content,
                TileContent::ChargerPad
                    | TileContent::Lot
                    | TileContent::Grass
                    | TileContent::Empty
                    | TileContent::WheelStop
            ) {
                return false;
            }

            // Find adjacent parking bay that would use this position as its charger pad
            // ParkingBaySouth at y-1 has charger at y (current position)
            // ParkingBayNorth at y+1 has charger at y (current position)
            let bay_pos = if grid.get_content(x, y - 1) == TileContent::ParkingBaySouth {
                Some((x, y - 1))
            } else if grid.get_content(x, y + 1) == TileContent::ParkingBayNorth {
                Some((x, y + 1))
            } else {
                None
            };

            let Some((bay_x, bay_y)) = bay_pos else {
                return false; // No adjacent parking bay
            };

            // Check bay doesn't already have a charger
            if let Some(tile) = grid.get_tile(bay_x, bay_y) {
                if tile.linked_charger_pad.is_some() || tile.has_adjacent_charger {
                    return false;
                }
            } else {
                return false;
            }

            true
        }
        BuildTool::Transformer100kVA
        | BuildTool::Transformer500kVA
        | BuildTool::Transformer1000kVA
        | BuildTool::Transformer2500kVA => {
            // Multiple transformers allowed - just check footprint
            grid.can_place_footprint(x, y, crate::resources::StructureSize::TwoByTwo)
                .is_ok()
        }
        BuildTool::SolarCanopy => grid
            .can_place_footprint(x, y, crate::resources::StructureSize::ThreeByTwo)
            .is_ok(),
        BuildTool::BatteryStorage => grid
            .can_place_footprint(x, y, crate::resources::StructureSize::TwoByTwo)
            .is_ok(),
        BuildTool::SecuritySystem => grid
            .can_place_footprint(x, y, crate::resources::StructureSize::TwoByTwo)
            .is_ok(),
        BuildTool::AmenityWifiRestrooms => grid
            .can_place_footprint(x, y, crate::resources::StructureSize::ThreeByThree)
            .is_ok(),
        BuildTool::AmenityLoungeSnacks => grid
            .can_place_footprint(x, y, crate::resources::StructureSize::FourByFour)
            .is_ok(),
        BuildTool::AmenityRestaurant => grid
            .can_place_footprint(x, y, crate::resources::StructureSize::FiveByFour)
            .is_ok(),
        BuildTool::AmenityDriverRestLounge => grid
            .can_place_footprint(x, y, crate::resources::StructureSize::ThreeByThree)
            .is_ok(),
    }
}

/// Update/spawn sell cursor highlight overlay when Sell tool is active
pub fn update_sell_cursor(
    mut commands: Commands,
    build_state: Res<BuildState>,
    multi_site: Res<MultiSiteManager>,
    pointer: Res<GamePointer>,
    camera_query: Query<(&Camera, &GlobalTransform), With<WorldCamera>>,
    mut cursor_query: Query<(Entity, &mut Transform, &mut Sprite), With<SellCursor>>,
) {
    // Get active site grid and world offset
    let (world_offset, grid) = {
        let Some(site) = multi_site.active_site() else {
            return;
        };
        (site.world_offset(), &site.grid)
    };

    let tool = build_state.selected_tool;

    // Only show sell cursor when Sell tool is active
    if tool != BuildTool::Sell {
        // Despawn cursor if it exists
        for (entity, _, _) in &cursor_query {
            commands.entity(entity).try_despawn();
        }
        return;
    }

    // Get camera
    let Ok((camera, camera_transform)) = camera_query.single() else {
        return;
    };

    let Some(cursor_pos) = pointer.screen_position else {
        // Hide cursor if no pointer position
        for (entity, _, _) in &cursor_query {
            commands.entity(entity).try_despawn();
        }
        return;
    };

    // Convert to world coordinates
    let Ok(world_pos) = camera.viewport_to_world_2d(camera_transform, cursor_pos) else {
        return;
    };

    // Subtract site world offset before converting to grid coordinates
    let grid_relative_pos = world_pos - world_offset;
    let (grid_x, grid_y) = SiteGrid::world_to_grid(grid_relative_pos);

    // Check if valid grid position
    if !grid.is_valid(grid_x, grid_y) {
        // Hide cursor if out of bounds
        for (entity, _, _) in &cursor_query {
            commands.entity(entity).try_despawn();
        }
        return;
    }

    let content = grid.get_content(grid_x, grid_y);

    // Determine what we're hovering over and if it can be sold
    let (can_sell, size, grid_highlight_pos) = determine_sell_target(grid, grid_x, grid_y, content);

    // If there's nothing to sell (grass, empty), hide cursor
    if can_sell.is_none() {
        for (entity, _, _) in &cursor_query {
            commands.entity(entity).try_despawn();
        }
        return;
    }

    // Determine color based on sellability
    let color = match can_sell {
        Some(true) => Color::srgba(1.0, 0.8, 0.0, 0.6), // Yellow/orange for sellable
        Some(false) => Color::srgba(1.0, 0.2, 0.2, 0.6), // Red for protected/unsellable
        None => {
            // Hide cursor
            for (entity, _, _) in &cursor_query {
                commands.entity(entity).try_despawn();
            }
            return;
        }
    };

    // Apply world offset to highlight position
    let world_highlight_pos = (
        grid_highlight_pos.0 + world_offset.x,
        grid_highlight_pos.1 + world_offset.y,
    );

    // Calculate size for the highlight rectangle
    let tile_size = crate::resources::TILE_SIZE;
    let (width, height) = match size {
        (1, 1) => (tile_size, tile_size),
        (w, h) => (tile_size * w as f32, tile_size * h as f32),
    };

    // Update or spawn cursor
    if let Some((_, mut transform, mut sprite)) = cursor_query.iter_mut().next() {
        transform.translation = Vec3::new(world_highlight_pos.0, world_highlight_pos.1, 99.0);
        sprite.color = color;
        sprite.custom_size = Some(Vec2::new(width, height));
    } else {
        // Spawn rectangle highlight
        commands.spawn((
            Sprite {
                color,
                custom_size: Some(Vec2::new(width, height)),
                ..default()
            },
            Transform::from_xyz(world_highlight_pos.0, world_highlight_pos.1, 99.0),
            SellCursor,
        ));
    }
}

/// Determine what's being targeted for selling and if it can be sold
/// Returns (Some(can_sell), size, position) or (None, _, _) if nothing to highlight
fn determine_sell_target(
    grid: &SiteGrid,
    x: i32,
    y: i32,
    content: TileContent,
) -> (Option<bool>, (i32, i32), (f32, f32)) {
    use crate::resources::StructureSize;

    // Check if there's a charger on this tile
    if let Some(tile) = grid.get_tile(x, y) {
        if let Some((pad_x, pad_y)) = tile.linked_charger_pad {
            // Highlight charger at ChargerPad position
            let charger_pad_pos = SiteGrid::grid_to_world(pad_x, pad_y);
            return (Some(true), (1, 1), (charger_pad_pos.x, charger_pad_pos.y));
        } else if tile.has_adjacent_charger {
            // Legacy: old style charger on bay (backward compat)
            let world_pos = SiteGrid::grid_to_world(x, y);
            let charger_y = world_pos.y + crate::resources::TILE_SIZE / 2.0 - 8.0;
            return (Some(true), (1, 1), (world_pos.x, charger_y));
        }
    }

    // Check various content types
    match content {
        TileContent::Grass | TileContent::Empty => {
            // Nothing to sell
            (None, (1, 1), (0.0, 0.0))
        }
        TileContent::Road | TileContent::ParkingBayNorth | TileContent::ParkingBaySouth => {
            // Protected - cannot sell
            let world_pos = SiteGrid::grid_to_world(x, y);
            (Some(false), (1, 1), (world_pos.x, world_pos.y))
        }
        TileContent::Entry | TileContent::Exit => {
            // Protected - cannot sell
            let world_pos = SiteGrid::grid_to_world(x, y);
            (Some(false), (1, 1), (world_pos.x, world_pos.y))
        }
        TileContent::TransformerPad => {
            // Find anchor and highlight 2x2
            let (anchor_x, anchor_y) = find_anchor(grid, x, y, content);
            let center = SiteGrid::multi_tile_center(anchor_x, anchor_y, StructureSize::TwoByTwo);
            (Some(true), (2, 2), (center.x, center.y))
        }
        TileContent::SolarPad => {
            // Find anchor and highlight 3x2
            let (anchor_x, anchor_y) = find_anchor(grid, x, y, content);
            let center = SiteGrid::multi_tile_center(anchor_x, anchor_y, StructureSize::ThreeByTwo);
            (Some(true), (3, 2), (center.x, center.y))
        }
        TileContent::BatteryPad => {
            // Find anchor and highlight 2x2
            let (anchor_x, anchor_y) = find_anchor(grid, x, y, content);
            let center = SiteGrid::multi_tile_center(anchor_x, anchor_y, StructureSize::TwoByTwo);
            (Some(true), (2, 2), (center.x, center.y))
        }
        TileContent::SecurityPad => {
            // Find anchor and highlight 2x2
            let (anchor_x, anchor_y) = find_anchor(grid, x, y, content);
            let center = SiteGrid::multi_tile_center(anchor_x, anchor_y, StructureSize::TwoByTwo);
            (Some(true), (2, 2), (center.x, center.y))
        }
        TileContent::AmenityWifiRestrooms => {
            // Find anchor and highlight 3x3
            let (anchor_x, anchor_y) = find_anchor(grid, x, y, content);
            let center =
                SiteGrid::multi_tile_center(anchor_x, anchor_y, StructureSize::ThreeByThree);
            (Some(true), (3, 3), (center.x, center.y))
        }
        TileContent::AmenityLoungeSnacks => {
            // Find anchor and highlight 4x4
            let (anchor_x, anchor_y) = find_anchor(grid, x, y, content);
            let center = SiteGrid::multi_tile_center(anchor_x, anchor_y, StructureSize::FourByFour);
            (Some(true), (4, 4), (center.x, center.y))
        }
        TileContent::AmenityRestaurant => {
            // Find anchor and highlight 5x4
            let (anchor_x, anchor_y) = find_anchor(grid, x, y, content);
            let center = SiteGrid::multi_tile_center(anchor_x, anchor_y, StructureSize::FiveByFour);
            (Some(true), (5, 4), (center.x, center.y))
        }
        TileContent::AmenityDriverRestLounge => {
            let (anchor_x, anchor_y) = find_anchor(grid, x, y, content);
            let center =
                SiteGrid::multi_tile_center(anchor_x, anchor_y, StructureSize::ThreeByThree);
            (Some(true), (3, 3), (center.x, center.y))
        }
        TileContent::TransformerOccupied
        | TileContent::SolarOccupied
        | TileContent::BatteryOccupied
        | TileContent::SecurityOccupied
        | TileContent::AmenityOccupied => {
            // This is part of a multi-tile structure, find its anchor
            if let Some(tile) = grid.get_tile(x, y)
                && let Some((anchor_x, anchor_y)) = tile.anchor_pos
            {
                let anchor_content = grid.get_content(anchor_x, anchor_y);
                // Recursively check the anchor
                return determine_sell_target(grid, anchor_x, anchor_y, anchor_content);
            }
            // Fallback
            (None, (1, 1), (0.0, 0.0))
        }
        TileContent::ChargerPad | TileContent::Lot => {
            // These are other tile types that shouldn't normally be encountered
            // or have specific handling elsewhere
            (None, (1, 1), (0.0, 0.0))
        }
        // Gas station locked infrastructure - cannot sell
        TileContent::StoreWall
        | TileContent::StoreEntrance
        | TileContent::Storefront
        | TileContent::PumpIsland
        | TileContent::Canopy
        | TileContent::FuelCap
        | TileContent::DumpsterPad
        | TileContent::DumpsterOccupied
        | TileContent::CanopyShadow
        | TileContent::CanopyColumn
        | TileContent::GasStationSign
        | TileContent::Bollard
        | TileContent::WheelStop
        | TileContent::StreetTree
        | TileContent::LightPole
        | TileContent::AsphaltWorn
        | TileContent::AsphaltSkid
        // Site-specific locked infrastructure - cannot sell
        | TileContent::GarageFloor
        | TileContent::GaragePillar
        | TileContent::MallFacade
        | TileContent::ReservedSpot
        | TileContent::OfficeBackdrop
        | TileContent::LoadingZone
        | TileContent::Concrete
        | TileContent::Planter => {
            // Protected infrastructure - cannot sell
            let world_pos = SiteGrid::grid_to_world(x, y);
            (Some(false), (1, 1), (world_pos.x, world_pos.y))
        }
    }
}

/// Find the anchor position for a tile (handles both anchor and occupied tiles)
fn find_anchor(grid: &SiteGrid, x: i32, y: i32, content: TileContent) -> (i32, i32) {
    if content.is_anchor() {
        (x, y)
    } else if let Some(tile) = grid.get_tile(x, y) {
        tile.anchor_pos.unwrap_or((x, y))
    } else {
        (x, y)
    }
}

/// Keyboard shortcuts for build mode
pub fn build_keyboard_shortcuts(
    mut build_state: ResMut<BuildState>,
    keyboard: Res<ButtonInput<KeyCode>>,
) {
    // Number keys for tool selection
    // 1 = L2, 2 = DCFC 50kW, 3 = DCFC 150kW, 4 = DCFC 350kW
    // 5 = Xfmr 500kVA, 6 = Xfmr 1000kVA, 7 = Xfmr 2500kVA
    // 8 = Solar, 9 = Battery, 0 = Sell
    if keyboard.just_pressed(KeyCode::Digit1) {
        build_state.selected_tool = BuildTool::ChargerL2;
    }
    if keyboard.just_pressed(KeyCode::Digit2) {
        build_state.selected_tool = BuildTool::ChargerDCFC50;
    }
    if keyboard.just_pressed(KeyCode::Digit3) {
        build_state.selected_tool = BuildTool::ChargerDCFC150;
    }
    if keyboard.just_pressed(KeyCode::Digit4) {
        build_state.selected_tool = BuildTool::ChargerDCFC350;
    }
    if keyboard.just_pressed(KeyCode::Digit5) {
        build_state.selected_tool = BuildTool::Transformer500kVA;
    }
    if keyboard.just_pressed(KeyCode::Digit6) {
        build_state.selected_tool = BuildTool::Transformer1000kVA;
    }
    if keyboard.just_pressed(KeyCode::Digit7) {
        build_state.selected_tool = BuildTool::Transformer2500kVA;
    }
    if keyboard.just_pressed(KeyCode::Digit8) {
        build_state.selected_tool = BuildTool::SolarCanopy;
    }
    if keyboard.just_pressed(KeyCode::Digit9) {
        build_state.selected_tool = BuildTool::BatteryStorage;
    }
    if keyboard.just_pressed(KeyCode::Digit0) {
        build_state.selected_tool = BuildTool::Sell;
    }
    if keyboard.just_pressed(KeyCode::KeyX) || keyboard.just_pressed(KeyCode::Backspace) {
        build_state.selected_tool = BuildTool::Sell;
    }
    if keyboard.just_pressed(KeyCode::Escape) {
        build_state.selected_tool = BuildTool::Select;
    }
}
