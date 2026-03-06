//! Fleet contract systems -- spawning fleet vehicles, tracking SLA breaches,
//! and applying fleet visual markers.

use bevy::prelude::*;
use bevy_northstar::prelude::*;
use rand::Rng;

use crate::components::VehicleFootprint;
use crate::components::charger::Charger;
use crate::components::driver::{
    Driver, DriverMood, DriverState, MovementPhase, PatienceLevel, VehicleMovement,
};
use crate::events::DriverArrivedEvent;
use crate::helpers::ui_builders::colors;
use crate::resources::fleet::{
    FleetContractTerminatedEvent, FleetDebugMode, FleetDebugOverlay, FleetHaloLayer,
    FleetHaloTexture, FleetOverlay, FleetVehicle,
};
use crate::resources::{BuildState, FleetContractManager, GameClock, GameState, SiteGrid};

/// Spawn fleet vehicles based on active contracts and current game time.
///
/// Runs in `GameSystemSet::DriverSpawn`. Spawns at most one fleet vehicle per
/// frame (same constraint as the regular driver spawn system).
pub fn fleet_spawn_system(
    mut commands: Commands,
    build_state: Res<BuildState>,
    multi_site: Res<crate::resources::MultiSiteManager>,
    game_clock: Res<GameClock>,
    mut fleet_mgr: ResMut<FleetContractManager>,
    existing_drivers: Query<(&Driver, &crate::components::BelongsToSite, &AgentPos)>,
    chargers: Query<(Entity, &Charger, &crate::components::BelongsToSite), Without<Driver>>,
    mut arrived_events: MessageWriter<DriverArrivedEvent>,
    blocking_map: Res<BlockingMap>,
    charger_index: Res<crate::hooks::ChargerIndex>,
) {
    if !build_state.is_open || game_clock.is_paused() || game_clock.day_ending {
        return;
    }

    let Some(viewed_id) = multi_site.viewed_site_id else {
        return;
    };
    let Some(site_state) = multi_site.owned_sites.get(&viewed_id) else {
        return;
    };
    if site_state.root_entity.is_none() {
        return;
    }

    let hour = game_clock.hour();
    if fleet_mgr.active.is_empty() {
        return;
    }

    // Find first contract + window that needs a spawn
    let mut spawn_request: Option<(usize, usize, crate::components::driver::VehicleType, f32)> =
        None;

    for (ci, contract) in fleet_mgr.active.iter().enumerate() {
        if contract.terminated || contract.def.site_archetype != site_state.archetype {
            continue;
        }
        for (wi, window) in contract.def.time_windows.iter().enumerate() {
            let in_window = if window.start_hour <= window.end_hour {
                hour >= window.start_hour && hour < window.end_hour
            } else {
                hour >= window.start_hour || hour < window.end_hour
            };
            if !in_window {
                continue;
            }
            let already_spawned = contract.window_spawned.get(wi).copied().unwrap_or(0);
            if already_spawned >= window.vehicle_count {
                continue;
            }

            let window_duration_hours = if window.end_hour > window.start_hour {
                window.end_hour - window.start_hour
            } else {
                24 - window.start_hour + window.end_hour
            };
            let window_duration_secs = window_duration_hours as f32 * 3600.0;
            let secs_into_window = if hour >= window.start_hour {
                (hour - window.start_hour) as f32 * 3600.0 + (game_clock.game_time % 3600.0)
            } else {
                (24 - window.start_hour + hour) as f32 * 3600.0 + (game_clock.game_time % 3600.0)
            };
            let progress = (secs_into_window / window_duration_secs).clamp(0.0, 1.0);
            let expected = ((window.vehicle_count as f32) * progress).floor() as u32;

            if already_spawned < expected {
                let mut rng = rand::rng();
                let vt = if contract.def.vehicle_types.is_empty() {
                    crate::components::driver::VehicleType::Bus
                } else {
                    contract.def.vehicle_types
                        [rng.random_range(0..contract.def.vehicle_types.len())]
                };
                let charge = crate::resources::demand::charge_needed_for_vehicle(&mut rng, vt);
                spawn_request = Some((ci, wi, vt, charge));
                break;
            }
        }
        if spawn_request.is_some() {
            break;
        }
    }

    let Some((contract_idx, window_idx, vehicle_type, charge_kwh)) = spawn_request else {
        return;
    };

    // Gate on site capacity
    let charger_bays = site_state.grid.get_charger_bays();
    if charger_bays.is_empty() {
        return;
    }
    let charger_bay_positions: Vec<(i32, i32)> =
        charger_bays.iter().map(|&(x, y, _)| (x, y)).collect();
    let current_driver_count = existing_drivers
        .iter()
        .filter(|(_, b, _)| b.site_id == viewed_id)
        .count();
    if current_driver_count >= site_state.max_vehicles {
        return;
    }

    let entry_pos = site_state.grid.entry_pos;
    let entry_uvec = UVec3::new(entry_pos.0 as u32, entry_pos.1 as u32, 0);
    if blocking_map.0.contains_key(&entry_uvec) {
        return;
    }

    let occupied_bays: Vec<(i32, i32)> = existing_drivers
        .iter()
        .filter(|(_, b, _)| b.site_id == viewed_id)
        .filter(|(d, _, _)| {
            !matches!(
                d.state,
                DriverState::Leaving | DriverState::LeftAngry | DriverState::Complete
            )
        })
        .filter_map(|(d, _, _)| d.assigned_bay)
        .collect();

    let compatible_bays: Vec<_> = charger_bays
        .iter()
        .filter(|(_, _, pad_type)| vehicle_type.is_compatible_with(pad_type.to_charger_type()))
        .collect();
    if compatible_bays.is_empty() {
        return;
    }

    let available_compatible: Vec<_> = compatible_bays
        .iter()
        .filter(|(x, y, _)| !occupied_bays.contains(&(*x, *y)))
        .collect();

    let root_entity = match site_state.root_entity {
        Some(e) => e,
        None => return,
    };
    let exit_pos = site_state.grid.exit_pos;
    let world_pos = SiteGrid::grid_to_world(entry_pos.0, entry_pos.1);
    let pos = Vec3::new(world_pos.x, world_pos.y, 1.0);

    let contract = &fleet_mgr.active[contract_idx];
    let contract_id = contract.def.id.clone();
    let company_color = contract.company_color();
    let company_name = contract.def.company_name.clone();
    let fleet_id_counter = contract.vehicles_spawned_today + 1;
    let driver_id = format!("fleet_{}_{}", contract_id, fleet_id_counter);

    let mut rng = rand::rng();
    let evcc_id = crate::resources::demand::generate_evcc_mac(&mut rng);
    let vehicle_name =
        crate::resources::demand::random_vehicle_name_for_type(&mut rng, vehicle_type);

    let fleet_marker = FleetVehicle {
        contract_id: contract_id.clone(),
        company_color,
    };

    if available_compatible.is_empty() {
        // No bay -- try waiting tile, else drive-through angry
        let occupied_waiting: Vec<(i32, i32)> = existing_drivers
            .iter()
            .filter(|(_, b, _)| b.site_id == viewed_id)
            .filter(|(d, _, _)| {
                !matches!(
                    d.state,
                    DriverState::Leaving | DriverState::LeftAngry | DriverState::Complete
                )
            })
            .filter_map(|(d, _, _)| d.waiting_tile)
            .collect();

        if let Some((wx, wy)) = site_state.grid.find_waiting_tile(
            &occupied_waiting,
            &occupied_bays,
            &charger_bay_positions,
        ) {
            let driver = Driver {
                id: driver_id.clone(),
                evcc_id,
                vehicle_name,
                vehicle_type,
                patience_level: PatienceLevel::High,
                patience: PatienceLevel::High.initial_patience(),
                charge_needed_kwh: charge_kwh,
                waiting_tile: Some((wx, wy)),
                state: DriverState::Arriving,
                ..default()
            };
            let movement = VehicleMovement {
                speed: 100.0,
                phase: MovementPhase::Arriving,
                waypoints: vec![world_pos],
                ..default()
            };
            let footprint = VehicleFootprint {
                length_tiles: vehicle_type.footprint_length_tiles(),
            };
            let agent_pos = AgentPos(UVec3::new(entry_pos.0 as u32, entry_pos.1 as u32, 0));
            let pathfind = Pathfind::new_2d(wx as u32, wy as u32);

            let mut entity = Entity::PLACEHOLDER;
            commands.entity(root_entity).with_children(|parent| {
                entity = parent
                    .spawn((
                        driver,
                        movement,
                        footprint,
                        fleet_marker,
                        agent_pos,
                        pathfind,
                        Blocking,
                        Transform::from_translation(pos),
                        GlobalTransform::default(),
                        Visibility::default(),
                        crate::components::BelongsToSite::new(viewed_id),
                    ))
                    .id();
            });

            arrived_events.write(DriverArrivedEvent {
                driver_entity: entity,
                driver_id: driver_id.clone(),
                target_charger_id: None,
            });
        } else {
            let driver = Driver {
                id: driver_id.clone(),
                evcc_id,
                vehicle_name,
                vehicle_type,
                patience_level: PatienceLevel::High,
                patience: 0.0,
                charge_needed_kwh: charge_kwh,
                state: DriverState::LeftAngry,
                mood: DriverMood::Angry,
                ..default()
            };
            let movement = VehicleMovement {
                speed: 100.0,
                phase: MovementPhase::DepartingAngry,
                waypoints: vec![world_pos],
                ..default()
            };
            let footprint = VehicleFootprint {
                length_tiles: vehicle_type.footprint_length_tiles(),
            };
            let agent_pos = AgentPos(UVec3::new(entry_pos.0 as u32, entry_pos.1 as u32, 0));
            let pathfind = Pathfind::new_2d(exit_pos.0 as u32, exit_pos.1 as u32);

            commands.entity(root_entity).with_children(|parent| {
                parent.spawn((
                    driver,
                    movement,
                    footprint,
                    fleet_marker,
                    agent_pos,
                    pathfind,
                    Transform::from_translation(pos),
                    GlobalTransform::default(),
                    Visibility::default(),
                    crate::components::BelongsToSite::new(viewed_id),
                ));
            });
        }
    } else {
        // Pick best available bay
        let selected_bay = {
            let mut best: Option<(
                &(i32, i32, crate::resources::site_grid::ChargerPadType),
                f32,
            )> = None;
            for bay in &available_compatible {
                let score = site_state
                    .grid
                    .get_tile(bay.0, bay.1)
                    .and_then(|t| t.linked_charger_pad)
                    .and_then(|(px, py)| charger_index.get_by_position(px, py))
                    .and_then(|e| chargers.get(e).ok())
                    .map(|(_, c, _)| c.rated_power_kw)
                    .unwrap_or(0.0)
                    + rand::random::<f32>() * 0.01;
                if best.is_none_or(|(_, s)| score > s) {
                    best = Some((bay, score));
                }
            }
            best.map(|(b, _)| b)
        };

        let Some(&(bay_x, bay_y, _)) = selected_bay else {
            return;
        };

        let driver = Driver {
            id: driver_id.clone(),
            evcc_id,
            vehicle_name,
            vehicle_type,
            patience_level: PatienceLevel::High,
            patience: PatienceLevel::High.initial_patience(),
            charge_needed_kwh: charge_kwh,
            assigned_bay: Some((bay_x, bay_y)),
            state: DriverState::Arriving,
            ..default()
        };
        let movement = VehicleMovement {
            speed: 100.0,
            phase: MovementPhase::Arriving,
            waypoints: vec![world_pos],
            ..default()
        };
        let footprint = VehicleFootprint {
            length_tiles: vehicle_type.footprint_length_tiles(),
        };
        let agent_pos = AgentPos(UVec3::new(entry_pos.0 as u32, entry_pos.1 as u32, 0));
        let pathfind = Pathfind::new_2d(bay_x as u32, bay_y as u32);

        let mut entity = Entity::PLACEHOLDER;
        commands.entity(root_entity).with_children(|parent| {
            entity = parent
                .spawn((
                    driver,
                    movement,
                    footprint,
                    fleet_marker,
                    agent_pos,
                    pathfind,
                    Blocking,
                    Transform::from_translation(pos),
                    GlobalTransform::default(),
                    Visibility::default(),
                    crate::components::BelongsToSite::new(viewed_id),
                ))
                .id();
        });

        arrived_events.write(DriverArrivedEvent {
            driver_entity: entity,
            driver_id: driver_id.clone(),
            target_charger_id: None,
        });

        info!(
            "[Fleet] {} ({:?}) from '{}' heading to bay ({}, {})",
            driver_id, vehicle_type, company_name, bay_x, bay_y
        );
    }

    let contract = &mut fleet_mgr.active[contract_idx];
    contract.vehicles_spawned_today += 1;
    if let Some(count) = contract.window_spawned.get_mut(window_idx) {
        *count += 1;
    }
}

/// Track fleet vehicle outcomes and apply SLA penalties.
pub fn fleet_sla_system(
    fleet_drivers: Query<(&Driver, &FleetVehicle), Changed<Driver>>,
    mut fleet_mgr: ResMut<FleetContractManager>,
    mut game_state: ResMut<GameState>,
    mut terminated_events: MessageWriter<FleetContractTerminatedEvent>,
) {
    for (driver, fleet_vehicle) in &fleet_drivers {
        match driver.state {
            DriverState::LeftAngry => {
                if let Some(contract) = fleet_mgr
                    .active
                    .iter_mut()
                    .find(|c| c.def.id == fleet_vehicle.contract_id && !c.terminated)
                {
                    let was_terminated = contract.record_breach();
                    game_state.add_fleet_penalty(
                        contract.def.penalty_per_miss,
                        &contract.def.company_name,
                    );
                    game_state.record_reputation(crate::resources::ReputationSource::FleetBreach(
                        -contract.def.reputation_penalty_per_miss,
                    ));

                    warn!(
                        "[Fleet] Breach! {} missed from '{}' ({}/{})",
                        driver.id,
                        contract.def.company_name,
                        contract.breaches_total,
                        contract.def.max_breaches_before_termination
                    );

                    if was_terminated {
                        game_state.add_fleet_penalty(
                            contract.def.termination_fine,
                            &contract.def.company_name,
                        );
                        error!(
                            "[Fleet] Contract '{}' TERMINATED after {} breaches (fine: ${})",
                            contract.def.company_name,
                            contract.breaches_total,
                            contract.def.termination_fine,
                        );
                        terminated_events.write(FleetContractTerminatedEvent {
                            contract_id: contract.def.id.clone(),
                            company_name: contract.def.company_name.clone(),
                            breaches_total: contract.breaches_total,
                            termination_fine: contract.def.termination_fine,
                        });
                    }
                }
            }
            DriverState::Complete | DriverState::Leaving => {
                if let Some(contract) = fleet_mgr
                    .active
                    .iter_mut()
                    .find(|c| c.def.id == fleet_vehicle.contract_id)
                {
                    contract.record_charged();
                }
            }
            _ => {}
        }
    }
}

/// Apply fleet visual distinction (company color tint + premium halo) to newly spawned fleet vehicles.
///
/// Triggers on `Added<VehicleSprite>` so the child sprite entity already exists
/// when we apply the tint. The previous `Added<Driver>` approach raced with
/// `spawn_vehicle_sprites` in the same system set, causing the tint and overlay
/// to be silently skipped.
const HALO_BASE_Z: f32 = 1.0;
const HALO_LABEL_SHADOW_Z: f32 = 17.5;
const HALO_LABEL_Z: f32 = 17.7;
const HALO_TEXTURE_W: u32 = 256;
const HALO_TEXTURE_H: u32 = 128;

#[derive(Clone, Copy)]
struct VehicleHaloMetrics {
    halo_size: Vec2,
    font_size: f32,
}

pub fn ensure_fleet_halo_texture(
    mut halo_texture: ResMut<FleetHaloTexture>,
    mut images: ResMut<Assets<Image>>,
) {
    if halo_texture.image.is_some() {
        return;
    }

    halo_texture.image = Some(images.add(build_fleet_halo_texture()));
}

pub fn fleet_visual_system(
    mut commands: Commands,
    mut new_sprites: Query<
        (&crate::systems::sprite::VehicleSprite, &mut Sprite),
        Added<crate::systems::sprite::VehicleSprite>,
    >,
    drivers: Query<&Driver>,
    fleet_vehicles: Query<&FleetVehicle>,
    fleet_mgr: Res<FleetContractManager>,
    children_query: Query<&Children>,
    existing_overlays: Query<&FleetOverlay>,
    halo_texture: Res<FleetHaloTexture>,
) {
    for (vehicle_sprite, mut sprite) in &mut new_sprites {
        let driver_entity = vehicle_sprite.driver_entity;
        let Ok(driver) = drivers.get(driver_entity) else {
            continue;
        };
        let Ok(fleet_vehicle) = fleet_vehicles.get(driver_entity) else {
            continue;
        };

        sprite.color = fleet_vehicle.company_color;

        let has_overlay = children_query
            .get(driver_entity)
            .map(|children| children.iter().any(|c| existing_overlays.get(c).is_ok()))
            .unwrap_or(false);

        if !has_overlay {
            let name = fleet_mgr
                .active
                .iter()
                .find(|c| c.def.id == fleet_vehicle.contract_id)
                .map(|c| c.def.company_name.chars().take(8).collect::<String>())
                .unwrap_or_else(|| fleet_vehicle.contract_id.chars().take(8).collect());

            spawn_vehicle_overlay::<FleetOverlay>(
                &mut commands,
                driver_entity,
                driver.vehicle_type,
                &name,
                fleet_vehicle.company_color,
                fleet_vehicle.company_color,
                &halo_texture,
            );
        }
    }
}

/// Toggle the fleet debug overlay with F5.
///
/// Adds a blue premium halo + label for non-fleet vehicles so you can compare
/// them against the always-on fleet overlays.
pub fn toggle_fleet_debug(
    keyboard: Res<ButtonInput<KeyCode>>,
    mut commands: Commands,
    mut debug_mode: ResMut<FleetDebugMode>,
    non_fleet_drivers: Query<(Entity, &Driver), Without<FleetVehicle>>,
    fleet_vehicles: Query<Entity, With<FleetVehicle>>,
    children_query: Query<&Children>,
    existing_labels: Query<Entity, With<FleetDebugOverlay>>,
    halo_texture: Res<FleetHaloTexture>,
) {
    if !keyboard.just_pressed(KeyCode::F5) {
        return;
    }

    debug_mode.active = !debug_mode.active;

    let fleet_count = fleet_vehicles.iter().count();
    let customer_count = non_fleet_drivers.iter().count();
    info!(
        "Fleet debug overlay: {} ({} fleet / {} customers on screen)",
        if debug_mode.active { "ON" } else { "OFF" },
        fleet_count,
        customer_count,
    );

    if debug_mode.active {
        for (driver_entity, driver) in &non_fleet_drivers {
            let already_has = children_query
                .get(driver_entity)
                .map(|children| children.iter().any(|c| existing_labels.get(c).is_ok()))
                .unwrap_or(false);

            if !already_has {
                spawn_customer_debug_overlay(
                    &mut commands,
                    driver_entity,
                    driver.vehicle_type,
                    &halo_texture,
                );
            }
        }
    } else {
        for label_entity in &existing_labels {
            commands.entity(label_entity).despawn();
        }
    }
}

/// Ensure newly-spawned non-fleet drivers get the F5 overlay when the debug mode is active.
pub fn fleet_debug_label_sync(
    mut commands: Commands,
    debug_mode: Res<FleetDebugMode>,
    new_drivers: Query<(Entity, &Driver), (Added<Driver>, Without<FleetVehicle>)>,
    children_query: Query<&Children>,
    existing_labels: Query<Entity, With<FleetDebugOverlay>>,
    halo_texture: Res<FleetHaloTexture>,
) {
    if !debug_mode.active {
        return;
    }

    for (driver_entity, driver) in &new_drivers {
        let already_has = children_query
            .get(driver_entity)
            .map(|children| children.iter().any(|c| existing_labels.get(c).is_ok()))
            .unwrap_or(false);
        if already_has {
            continue;
        }

        spawn_customer_debug_overlay(
            &mut commands,
            driver_entity,
            driver.vehicle_type,
            &halo_texture,
        );
    }
}

fn spawn_customer_debug_overlay(
    commands: &mut Commands,
    driver_entity: Entity,
    vehicle_type: crate::components::driver::VehicleType,
    halo_texture: &FleetHaloTexture,
) {
    spawn_vehicle_overlay::<FleetDebugOverlay>(
        commands,
        driver_entity,
        vehicle_type,
        "DRIVER",
        crate::systems::sprite::colors::SELECTION_HIGHLIGHT,
        crate::systems::sprite::colors::SELECTION_HIGHLIGHT,
        halo_texture,
    );
}

fn spawn_vehicle_overlay<T: Component + Default>(
    commands: &mut Commands,
    driver_entity: Entity,
    vehicle_type: crate::components::driver::VehicleType,
    label: &str,
    glow_color: Color,
    text_color: Color,
    halo_texture: &FleetHaloTexture,
) {
    let metrics = halo_metrics(vehicle_type);
    let Some(glow_image) = halo_texture.image.clone() else {
        return;
    };
    let halo_scale = scale_generated_halo(metrics.halo_size);
    let shadow_color = Color::srgba(0.02, 0.04, 0.06, 0.55);
    let label_rotation = Quat::from_rotation_z(std::f32::consts::FRAC_PI_2);

    commands.entity(driver_entity).with_children(|parent| {
        let mut halo = Sprite::from_image(glow_image);
        halo.color = glow_color.with_alpha(0.62);
        parent.spawn((
            halo,
            Transform::from_xyz(0.0, 0.0, HALO_BASE_Z).with_scale(halo_scale),
            FleetHaloLayer {
                base_scale: halo_scale,
                base_alpha: 0.62,
                pulse_scale: 0.018,
                pulse_alpha: 0.07,
                rotation_speed: 0.0,
                phase_offset: halo_phase(vehicle_type),
            },
            T::default(),
        ));

        parent.spawn((
            Text2d::new(label),
            TextFont {
                font_size: metrics.font_size,
                ..default()
            },
            TextColor(shadow_color),
            Transform::from_xyz(1.5, -1.5, HALO_LABEL_SHADOW_Z).with_rotation(label_rotation),
            T::default(),
        ));

        parent.spawn((
            Text2d::new(label),
            TextFont {
                font_size: metrics.font_size,
                ..default()
            },
            TextColor(text_color),
            Transform::from_xyz(0.0, 0.0, HALO_LABEL_Z).with_rotation(label_rotation),
            T::default(),
        ));
    });
}

pub fn animate_vehicle_halos(
    time: Res<Time>,
    mut halo_layers: Query<(&FleetHaloLayer, &mut Transform, &mut Sprite)>,
) {
    let elapsed = time.elapsed_secs();

    for (layer, mut transform, mut sprite) in &mut halo_layers {
        let pulse = (elapsed * 1.8 + layer.phase_offset).sin();
        let scale = 1.0 + pulse * layer.pulse_scale;
        transform.scale = layer.base_scale * scale;
        transform.rotation = if layer.rotation_speed.abs() > f32::EPSILON {
            Quat::from_rotation_z(elapsed * layer.rotation_speed)
        } else {
            Quat::IDENTITY
        };
        sprite.color = sprite
            .color
            .with_alpha((layer.base_alpha + pulse * layer.pulse_alpha).clamp(0.0, 1.0));
    }
}

fn halo_metrics(vehicle_type: crate::components::driver::VehicleType) -> VehicleHaloMetrics {
    let vehicle_width = crate::resources::sprite_metadata::vehicle_world_size(vehicle_type).width;
    let halo_width = (vehicle_width * 1.55).clamp(42.0, 96.0);
    let halo_height = (vehicle_width * 0.78).clamp(24.0, 52.0);
    let font_size = (vehicle_width * 0.28).clamp(10.0, 16.0);

    VehicleHaloMetrics {
        halo_size: Vec2::new(halo_width, halo_height),
        font_size,
    }
}

fn scale_generated_halo(target_size: Vec2) -> Vec3 {
    Vec3::new(
        target_size.x / HALO_TEXTURE_W as f32,
        target_size.y / HALO_TEXTURE_H as f32,
        1.0,
    )
}

fn build_fleet_halo_texture() -> Image {
    let width = HALO_TEXTURE_W as usize;
    let height = HALO_TEXTURE_H as usize;
    let mut rgba = vec![0u8; width * height * 4];

    for y in 0..height {
        for x in 0..width {
            let nx = ((x as f32 + 0.5) / width as f32) * 2.0 - 1.0;
            let ny = ((y as f32 + 0.5) / height as f32) * 2.0 - 1.0;
            let elliptical_y = ny * 1.65;
            let radius = (nx * nx + elliptical_y * elliptical_y).sqrt();
            let inner_glow = (1.0 - smoothstep(0.18, 0.92, radius)) * 0.22;
            let rim_glow = ring_band(radius, 0.68, 0.24, 0.12) * 0.95;
            let alpha = (inner_glow + rim_glow).clamp(0.0, 1.0);

            let offset = (y * width + x) * 4;
            rgba[offset] = 255;
            rgba[offset + 1] = 255;
            rgba[offset + 2] = 255;
            rgba[offset + 3] = (alpha * 255.0).round() as u8;
        }
    }

    let mut image = Image::new(
        bevy::render::render_resource::Extent3d {
            width: HALO_TEXTURE_W,
            height: HALO_TEXTURE_H,
            depth_or_array_layers: 1,
        },
        bevy::render::render_resource::TextureDimension::D2,
        rgba,
        bevy::render::render_resource::TextureFormat::Rgba8UnormSrgb,
        bevy::asset::RenderAssetUsages::RENDER_WORLD | bevy::asset::RenderAssetUsages::MAIN_WORLD,
    );
    image.sampler = bevy::image::ImageSampler::Descriptor(bevy::image::ImageSamplerDescriptor {
        mag_filter: bevy::image::ImageFilterMode::Linear,
        min_filter: bevy::image::ImageFilterMode::Linear,
        ..default()
    });
    image
}

fn ring_band(radius: f32, center: f32, half_width: f32, feather: f32) -> f32 {
    let inner = smoothstep(
        center - half_width - feather,
        center - half_width + feather,
        radius,
    );
    let outer = 1.0
        - smoothstep(
            center + half_width - feather,
            center + half_width + feather,
            radius,
        );
    (inner * outer).clamp(0.0, 1.0)
}

fn smoothstep(edge0: f32, edge1: f32, value: f32) -> f32 {
    let t = ((value - edge0) / (edge1 - edge0)).clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

fn halo_phase(vehicle_type: crate::components::driver::VehicleType) -> f32 {
    use crate::components::driver::VehicleType;

    match vehicle_type {
        VehicleType::Compact => 0.15,
        VehicleType::Sedan => 0.55,
        VehicleType::Suv => 0.95,
        VehicleType::Crossover => 1.35,
        VehicleType::Pickup => 1.75,
        VehicleType::Bus => 2.15,
        VehicleType::Semi => 2.55,
        VehicleType::Tractor => 2.95,
        VehicleType::Firetruck => 3.35,
        VehicleType::Scooter => 3.75,
        VehicleType::Motorcycle => 4.15,
    }
}

/// Handle fleet contract offer banner interactions (accept/decline).
pub fn fleet_offer_interaction_system(
    mut commands: Commands,
    mut fleet_mgr: ResMut<FleetContractManager>,
    game_clock: Res<GameClock>,
    accept_query: Query<
        (
            &Interaction,
            &crate::resources::fleet::FleetOfferAcceptButton,
        ),
        Changed<Interaction>,
    >,
    decline_query: Query<
        &Interaction,
        (
            Changed<Interaction>,
            With<crate::resources::fleet::FleetOfferDeclineButton>,
        ),
    >,
    banner_query: Query<Entity, With<crate::resources::fleet::FleetOfferBanner>>,
) {
    let mut dismiss = false;

    for (interaction, accept_btn) in &accept_query {
        if *interaction == Interaction::Pressed {
            fleet_mgr.accept_contract(&accept_btn.contract_id, game_clock.day);
            info!("[Fleet] Contract '{}' accepted", accept_btn.contract_id);
            dismiss = true;
        }
    }

    for interaction in &decline_query {
        if *interaction == Interaction::Pressed {
            info!("[Fleet] Contract offer declined");
            dismiss = true;
        }
    }

    if dismiss {
        fleet_mgr.offer_shown_today = true;
        for entity in &banner_query {
            commands.entity(entity).try_despawn();
        }
    }
}

/// Minimum reputation required for fleet companies to approach the player.
const FLEET_OFFER_MIN_REPUTATION: i32 = 80;

/// Spawn the fleet contract offer modal if there are available contracts
/// and the player's reputation is high enough (80+, "Premium partners" tier).
pub fn spawn_fleet_offer_banner(
    mut commands: Commands,
    fleet_mgr: Res<FleetContractManager>,
    game_state: Res<GameState>,
    multi_site: Res<crate::resources::MultiSiteManager>,
    existing_banners: Query<&crate::resources::fleet::FleetOfferBanner>,
) {
    if fleet_mgr.offer_shown_today || !fleet_mgr.has_offers() || !existing_banners.is_empty() {
        return;
    }

    if game_state.reputation < FLEET_OFFER_MIN_REPUTATION {
        return;
    }

    let Some(site) = multi_site.active_site() else {
        return;
    };

    let Some(offer) = fleet_mgr
        .available
        .iter()
        .find(|d| d.site_archetype == site.archetype)
    else {
        return;
    };

    let contract_id = offer.id.clone();
    let company_name = offer.company_name.clone();
    let vehicles = offer.vehicles_per_day;
    let retainer = offer.daily_payment;
    let penalty = offer.penalty_per_miss;
    let price_kwh = offer.contracted_price_per_kwh;
    let max_breaches = offer.max_breaches_before_termination;
    let termination_fine = offer.termination_fine;
    let rep_penalty = offer.reputation_penalty_per_miss;

    let window_summary: String = offer
        .time_windows
        .iter()
        .map(|w| {
            let fmt_hour = |h: u32| -> String {
                match h {
                    0 | 24 => "12AM".to_string(),
                    1..=11 => format!("{h}AM"),
                    12 => "12PM".to_string(),
                    13..=23 => format!("{}PM", h - 12),
                    _ => format!("{h}:00"),
                }
            };
            format!(
                "{} vehicles {}-{}",
                w.vehicle_count,
                fmt_hour(w.start_hour),
                fmt_hour(w.end_hour)
            )
        })
        .collect::<Vec<_>>()
        .join("  |  ");

    // Full-screen dim overlay with centered modal (matches day-end style)
    commands
        .spawn((
            crate::resources::fleet::FleetOfferBanner,
            Node {
                width: Val::Percent(100.0),
                height: Val::Percent(100.0),
                position_type: PositionType::Absolute,
                justify_content: JustifyContent::Center,
                align_items: AlignItems::Center,
                ..default()
            },
            BackgroundColor(Color::srgba(0.0, 0.0, 0.0, 0.6)),
            GlobalZIndex(900),
        ))
        .with_children(|overlay| {
            overlay
                .spawn((
                    Node {
                        width: Val::Px(520.0),
                        flex_direction: FlexDirection::Column,
                        padding: UiRect::all(Val::Px(28.0)),
                        row_gap: Val::Px(16.0),
                        border: UiRect::all(Val::Px(2.0)),
                        ..default()
                    },
                    BackgroundColor(Color::srgb(0.12, 0.14, 0.18)),
                    BorderColor::all(colors::MODAL_BORDER_GLOW),
                    BorderRadius::all(Val::Px(12.0)),
                ))
                .with_children(|modal| {
                    // Title
                    modal.spawn((
                        Text::new("FLEET CONTRACT OFFER"),
                        TextFont {
                            font_size: 22.0,
                            ..default()
                        },
                        TextColor(Color::srgb(0.4, 0.8, 1.0)),
                    ));

                    // Company name
                    modal.spawn((
                        Text::new(company_name.clone()),
                        TextFont {
                            font_size: 18.0,
                            ..default()
                        },
                        TextColor(Color::WHITE),
                    ));

                    // Divider
                    modal.spawn((
                        Node {
                            width: Val::Percent(100.0),
                            height: Val::Px(1.0),
                            ..default()
                        },
                        BackgroundColor(Color::srgb(0.3, 0.4, 0.5)),
                    ));

                    // Terms grid
                    spawn_offer_row(
                        modal,
                        "Daily Retainer",
                        &format!("${retainer:.0}"),
                        Color::srgb(0.4, 0.9, 0.4),
                    );
                    spawn_offer_row(modal, "Vehicles/Day", &format!("{vehicles}"), Color::WHITE);
                    spawn_offer_row(
                        modal,
                        "Rate",
                        &format!("${price_kwh:.2}/kWh"),
                        Color::srgb(0.8, 0.8, 0.8),
                    );
                    modal
                        .spawn(Node {
                            width: Val::Percent(100.0),
                            flex_direction: FlexDirection::Column,
                            row_gap: Val::Px(4.0),
                            ..default()
                        })
                        .with_children(|col| {
                            col.spawn((
                                Text::new("Schedule"),
                                TextFont {
                                    font_size: 15.0,
                                    ..default()
                                },
                                TextColor(Color::srgb(0.6, 0.6, 0.6)),
                            ));
                            col.spawn((
                                Text::new(window_summary.clone()),
                                TextFont {
                                    font_size: 15.0,
                                    ..default()
                                },
                                TextColor(Color::srgb(0.7, 0.7, 0.7)),
                            ));
                        });

                    // Divider
                    modal.spawn((
                        Node {
                            width: Val::Percent(100.0),
                            height: Val::Px(1.0),
                            ..default()
                        },
                        BackgroundColor(Color::srgb(0.3, 0.4, 0.5)),
                    ));

                    // Penalties section
                    spawn_offer_row(
                        modal,
                        "Penalty/Miss",
                        &format!("${penalty:.0} + {rep_penalty} rep"),
                        Color::srgb(0.9, 0.5, 0.3),
                    );
                    spawn_offer_row(
                        modal,
                        "Max Breaches",
                        &format!(
                            "{max_breaches} total, then terminated + ${termination_fine:.0} fine"
                        ),
                        Color::srgb(0.9, 0.5, 0.3),
                    );

                    // Buttons
                    modal
                        .spawn(Node {
                            flex_direction: FlexDirection::Row,
                            column_gap: Val::Px(16.0),
                            justify_content: JustifyContent::Center,
                            margin: UiRect::top(Val::Px(8.0)),
                            ..default()
                        })
                        .with_children(|row| {
                            row.spawn((
                                Button,
                                Node {
                                    padding: UiRect::new(
                                        Val::Px(32.0),
                                        Val::Px(32.0),
                                        Val::Px(12.0),
                                        Val::Px(12.0),
                                    ),
                                    justify_content: JustifyContent::Center,
                                    align_items: AlignItems::Center,
                                    border: UiRect::all(Val::Px(2.0)),
                                    ..default()
                                },
                                BackgroundColor(Color::srgb(0.2, 0.6, 0.3)),
                                BorderColor::all(Color::srgb(0.25, 0.7, 0.35)),
                                BorderRadius::all(Val::Px(6.0)),
                                crate::resources::fleet::FleetOfferAcceptButton {
                                    contract_id: contract_id.clone(),
                                },
                            ))
                            .with_child((
                                Text::new("Accept Contract"),
                                TextFont {
                                    font_size: 16.0,
                                    ..default()
                                },
                                TextColor(Color::WHITE),
                            ));

                            row.spawn((
                                Button,
                                Node {
                                    padding: UiRect::new(
                                        Val::Px(32.0),
                                        Val::Px(32.0),
                                        Val::Px(12.0),
                                        Val::Px(12.0),
                                    ),
                                    justify_content: JustifyContent::Center,
                                    align_items: AlignItems::Center,
                                    border: UiRect::all(Val::Px(1.0)),
                                    ..default()
                                },
                                BackgroundColor(Color::srgb(0.25, 0.25, 0.3)),
                                BorderColor::all(Color::srgb(0.4, 0.4, 0.45)),
                                BorderRadius::all(Val::Px(6.0)),
                                crate::resources::fleet::FleetOfferDeclineButton,
                            ))
                            .with_child((
                                Text::new("Decline"),
                                TextFont {
                                    font_size: 16.0,
                                    ..default()
                                },
                                TextColor(Color::srgb(0.7, 0.7, 0.7)),
                            ));
                        });
                });
        });
}

fn spawn_offer_row(
    parent: &mut bevy::ecs::hierarchy::ChildSpawnerCommands,
    label: &str,
    value: &str,
    value_color: Color,
) {
    parent
        .spawn(Node {
            width: Val::Percent(100.0),
            justify_content: JustifyContent::SpaceBetween,
            ..default()
        })
        .with_children(|row| {
            row.spawn((
                Text::new(label),
                TextFont {
                    font_size: 15.0,
                    ..default()
                },
                TextColor(Color::srgb(0.6, 0.6, 0.6)),
            ));
            row.spawn((
                Text::new(value),
                TextFont {
                    font_size: 15.0,
                    ..default()
                },
                TextColor(value_color),
            ));
        });
}
