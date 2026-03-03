//! Power system

use bevy::prelude::*;

use crate::components::BelongsToSite;
use crate::components::charger::Charger;
use crate::components::driver::VehicleType;
use crate::components::power::Transformer;
use crate::events::{
    OverloadSeverity, TransformerFireEvent, TransformerOverloadWarningEvent,
    TransformerWarningEvent,
};
use crate::resources::{GameClock, GameState, ImageAssets, MultiSiteManager, SiteConfig, SiteGrid};

const FIRE_IGNITION_OVERLOAD_SECONDS: f32 = 30.0;
const FIRE_INCIDENT_FINE: f32 = 5_000.0;
const FIRETRUCK_SPEED: f32 = 300.0;
const FIRE_SUPPRESSION_SECONDS: f32 = 10.0;
const WATER_SPRAY_SPAWN_SECONDS: f32 = 0.1;

#[derive(Component, Debug, Clone, Copy, PartialEq, Eq)]
pub enum FiretruckPhase {
    Responding,
    Spraying,
    Returning,
}

#[derive(Component, Debug, Clone)]
pub struct EmergencyFiretruck {
    pub site_id: crate::resources::SiteId,
    pub transformer_entity: Entity,
    pub target_world: Vec2,
    pub exit_world: Vec2,
    pub phase: FiretruckPhase,
    pub spray_seconds_remaining: f32,
    pub spray_spawn_cooldown: f32,
    pub speed: f32,
    pub waypoints: Vec<Vec2>,
    pub current_waypoint: usize,
}

/// Procedural pixel-fire effect using a Doom-style cellular automaton.
/// One entity per burning transformer; holds the heat buffer and a dynamic
/// `Image` handle that is rewritten every frame.
#[derive(Component, Debug, Clone)]
pub struct PixelFire {
    pub transformer_entity: Entity,
    pub width: u32,
    pub height: u32,
    /// Per-pixel heat value (0 = cold/transparent, 255 = max heat).
    /// Stored row-major, bottom row = fire source.
    pub heat: Vec<u8>,
    pub image_handle: Handle<Image>,
}

/// Classic 37-entry Doom fire palette.
/// Index 0 is fully transparent; higher indices go from
/// dark red -> red -> orange -> yellow -> white-hot.
const FIRE_PALETTE: [[u8; 4]; 37] = [
    [0x07, 0x07, 0x07, 0],   //  0  black (transparent)
    [0x1F, 0x07, 0x07, 160], //  1
    [0x2F, 0x0F, 0x07, 180], //  2
    [0x47, 0x0F, 0x07, 195], //  3
    [0x57, 0x17, 0x07, 205], //  4
    [0x67, 0x1F, 0x07, 215], //  5
    [0x77, 0x1F, 0x07, 220], //  6
    [0x8F, 0x27, 0x07, 225], //  7
    [0x9F, 0x2F, 0x07, 230], //  8
    [0xAF, 0x3F, 0x07, 235], //  9
    [0xBF, 0x47, 0x07, 240], // 10
    [0xC7, 0x47, 0x07, 242], // 11
    [0xDF, 0x4F, 0x07, 244], // 12
    [0xDF, 0x57, 0x07, 246], // 13
    [0xDF, 0x57, 0x07, 248], // 14
    [0xD7, 0x5F, 0x07, 249], // 15
    [0xD7, 0x5F, 0x07, 250], // 16
    [0xD7, 0x67, 0x0F, 250], // 17
    [0xCF, 0x6F, 0x0F, 251], // 18
    [0xCF, 0x77, 0x0F, 251], // 19
    [0xCF, 0x7F, 0x0F, 252], // 20
    [0xCF, 0x87, 0x17, 252], // 21
    [0xC7, 0x87, 0x17, 253], // 22
    [0xC7, 0x8F, 0x17, 253], // 23
    [0xC7, 0x97, 0x1F, 253], // 24
    [0xBF, 0x9F, 0x1F, 254], // 25
    [0xBF, 0x9F, 0x1F, 254], // 26
    [0xBF, 0xA7, 0x27, 254], // 27
    [0xBF, 0xA7, 0x27, 254], // 28
    [0xBF, 0xAF, 0x2F, 255], // 29
    [0xB7, 0xAF, 0x2F, 255], // 30
    [0xB7, 0xB7, 0x2F, 255], // 31
    [0xB7, 0xB7, 0x37, 255], // 32
    [0xCF, 0xCF, 0x6F, 255], // 33
    [0xDF, 0xDF, 0x9F, 255], // 34
    [0xEF, 0xEF, 0xC7, 255], // 35
    [0xFF, 0xFF, 0xFF, 255], // 36  white-hot
];

#[derive(Component, Debug, Clone, Copy)]
pub struct TransformerWaterVfx {
    pub lifetime: f32,
    pub max_lifetime: f32,
    pub velocity: Vec2,
}

/// Update power system state
/// Load is distributed proportionally across transformers based on their kVA rating.
pub fn power_system(
    mut multi_site: ResMut<MultiSiteManager>,
    chargers: Query<(&Charger, &BelongsToSite)>,
    mut transformers: Query<&mut Transformer>,
    game_clock: Res<GameClock>,
    time: Res<Time>,
    _site_config: Res<SiteConfig>,
    mut warning_events: MessageWriter<TransformerWarningEvent>,
) {
    if game_clock.is_paused() {
        return;
    }

    let Some(viewed_id) = multi_site.viewed_site_id else {
        return;
    };
    let Some(site_state) = multi_site.owned_sites.get_mut(&viewed_id) else {
        return;
    };
    let site_id = &viewed_id;

    // Reset phase loads for this site
    site_state.phase_loads.reset();

    // Sum up apparent power (kVA) from all chargers at this site
    // kVA = kW_output / (efficiency * power_factor)
    for (charger, belongs) in &chargers {
        if belongs.site_id == *site_id {
            // Calculate apparent power drawn from grid for this charger's output
            let apparent_power_kva = charger.input_kva(charger.current_power_kw);
            site_state
                .phase_loads
                .add_load(charger.phase, apparent_power_kva);
        }
    }

    let total_load_kva = site_state.phase_loads.total_load();

    // Drama delta: real-time scaled by drama_multiplier so higher game speed
    // compresses fire events (2x faster at 10x speed) while staying perceivable.
    let delta_drama = time.delta_secs() * game_clock.speed.drama_multiplier();

    // Calculate total transformer capacity for this site (for proportional distribution)
    let total_site_kva = site_state.grid.total_transformer_capacity();

    // Track hottest transformer for warning events (only fire one warning per site)
    let mut hottest_temp: f32 = 0.0;
    let mut any_warning = false;
    let mut any_critical = false;

    // Get site-specific ambient temperature based on archetype/climate
    let site_ambient_temp_c = site_state.archetype.ambient_temp_c();

    // Track whether any transformer incident is active; this hard-stops charging throughput.
    let mut any_active_fire = false;

    // Update each transformer at this site with its proportional load (in kVA)
    for mut transformer in &mut transformers {
        // Only process transformers belonging to this site
        if transformer.site_id != *site_id {
            continue;
        }

        // Burning or destroyed transformers are out of service until replaced.
        if transformer.on_fire || transformer.destroyed {
            transformer.current_load_kva = 0.0;
            any_active_fire = true;
            continue;
        }

        // Update ambient temperature based on site climate
        transformer.ambient_temp_c = site_ambient_temp_c;

        // Proportional load distribution: load = total_load * (this_kva / total_kva)
        let load_share = if total_site_kva > 0.0 {
            total_load_kva * (transformer.rating_kva / total_site_kva)
        } else {
            0.0
        };
        transformer.current_load_kva = load_share;

        transformer.update_temperature(delta_drama);
        transformer.update_visual_tier();

        // Track hottest transformer
        if transformer.current_temp_c > hottest_temp {
            hottest_temp = transformer.current_temp_c;
        }

        // Check for warnings on this transformer
        if transformer.is_critical() {
            any_critical = true;
        } else if transformer.is_warning() {
            any_warning = true;
        }
    }

    // Thermal throttle: only active when Smart Load Shedding upgrade is purchased
    // AND no hacker overload is in effect. While hacker overload is active, the
    // load shedding is bypassed and chargers draw unrestricted power.
    site_state.thermal_throttle_factor = if site_state.hacker_overload_remaining_secs > 0.0 {
        1.0
    } else if site_state.site_upgrades.has_smart_load_shedding() {
        if hottest_temp >= 90.0 {
            0.25
        } else if hottest_temp >= 75.0 {
            1.0 - 0.5 * ((hottest_temp - 75.0) / 15.0)
        } else {
            1.0
        }
    } else {
        1.0
    };

    if any_active_fire {
        site_state.thermal_throttle_factor = 0.0;
    }

    // Fire a single warning event for the site based on hottest transformer
    if any_critical {
        warning_events.write(TransformerWarningEvent {
            temperature: hottest_temp,
            is_critical: true,
        });
    } else if any_warning {
        warning_events.write(TransformerWarningEvent {
            temperature: hottest_temp,
            is_critical: false,
        });
    }

    // Calculate voltage sag based on load vs capacity (using kVA)
    let capacity = site_state.effective_capacity_kva();
    if capacity > 0.0 {
        let load_ratio = total_load_kva / capacity;
        // Simple voltage sag model: voltage drops under heavy load
        let voltage_pct = (100.0 - load_ratio * 10.0).clamp(75.0, 100.0);
        site_state.voltage_state.current_voltage_pct = voltage_pct;
    }
}

pub fn update_transformer_overload_fire_state(
    mut transformers: Query<&mut Transformer>,
    game_clock: Res<GameClock>,
    time: Res<Time>,
    mut game_state: ResMut<GameState>,
    multi_site: Res<MultiSiteManager>,
    mut overload_events: MessageWriter<TransformerOverloadWarningEvent>,
    mut fire_events: MessageWriter<TransformerFireEvent>,
) {
    if game_clock.is_paused() {
        return;
    }

    let delta_drama = time.delta_secs() * game_clock.speed.drama_multiplier();

    for mut transformer in &mut transformers {
        if transformer.on_fire || transformer.destroyed {
            continue;
        }

        // Fire risk is temperature-driven, not load-threshold-driven.
        // Sustained high utilization (even within rated capacity) heats the
        // transformer over time. Once it crosses the critical temperature
        // threshold (90 C), the fire countdown starts ticking.
        if transformer.is_critical() {
            transformer.overload_seconds += delta_drama;
        } else {
            // Cool down when temperature drops below critical.
            // Warning zone cools slowly; normal zone cools faster.
            let cooldown_rate = if transformer.is_warning() { 1.0 } else { 2.0 };
            transformer.overload_seconds =
                (transformer.overload_seconds - delta_drama * cooldown_rate).max(0.0);
            if transformer.overload_seconds <= 0.0 {
                transformer.last_warning_level = 0;
            }
        }

        let overload_pct = transformer.overload_seconds / FIRE_IGNITION_OVERLOAD_SECONDS;

        let has_apm = multi_site
            .get_site(transformer.site_id)
            .map(|s| s.site_upgrades.has_power_management())
            .unwrap_or(false);

        if overload_pct >= 0.67 && transformer.last_warning_level < 2 {
            transformer.last_warning_level = 2;
            overload_events.write(TransformerOverloadWarningEvent {
                severity: OverloadSeverity::Critical,
                overload_pct,
                has_power_management: has_apm,
            });
        } else if overload_pct >= 0.33 && transformer.last_warning_level < 1 {
            transformer.last_warning_level = 1;
            overload_events.write(TransformerOverloadWarningEvent {
                severity: OverloadSeverity::Warning,
                overload_pct,
                has_power_management: has_apm,
            });
        }

        if transformer.overload_seconds >= FIRE_IGNITION_OVERLOAD_SECONDS {
            transformer.on_fire = true;
            transformer.firetruck_dispatched = false;
            transformer.overload_seconds = FIRE_IGNITION_OVERLOAD_SECONDS;
            transformer.last_warning_level = 0;
            game_state.add_penalty(FIRE_INCIDENT_FINE);
            game_state.change_reputation(-10);
            fire_events.write(TransformerFireEvent {
                grid_pos: transformer.grid_pos,
            });
            warn!(
                "Transformer at {:?} ignited after overload. Fine: ${:.0}, Rep: -10",
                transformer.grid_pos, FIRE_INCIDENT_FINE
            );
        }
    }
}

pub fn dispatch_firetrucks_to_transformer_fires(
    mut commands: Commands,
    mut transformers: Query<(Entity, &mut Transformer)>,
    multi_site: Res<MultiSiteManager>,
    image_assets: Res<ImageAssets>,
    images: Res<Assets<Image>>,
) {
    for (transformer_entity, mut transformer) in &mut transformers {
        if !transformer.on_fire || transformer.firetruck_dispatched {
            continue;
        }

        let Some(site) = multi_site.get_site(transformer.site_id) else {
            continue;
        };

        let world_offset = site.world_offset();
        let entry_world =
            SiteGrid::grid_to_world(site.grid.entry_pos.0, site.grid.entry_pos.1) + world_offset;
        let exit_world =
            SiteGrid::grid_to_world(site.grid.exit_pos.0, site.grid.exit_pos.1) + world_offset;
        let target_world = SiteGrid::multi_tile_center(
            transformer.grid_pos.0,
            transformer.grid_pos.1,
            crate::resources::StructureSize::TwoByTwo,
        ) + world_offset;

        let transformer_neighbor =
            find_nearest_driveable_neighbor(&site.grid, transformer.grid_pos);
        let grid_path = bfs_path_grid(&site.grid, site.grid.entry_pos, transformer_neighbor);
        // Stop at the driveable neighbor tile, not on top of the transformer.
        // The firetruck sprays water toward target_world from this position.
        let waypoints: Vec<Vec2> = grid_path
            .iter()
            .map(|&(gx, gy)| SiteGrid::grid_to_world(gx, gy) + world_offset)
            .collect();

        let firetruck_image = image_assets.vehicle_firetruck.clone();
        let intended_size =
            crate::resources::sprite_metadata::vehicle_world_size(VehicleType::Firetruck);
        let sprite_scale = if let Some(image) = images.get(&firetruck_image) {
            intended_size.scale_for_image(image)
        } else {
            0.2
        };

        let spawn_pos = waypoints.first().copied().unwrap_or(entry_world);

        // Spawned as a top-level entity (not a child of site root) because
        // waypoints and target_world use world coordinates. Water spray VFX
        // also spawns at world coordinates from target_world.
        commands.spawn((
            Sprite::from_image(firetruck_image),
            Transform::from_xyz(spawn_pos.x, spawn_pos.y, 6.0)
                .with_scale(Vec3::splat(sprite_scale)),
            EmergencyFiretruck {
                site_id: transformer.site_id,
                transformer_entity,
                target_world,
                exit_world,
                phase: FiretruckPhase::Responding,
                spray_seconds_remaining: FIRE_SUPPRESSION_SECONDS,
                spray_spawn_cooldown: 0.0,
                speed: FIRETRUCK_SPEED,
                waypoints,
                current_waypoint: 0,
            },
            BelongsToSite::new(transformer.site_id),
        ));

        transformer.firetruck_dispatched = true;
        info!(
            "Dispatched emergency firetruck to transformer at {:?}",
            transformer.grid_pos
        );
    }
}

pub fn update_emergency_firetruck_response(
    mut commands: Commands,
    mut firetrucks: Query<(Entity, &mut Transform, &mut EmergencyFiretruck)>,
    mut transformers: Query<&mut Transformer>,
    game_clock: Res<GameClock>,
    time: Res<Time>,
    image_assets: Res<ImageAssets>,
    images: Res<Assets<Image>>,
) {
    if game_clock.is_paused() {
        return;
    }

    let delta_drama = time.delta_secs() * game_clock.speed.drama_multiplier();

    for (entity, mut transform, mut firetruck) in &mut firetrucks {
        match firetruck.phase {
            FiretruckPhase::Responding => {
                let reached_target =
                    advance_firetruck_along_waypoints(&mut transform, &mut firetruck, delta_drama);
                if reached_target {
                    let truck_pos = transform.translation.truncate();
                    let to_target = firetruck.target_world - truck_pos;
                    if to_target.length_squared() > 0.01 {
                        let heading = to_target.x.atan2(to_target.y);
                        transform.rotation = Quat::from_rotation_z(heading);
                    }
                    firetruck.phase = FiretruckPhase::Spraying;
                    firetruck.spray_seconds_remaining = FIRE_SUPPRESSION_SECONDS;
                    firetruck.spray_spawn_cooldown = 0.0;
                }
            }
            FiretruckPhase::Spraying => {
                firetruck.spray_seconds_remaining -= delta_drama;
                firetruck.spray_spawn_cooldown -= delta_drama;

                if firetruck.spray_spawn_cooldown <= 0.0 {
                    firetruck.spray_spawn_cooldown = WATER_SPRAY_SPAWN_SECONDS;
                    let spray_image = image_assets.vfx_light_pulse_blue.clone();
                    let spray_scale = if let Some(image) = images.get(&spray_image) {
                        let w = image.width().max(1) as f32;
                        48.0 / w
                    } else {
                        0.6
                    };

                    let truck_pos = transform.translation.truncate();
                    let fire_center = Vec2::new(
                        firetruck.target_world.x,
                        firetruck.target_world.y + PIXEL_FIRE_WORLD_H * 0.25,
                    );
                    let to_fire = fire_center - truck_pos;
                    let flight_time = 0.35;
                    let base_vel = to_fire / flight_time;
                    let spread = Vec2::new(
                        (rand::random::<f32>() - 0.5) * 80.0,
                        (rand::random::<f32>() - 0.5) * 60.0,
                    );

                    commands.spawn((
                        Sprite {
                            image: spray_image,
                            color: Color::srgba(0.45, 0.8, 1.0, 0.9),
                            ..default()
                        },
                        Transform::from_xyz(truck_pos.x, truck_pos.y + 10.0, 8.0)
                            .with_scale(Vec3::splat(spray_scale)),
                        TransformerWaterVfx {
                            lifetime: 0.0,
                            max_lifetime: 0.4,
                            velocity: base_vel + spread,
                        },
                        BelongsToSite::new(firetruck.site_id),
                    ));
                }

                if firetruck.spray_seconds_remaining <= 0.0 {
                    if let Ok(mut transformer) = transformers.get_mut(firetruck.transformer_entity)
                    {
                        transformer.on_fire = false;
                        transformer.destroyed = true;
                        transformer.firetruck_dispatched = false;
                        transformer.current_temp_c = transformer.ambient_temp_c + 10.0;
                        transformer.overload_seconds = 0.0;
                    }
                    // Build return waypoints by reversing approach path toward exit
                    let reverse: Vec<Vec2> = firetruck.waypoints.iter().copied().rev().collect();
                    firetruck.waypoints = reverse;
                    firetruck.current_waypoint = 0;
                    firetruck.phase = FiretruckPhase::Returning;
                }
            }
            FiretruckPhase::Returning => {
                let reached_exit =
                    advance_firetruck_along_waypoints(&mut transform, &mut firetruck, delta_drama);
                if reached_exit {
                    commands.entity(entity).try_despawn();
                }
            }
        }
    }
}

const PIXEL_FIRE_W: u32 = 28;
const PIXEL_FIRE_H: u32 = 48;
const PIXEL_FIRE_WORLD_W: f32 = 112.0;
const PIXEL_FIRE_WORLD_H: f32 = 144.0;
/// Maximum heat value = last palette index.
const FIRE_HEAT_MAX: u8 = (FIRE_PALETTE.len() - 1) as u8;

pub fn sync_transformer_fire_vfx(
    mut commands: Commands,
    transformers: Query<(Entity, &Transformer)>,
    existing_vfx: Query<(Entity, &PixelFire)>,
    multi_site: Res<MultiSiteManager>,
    mut images: ResMut<Assets<Image>>,
) {
    for (transformer_entity, transformer) in &transformers {
        let has_existing = existing_vfx
            .iter()
            .any(|(_, vfx)| vfx.transformer_entity == transformer_entity);

        if transformer.on_fire {
            if !has_existing {
                let Some(site) = multi_site.get_site(transformer.site_id) else {
                    continue;
                };
                let world = SiteGrid::multi_tile_center(
                    transformer.grid_pos.0,
                    transformer.grid_pos.1,
                    crate::resources::StructureSize::TwoByTwo,
                ) + site.world_offset();

                let w = PIXEL_FIRE_W;
                let h = PIXEL_FIRE_H;
                let pixel_count = (w * h) as usize;

                let mut heat = vec![0u8; pixel_count];
                for x in 0..w {
                    heat[((h - 1) * w + x) as usize] = FIRE_HEAT_MAX;
                }

                let rgba = vec![0u8; pixel_count * 4];
                let mut fire_image = Image::new(
                    bevy::render::render_resource::Extent3d {
                        width: w,
                        height: h,
                        depth_or_array_layers: 1,
                    },
                    bevy::render::render_resource::TextureDimension::D2,
                    rgba,
                    bevy::render::render_resource::TextureFormat::Rgba8UnormSrgb,
                    bevy::asset::RenderAssetUsages::RENDER_WORLD
                        | bevy::asset::RenderAssetUsages::MAIN_WORLD,
                );
                fire_image.sampler =
                    bevy::image::ImageSampler::Descriptor(bevy::image::ImageSamplerDescriptor {
                        mag_filter: bevy::image::ImageFilterMode::Nearest,
                        min_filter: bevy::image::ImageFilterMode::Nearest,
                        ..default()
                    });

                let image_handle = images.add(fire_image);

                let scale_x = PIXEL_FIRE_WORLD_W / w as f32;
                let scale_y = PIXEL_FIRE_WORLD_H / h as f32;

                commands.spawn((
                    Sprite::from_image(image_handle.clone()),
                    Transform::from_xyz(world.x, world.y + PIXEL_FIRE_WORLD_H * 0.25, 7.0)
                        .with_scale(Vec3::new(scale_x, scale_y, 1.0)),
                    PixelFire {
                        transformer_entity,
                        width: w,
                        height: h,
                        heat,
                        image_handle,
                    },
                    BelongsToSite::new(transformer.site_id),
                ));
            }
        } else {
            for (vfx_entity, vfx) in &existing_vfx {
                if vfx.transformer_entity == transformer_entity {
                    commands.entity(vfx_entity).try_despawn();
                }
            }
        }
    }
}

pub fn animate_transformer_fire_vfx(
    mut vfx_query: Query<&mut PixelFire>,
    game_clock: Res<GameClock>,
    mut images: ResMut<Assets<Image>>,
) {
    use rand::Rng;

    if game_clock.is_paused() {
        return;
    }

    let mut rng = rand::rng();

    for mut fire in &mut vfx_query {
        let w = fire.width as usize;
        let h = fire.height as usize;

        // Propagate fire upward (image row 0 = top, row h-1 = bottom/source).
        // Each pixel pulls heat from the row below with random horizontal drift
        // and random decay (classic Doom fire algorithm).
        for y in 0..(h - 1) {
            for x in 0..w {
                let src_x = (x as i32 + rng.random_range(-1..=1)).clamp(0, w as i32 - 1) as usize;
                let src_y = y + 1;
                let src_idx = src_y * w + src_x;
                let dst_idx = y * w + x;
                let decay = rng.random_range(0u8..=3);
                fire.heat[dst_idx] = fire.heat[src_idx].saturating_sub(decay);
            }
        }

        // Refresh source row with edge tapering for a natural flame silhouette.
        // Center columns burn at full intensity; edges taper down so the flame
        // narrows before going transparent.
        let half_w = w / 2;
        for x in 0..w {
            let idx = (h - 1) * w + x;
            let edge_dist = x.min(w - 1 - x);
            let peak = if edge_dist < 4 {
                // Edges: ramp from ~16 to ~30 over the outermost 4 columns
                (16 + edge_dist * 4).min(FIRE_HEAT_MAX as usize) as u8
            } else if edge_dist < half_w.saturating_sub(2) {
                rng.random_range(FIRE_HEAT_MAX - 3..=FIRE_HEAT_MAX)
            } else {
                FIRE_HEAT_MAX
            };
            fire.heat[idx] = rng.random_range(peak.saturating_sub(2)..=peak);
        }

        // Write pixel data into the Image (heat value IS the palette index)
        let Some(image) = images.get_mut(&fire.image_handle) else {
            continue;
        };
        let Some(data) = image.data.as_mut() else {
            continue;
        };
        let max_idx = FIRE_PALETTE.len() - 1;
        for (i, &heat_val) in fire.heat.iter().enumerate() {
            let [r, g, b, a] = FIRE_PALETTE[(heat_val as usize).min(max_idx)];
            let offset = i * 4;
            data[offset] = r;
            data[offset + 1] = g;
            data[offset + 2] = b;
            data[offset + 3] = a;
        }
    }
}

pub fn update_water_spray_vfx(
    mut commands: Commands,
    mut sprays: Query<(
        Entity,
        &mut TransformerWaterVfx,
        &mut Transform,
        &mut Sprite,
    )>,
    game_clock: Res<GameClock>,
    time: Res<Time>,
) {
    if game_clock.is_paused() {
        return;
    }

    let delta_real = time.delta_secs();
    for (entity, mut spray, mut transform, mut sprite) in &mut sprays {
        spray.lifetime += delta_real;
        transform.translation.x += spray.velocity.x * delta_real;
        transform.translation.y += spray.velocity.y * delta_real;
        let progress = (spray.lifetime / spray.max_lifetime).clamp(0.0, 1.0);
        sprite.color = Color::srgba(0.45, 0.8, 1.0, 1.0 - progress);
        if spray.lifetime >= spray.max_lifetime {
            commands.entity(entity).try_despawn();
        }
    }
}

fn advance_firetruck_along_waypoints(
    transform: &mut Transform,
    firetruck: &mut EmergencyFiretruck,
    delta_seconds: f32,
) -> bool {
    if firetruck.current_waypoint >= firetruck.waypoints.len() {
        return true;
    }

    let mut remaining_distance = firetruck.speed * delta_seconds;

    while remaining_distance > 0.0 && firetruck.current_waypoint < firetruck.waypoints.len() {
        let target = firetruck.waypoints[firetruck.current_waypoint];
        let current = transform.translation.truncate();
        let to_target = target - current;
        let distance = to_target.length();

        if distance <= 1.0 {
            transform.translation.x = target.x;
            transform.translation.y = target.y;
            firetruck.current_waypoint += 1;
            continue;
        }

        let direction = to_target / distance;
        let travel = remaining_distance.min(distance);
        transform.translation.x += direction.x * travel;
        transform.translation.y += direction.y * travel;
        remaining_distance -= travel;

        let heading = direction.x.atan2(direction.y);
        transform.rotation = Quat::from_rotation_z(heading);

        if travel >= distance {
            firetruck.current_waypoint += 1;
        } else {
            break;
        }
    }

    firetruck.current_waypoint >= firetruck.waypoints.len()
}

fn bfs_path_grid(
    grid: &crate::resources::SiteGrid,
    start: (i32, i32),
    goal: (i32, i32),
) -> Vec<(i32, i32)> {
    use std::collections::{HashMap, VecDeque};

    if start == goal {
        return vec![start];
    }

    let mut queue = VecDeque::new();
    let mut came_from: HashMap<(i32, i32), (i32, i32)> = HashMap::new();
    queue.push_back(start);
    came_from.insert(start, start);

    let neighbors = [(0, 1), (0, -1), (1, 0), (-1, 0)];

    while let Some(current) = queue.pop_front() {
        if current == goal {
            let mut path = vec![current];
            let mut node = current;
            while came_from[&node] != node {
                node = came_from[&node];
                path.push(node);
            }
            path.reverse();
            return path;
        }

        for (dx, dy) in &neighbors {
            let next = (current.0 + dx, current.1 + dy);
            if came_from.contains_key(&next) || !grid.is_valid(next.0, next.1) {
                continue;
            }
            let content = grid.get_content(next.0, next.1);
            if content.is_driveable() || next == goal {
                came_from.insert(next, current);
                queue.push_back(next);
            }
        }
    }

    vec![start, goal]
}

fn find_nearest_driveable_neighbor(
    grid: &crate::resources::SiteGrid,
    anchor: (i32, i32),
) -> (i32, i32) {
    // The transformer is 2x2 occupying anchor, anchor+1 in both axes.
    // Check the full perimeter around the footprint for a driveable tile.
    let (ax, ay) = anchor;
    let perimeter: [(i32, i32); 12] = [
        // Bottom edge (below the 2x2)
        (ax, ay - 1),
        (ax + 1, ay - 1),
        // Top edge (above the 2x2)
        (ax, ay + 2),
        (ax + 1, ay + 2),
        // Left edge
        (ax - 1, ay),
        (ax - 1, ay + 1),
        // Right edge
        (ax + 2, ay),
        (ax + 2, ay + 1),
        // Corners
        (ax - 1, ay - 1),
        (ax + 2, ay - 1),
        (ax - 1, ay + 2),
        (ax + 2, ay + 2),
    ];
    for n in &perimeter {
        if grid.is_valid(n.0, n.1) && grid.get_content(n.0, n.1).is_driveable() {
            return *n;
        }
    }
    // Fallback: return a position below the anchor
    (ax, ay - 1)
}
