//! Cyber-attack / hacker systems
//!
//! At random intervals (higher chance at night), a hacker spawns at a random
//! map edge, walks directly to infrastructure, hacks for a duration, then
//! either executes the attack (overload or price slash) or fails (firewalled).

use bevy::prelude::*;
use rand::Rng;
use rand::prelude::IndexedRandom;

use crate::audio::{PlaySfx, SfxKind};
use crate::components::hacker::{
    HACKER_NAMES, Hacker, HackerAttackType, HackerPhase, HackerVariant,
};
use crate::components::site::BelongsToSite;
use crate::events::{HackerAttackEvent, HackerDetectedEvent};
use crate::resources::{
    GRID_HEIGHT, GRID_OFFSET_X, GRID_OFFSET_Y, GRID_WIDTH, GameClock, ImageAssets,
    MultiSiteManager, SiteGrid, StructureSize, TILE_SIZE,
};
use crate::systems::sprite::{HackerLootBubble, HackingGlitchVfx};

/// Base probability of a hack attempt per game-hour per site.
const BASE_HACK_CHANCE_PER_HOUR: f32 = 0.005;

/// Night-time multiplier (10 PM – 5 AM).
const NIGHT_MULTIPLIER: f32 = 3.0;

/// Speed at which the hacker walks (pixels per second).
const HACKER_SPEED: f32 = 80.0;

/// Minimum hacking duration in game seconds (15 minutes).
const HACK_DURATION_MIN: f32 = 900.0;
/// Maximum hacking duration in game seconds (45 minutes).
const HACK_DURATION_MAX: f32 = 2700.0;

/// How close (in pixels) the hacker must be to its target to count as "arrived".
const ARRIVAL_THRESHOLD: f32 = 8.0;

/// Duration of the overload effect in game seconds (30 minutes).
/// Intentionally longer than the price slash so the boosted power density has
/// enough drama-time to heat the transformer to ignition.
const OVERLOAD_EFFECT_DURATION: f32 = 1800.0;

/// Duration of the price slash effect in game seconds (15 minutes).
const PRICE_SLASH_EFFECT_DURATION: f32 = 900.0;

/// Slashed price in $/kWh.
const HACKED_PRICE: f32 = 0.01;

/// Power density override during a TransformerOverload hack.
/// The player slider caps at 2.0 (200%); the hacker pushes beyond that.
pub const HACKER_OVERLOAD_POWER_DENSITY: f32 = 2.5;

/// Agentic SOC auto-terminate threshold in game seconds (5 minutes).
const AGENTIC_SOC_TERMINATE_SECS: f32 = 300.0;

/// Tracks whether a hack has been triggered today (informational).
#[derive(Resource, Default)]
pub struct DailyHackerTracker {
    pub hack_triggered_today: bool,
    pub last_day: u32,
}

// ─────────────────────────────────────────────────────
//  helpers
// ─────────────────────────────────────────────────────

fn random_edge_position(rng: &mut impl Rng, grid_w: i32, grid_h: i32, world_offset: Vec2) -> Vec2 {
    let margin = 40.0;
    let base_x = world_offset.x + GRID_OFFSET_X;
    let base_y = world_offset.y + GRID_OFFSET_Y;
    let grid_min_x = base_x - margin;
    let grid_max_x = base_x + (grid_w as f32) * TILE_SIZE + margin;
    let grid_min_y = base_y - margin;
    let grid_max_y = base_y + (grid_h as f32) * TILE_SIZE + margin;

    match rng.random_range(0..4) {
        0 => Vec2::new(rng.random_range(grid_min_x..grid_max_x), grid_max_y),
        1 => Vec2::new(rng.random_range(grid_min_x..grid_max_x), grid_min_y),
        2 => Vec2::new(grid_min_x, rng.random_range(grid_min_y..grid_max_y)),
        _ => Vec2::new(grid_max_x, rng.random_range(grid_min_y..grid_max_y)),
    }
}

/// Pick a target position on actual infrastructure based on attack type.
fn pick_infrastructure_target(
    grid: &SiteGrid,
    attack_type: HackerAttackType,
    world_offset: Vec2,
    rng: &mut impl Rng,
) -> Vec2 {
    match attack_type {
        HackerAttackType::TransformerOverload => {
            if let Some(t) = grid.transformers.choose(rng) {
                return SiteGrid::multi_tile_center(t.pos.0, t.pos.1, StructureSize::TwoByTwo)
                    + world_offset;
            }
        }
        HackerAttackType::PriceSlash => {
            let bays = grid.get_charger_bays();
            if let Some(&(x, y, _)) = bays.choose(rng) {
                return SiteGrid::grid_to_world(x, y) + world_offset;
            }
        }
    }
    // Fallback: grid center
    Vec2::new(
        world_offset.x + GRID_OFFSET_X + (grid.width as f32) * TILE_SIZE * 0.5,
        world_offset.y + GRID_OFFSET_Y + (grid.height as f32) * TILE_SIZE * 0.5,
    )
}

// ─────────────────────────────────────────────────────
//  spawn helper (shared by probability + debug paths)
// ─────────────────────────────────────────────────────

fn spawn_hacker_entity(
    commands: &mut Commands,
    site_id: crate::resources::multi_site::SiteId,
    grid: &SiteGrid,
    world_offset: Vec2,
    image_assets: &ImageAssets,
    images: &Assets<Image>,
    tracker: &mut DailyHackerTracker,
) {
    let mut rng = rand::rng();

    let attack_type = if rng.random::<bool>() {
        HackerAttackType::TransformerOverload
    } else {
        HackerAttackType::PriceSlash
    };

    let variant = if rng.random::<bool>() {
        HackerVariant::Green
    } else {
        HackerVariant::Purple
    };
    let name = HACKER_NAMES[rng.random_range(0..HACKER_NAMES.len())];

    let walking_image = match variant {
        HackerVariant::Green => image_assets.character_hacker_walking_green.clone(),
        HackerVariant::Purple => image_assets.character_hacker_walking_purple.clone(),
    };
    let hacker_size = crate::resources::sprite_metadata::hacker_world_size();
    let scale = if let Some(image) = images.get(&walking_image) {
        hacker_size.scale_for_image(image)
    } else {
        0.35
    };

    let hack_duration = rng.random_range(HACK_DURATION_MIN..=HACK_DURATION_MAX);
    let spawn_pos = random_edge_position(&mut rng, grid.width, grid.height, world_offset);
    let target_pos = pick_infrastructure_target(grid, attack_type, world_offset, &mut rng);

    commands.spawn((
        Hacker {
            target_site: site_id,
            target_pos,
            phase: HackerPhase::Infiltrating,
            attack_type,
            hack_timer: hack_duration,
            move_target: target_pos,
            name,
            variant,
            anim_timer: 0.0,
            base_y: spawn_pos.y,
        },
        Sprite::from_image(walking_image),
        Transform::from_xyz(spawn_pos.x, spawn_pos.y, 15.0).with_scale(Vec3::splat(scale)),
        GlobalTransform::default(),
        BelongsToSite { site_id },
    ));

    tracker.hack_triggered_today = true;

    info!(
        "Hacker \"{}\" ({:?}) spawned, attack: {:?}, targeting site {:?}",
        name, variant, attack_type, site_id,
    );
}

// ─────────────────────────────────────────────────────
//  hacker_spawn_system
// ─────────────────────────────────────────────────────

pub fn hacker_spawn_system(
    mut commands: Commands,
    existing_hackers: Query<&Hacker>,
    game_clock: Res<GameClock>,
    time: Res<Time>,
    multi_site: Res<MultiSiteManager>,
    image_assets: Res<ImageAssets>,
    images: Res<Assets<Image>>,
    mut tracker: ResMut<DailyHackerTracker>,
    tutorial_state: Option<Res<crate::resources::TutorialState>>,
) {
    if game_clock.is_paused() {
        return;
    }
    if tutorial_state.as_ref().is_some_and(|ts| ts.is_active()) {
        return;
    }

    let delta_hours = (time.delta_secs() * game_clock.speed.multiplier()) / 3600.0;
    if delta_hours <= 0.0 {
        return;
    }

    if game_clock.day != tracker.last_day {
        tracker.hack_triggered_today = false;
        tracker.last_day = game_clock.day;
    }

    let hour = game_clock.hour();
    let night_mult = if !(5..22).contains(&hour) {
        NIGHT_MULTIPLIER
    } else {
        1.0
    };

    let base_chance = BASE_HACK_CHANCE_PER_HOUR * night_mult * delta_hours;
    if base_chance <= 0.0 {
        return;
    }

    let mut rng = rand::rng();

    let Some(viewed_id) = multi_site.viewed_site_id else {
        return;
    };
    let Some(site_state) = multi_site.owned_sites.get(&viewed_id) else {
        return;
    };
    let site_id = site_state.id;

    if existing_hackers.iter().next().is_some() {
        return;
    }

    let challenge_mult = match site_state.challenge_level {
        0..=1 => 0.25,
        2 => 0.6,
        3 => 1.0,
        4 => 1.25,
        _ => 1.5,
    };

    let site_chance = base_chance * challenge_mult;

    if rng.random::<f32>() >= site_chance {
        return;
    }

    spawn_hacker_entity(
        &mut commands,
        site_id,
        &site_state.grid,
        site_state.world_offset(),
        &image_assets,
        &images,
        &mut tracker,
    );
}

// ─────────────────────────────────────────────────────
//  debug_spawn_hacker  (Shift+H — hidden cheat key)
// ─────────────────────────────────────────────────────

pub fn debug_spawn_hacker(
    mut commands: Commands,
    keyboard: Res<ButtonInput<KeyCode>>,
    existing_hackers: Query<Entity, With<Hacker>>,
    multi_site: Res<MultiSiteManager>,
    image_assets: Res<ImageAssets>,
    images: Res<Assets<Image>>,
    mut tracker: ResMut<DailyHackerTracker>,
    glitch_vfx: Query<Entity, With<crate::systems::sprite::HackingGlitchVfx>>,
    loot_bubbles: Query<Entity, With<crate::systems::sprite::HackerLootBubble>>,
) {
    let shift_held = keyboard.pressed(KeyCode::ShiftLeft) || keyboard.pressed(KeyCode::ShiftRight);
    if !shift_held || !keyboard.just_pressed(KeyCode::KeyH) {
        return;
    }

    let Some(viewed_id) = multi_site.viewed_site_id else {
        info!("Debug: no viewed site, cannot spawn hacker");
        return;
    };
    let Some(site_state) = multi_site.owned_sites.get(&viewed_id) else {
        info!("Debug: viewed site not in owned_sites");
        return;
    };

    for entity in &existing_hackers {
        commands.entity(entity).try_despawn();
    }
    for entity in &glitch_vfx {
        commands.entity(entity).try_despawn();
    }
    for entity in &loot_bubbles {
        commands.entity(entity).try_despawn();
    }

    spawn_hacker_entity(
        &mut commands,
        site_state.id,
        &site_state.grid,
        site_state.world_offset(),
        &image_assets,
        &images,
        &mut tracker,
    );

    info!("Debug: force-spawned hacker via Shift+H (cleared existing)");
}

// ─────────────────────────────────────────────────────
//  hacker_movement_system
// ─────────────────────────────────────────────────────

pub fn hacker_movement_system(
    time: Res<Time>,
    game_clock: Res<GameClock>,
    mut query: Query<(&mut Hacker, &mut Transform)>,
) {
    if game_clock.is_paused() {
        return;
    }

    let dt = time.delta_secs();
    let speed_multiplier = game_clock.speed.visual_multiplier();

    let bob_speed = 10.0;
    let bob_amplitude = 2.5;
    let sway_amplitude = 0.06;

    for (mut hacker, mut transform) in query.iter_mut() {
        if hacker.phase != HackerPhase::Infiltrating && hacker.phase != HackerPhase::Fleeing {
            transform.rotation = Quat::IDENTITY;
            continue;
        }

        let current = transform.translation.truncate();
        let target = hacker.move_target;
        let distance = current.distance(target);

        if distance < ARRIVAL_THRESHOLD {
            transform.translation.x = target.x;
            transform.translation.y = target.y;
            transform.rotation = Quat::IDENTITY;
            continue;
        }

        hacker.anim_timer += dt;

        let speed = HACKER_SPEED * speed_multiplier;
        let step = speed * dt;
        let direction = (target - current).normalize();

        transform.translation.x += direction.x * step;
        transform.translation.y += direction.y * step;

        hacker.base_y = transform.translation.y;

        let bob_offset = bevy::math::ops::sin(hacker.anim_timer * bob_speed).abs() * bob_amplitude;
        transform.translation.y += bob_offset;

        let sway = bevy::math::ops::sin(hacker.anim_timer * bob_speed) * sway_amplitude;
        transform.rotation = Quat::from_rotation_z(sway);

        let abs_scale = transform.scale.x.abs();
        if direction.x < -0.1 {
            transform.scale.x = -abs_scale;
        } else if direction.x > 0.1 {
            transform.scale.x = abs_scale;
        }
    }
}

// ─────────────────────────────────────────────────────
//  hacker_arrival_system
// ─────────────────────────────────────────────────────

pub fn hacker_arrival_system(
    mut commands: Commands,
    mut hackers: Query<(Entity, &mut Hacker, &Transform)>,
    image_assets: Res<ImageAssets>,
    images: Res<Assets<Image>>,
    mut sfx_writer: MessageWriter<PlaySfx>,
) {
    for (entity, mut hacker, hacker_transform) in hackers.iter_mut() {
        if hacker.phase != HackerPhase::Infiltrating {
            continue;
        }

        let current = hacker_transform.translation.truncate();
        if current.distance(hacker.move_target) > ARRIVAL_THRESHOLD {
            continue;
        }

        hacker.phase = HackerPhase::Hacking;

        let hacking_image = match hacker.variant {
            HackerVariant::Green => image_assets.character_hacker_hacking_green.clone(),
            HackerVariant::Purple => image_assets.character_hacker_hacking_purple.clone(),
        };
        commands
            .entity(entity)
            .insert(Sprite::from_image(hacking_image));

        // Green glitch VFX at hacker position
        let vfx_image = image_assets.vfx_light_pulse_yellow.clone();
        let vfx_size = crate::resources::sprite_metadata::vfx_world_size(
            crate::resources::VfxType::UrgentPulse,
        );
        let vfx_scale = if let Some(image) = images.get(&vfx_image) {
            vfx_size.scale_for_image(image)
        } else {
            1.0
        };

        let pos = hacker_transform.translation;
        commands.spawn((
            Sprite::from_image(vfx_image),
            Transform::from_xyz(pos.x, pos.y, 11.0).with_scale(Vec3::splat(vfx_scale)),
            HackingGlitchVfx {
                hacker_entity: entity,
                flash_time: 0.0,
            },
        ));

        sfx_writer.write(PlaySfx(SfxKind::AlarmTheft));

        info!(
            "\"{}\" arrived at target, hacking for {:.0}s ({:?})",
            hacker.name, hacker.hack_timer, hacker.attack_type
        );
    }
}

// ─────────────────────────────────────────────────────
//  hacker_attack_system
// ─────────────────────────────────────────────────────

pub fn hacker_attack_system(
    mut commands: Commands,
    mut hackers: Query<(Entity, &mut Hacker, &BelongsToSite)>,
    game_clock: Res<GameClock>,
    time: Res<Time>,
    image_assets: Res<ImageAssets>,
    mut multi_site: ResMut<MultiSiteManager>,
    glitch_vfx: Query<(Entity, &HackingGlitchVfx)>,
    mut attack_events: MessageWriter<HackerAttackEvent>,
    mut detected_events: MessageWriter<HackerDetectedEvent>,
    tutorial_state: Option<Res<crate::resources::TutorialState>>,
) {
    if game_clock.is_paused() {
        return;
    }
    if tutorial_state.as_ref().is_some_and(|ts| ts.is_active()) {
        return;
    }

    let delta = time.delta_secs() * game_clock.speed.multiplier();
    let mut rng = rand::rng();

    for (entity, mut hacker, belongs) in hackers.iter_mut() {
        if hacker.phase != HackerPhase::Hacking {
            continue;
        }

        hacker.hack_timer -= delta;
        if hacker.hack_timer > 0.0 {
            continue;
        }

        let has_firewall = multi_site
            .get_site(belongs.site_id)
            .map(|s| s.site_upgrades.has_cyber_firewall)
            .unwrap_or(false);
        let has_soc = multi_site
            .get_site(belongs.site_id)
            .map(|s| s.site_upgrades.has_agentic_soc)
            .unwrap_or(false);

        let success_chance = if has_soc {
            0.02
        } else if has_firewall {
            0.50
        } else {
            1.0
        };

        let attack_succeeded = rng.random::<f32>() < success_chance;

        if attack_succeeded {
            if let Some(site_state) = multi_site.get_site_mut(belongs.site_id) {
                match hacker.attack_type {
                    HackerAttackType::TransformerOverload => {
                        site_state.hacker_overload_remaining_secs = OVERLOAD_EFFECT_DURATION;
                        info!(
                            "\"{}\" hacked transformer overload on site {:?}!",
                            hacker.name, belongs.site_id
                        );
                    }
                    HackerAttackType::PriceSlash => {
                        site_state.service_strategy.pricing.hacker_price_override =
                            Some(HACKED_PRICE);
                        site_state
                            .service_strategy
                            .pricing
                            .hacker_price_override_remaining_secs = PRICE_SLASH_EFFECT_DURATION;
                        info!(
                            "\"{}\" slashed price to ${} on site {:?}!",
                            hacker.name, HACKED_PRICE, belongs.site_id
                        );
                    }
                }
            }

            attack_events.write(HackerAttackEvent {
                site_id: belongs.site_id,
                attack_type: hacker.attack_type,
            });

            let (text, color) = match hacker.attack_type {
                HackerAttackType::TransformerOverload => {
                    ("BURN IT DOWN", Color::srgb(1.0, 0.6, 0.1))
                }
                HackerAttackType::PriceSlash => ("POWER TO THE PEOPLE", Color::srgb(0.1, 1.0, 0.1)),
            };

            commands.spawn((
                Text2d::new(text),
                TextFont {
                    font_size: 18.0,
                    ..default()
                },
                TextColor(color),
                Transform::from_xyz(0.0, 48.0, 20.0),
                GlobalTransform::default(),
                HackerLootBubble {
                    hacker_entity: entity,
                    lifetime: 0.0,
                },
            ));
        } else {
            detected_events.write(HackerDetectedEvent {
                site_id: belongs.site_id,
                attack_type: hacker.attack_type,
                auto_blocked: false,
            });

            let text = match hacker.attack_type {
                HackerAttackType::TransformerOverload => "FIREWALLED",
                HackerAttackType::PriceSlash => "ACCESS DENIED",
            };

            commands.spawn((
                Text2d::new(text),
                TextFont {
                    font_size: 22.0,
                    ..default()
                },
                TextColor(Color::srgb(1.0, 0.1, 0.1)),
                Transform::from_xyz(0.0, 48.0, 20.0),
                GlobalTransform::default(),
                HackerLootBubble {
                    hacker_entity: entity,
                    lifetime: 0.0,
                },
            ));

            info!(
                "\"{}\" hack attempt blocked ({:?})!",
                hacker.name, hacker.attack_type
            );
        }

        // Despawn glitch VFX
        for (vfx_entity, vfx) in glitch_vfx.iter() {
            if vfx.hacker_entity == entity {
                commands.entity(vfx_entity).try_despawn();
            }
        }

        hacker.phase = HackerPhase::Fleeing;
        hacker.anim_timer = 0.0;

        let (gw, gh, wo) = multi_site
            .get_site(belongs.site_id)
            .map(|s| (s.grid.width, s.grid.height, s.world_offset()))
            .unwrap_or((GRID_WIDTH, GRID_HEIGHT, Vec2::ZERO));
        let exit_pos = random_edge_position(&mut rng, gw, gh, wo);
        hacker.move_target = exit_pos;

        let walking_image = match hacker.variant {
            HackerVariant::Green => image_assets.character_hacker_walking_green.clone(),
            HackerVariant::Purple => image_assets.character_hacker_walking_purple.clone(),
        };
        commands
            .entity(entity)
            .insert(Sprite::from_image(walking_image));

        info!(
            "\"{}\" fleeing to ({:.0},{:.0})",
            hacker.name, exit_pos.x, exit_pos.y
        );
    }
}

// ─────────────────────────────────────────────────────
//  hacker_flee_arrival_system
// ─────────────────────────────────────────────────────

pub fn hacker_flee_arrival_system(mut hackers: Query<(&Transform, &mut Hacker)>) {
    for (transform, mut hacker) in hackers.iter_mut() {
        if hacker.phase != HackerPhase::Fleeing {
            continue;
        }
        let current = transform.translation.truncate();
        if current.distance(hacker.move_target) <= ARRIVAL_THRESHOLD {
            hacker.phase = HackerPhase::Gone;
            info!("Hacker reached exit edge, ready for cleanup");
        }
    }
}

// ─────────────────────────────────────────────────────
//  hacker_cleanup_system
// ─────────────────────────────────────────────────────

pub fn hacker_cleanup_system(mut commands: Commands, hackers: Query<(Entity, &Hacker)>) {
    for (entity, hacker) in hackers.iter() {
        if hacker.phase == HackerPhase::Gone {
            commands.entity(entity).try_despawn();
            info!("Hacker entity cleaned up");
        }
    }
}

// ─────────────────────────────────────────────────────
//  hacker_effect_tick_system
// ─────────────────────────────────────────────────────

/// Ticks down active hacker effect timers and clears them when expired.
/// Also handles Agentic SOC auto-terminate (cancels effects after 5 game-min).
pub fn hacker_effect_tick_system(
    game_clock: Res<GameClock>,
    time: Res<Time>,
    mut multi_site: ResMut<MultiSiteManager>,
    mut detected_events: MessageWriter<HackerDetectedEvent>,
) {
    if game_clock.is_paused() {
        return;
    }

    let delta = time.delta_secs() * game_clock.speed.hack_multiplier();

    for (_site_id, site_state) in multi_site.owned_sites.iter_mut() {
        // Tick overload timer
        if site_state.hacker_overload_remaining_secs > 0.0 {
            site_state.hacker_overload_remaining_secs =
                (site_state.hacker_overload_remaining_secs - delta).max(0.0);

            // Agentic SOC auto-terminate
            if site_state.site_upgrades.has_agentic_soc
                && site_state.hacker_overload_remaining_secs > 0.0
                && (OVERLOAD_EFFECT_DURATION - site_state.hacker_overload_remaining_secs)
                    >= AGENTIC_SOC_TERMINATE_SECS
            {
                info!(
                    "Agentic SOC auto-terminated overload on site {:?}",
                    site_state.id
                );
                site_state.hacker_overload_remaining_secs = 0.0;
                detected_events.write(HackerDetectedEvent {
                    site_id: site_state.id,
                    attack_type: HackerAttackType::TransformerOverload,
                    auto_blocked: true,
                });
            }
        }

        // Tick price override timer
        let pricing = &mut site_state.service_strategy.pricing;
        if pricing.hacker_price_override.is_some() {
            pricing.hacker_price_override_remaining_secs =
                (pricing.hacker_price_override_remaining_secs - delta).max(0.0);

            // Agentic SOC auto-terminate
            if site_state.site_upgrades.has_agentic_soc
                && pricing.hacker_price_override_remaining_secs > 0.0
                && (PRICE_SLASH_EFFECT_DURATION - pricing.hacker_price_override_remaining_secs)
                    >= AGENTIC_SOC_TERMINATE_SECS
            {
                info!(
                    "Agentic SOC auto-terminated price slash on site {:?}",
                    site_state.id
                );
                pricing.hacker_price_override = None;
                pricing.hacker_price_override_remaining_secs = 0.0;
                detected_events.write(HackerDetectedEvent {
                    site_id: site_state.id,
                    attack_type: HackerAttackType::PriceSlash,
                    auto_blocked: true,
                });
            }

            // Natural expiry
            if pricing.hacker_price_override_remaining_secs <= 0.0 {
                pricing.hacker_price_override = None;
            }
        }
    }
}
