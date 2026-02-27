//! Cable theft / robber systems
//!
//! At random intervals (higher chance at night), a robber spawns at a random
//! map edge, walks directly to a charger (ignoring roads/pathfinding), steals
//! the cable, then flees to a different random edge and despawns.

use bevy::prelude::*;
use rand::Rng;

use crate::audio::{PlaySfx, SfxKind};
use crate::components::BelongsToSite;
use crate::components::charger::{Charger, FaultType};
use crate::components::robber::{ROBBER_NAMES, Robber, RobberPhase, RobberVariant};
use crate::resources::{
    GRID_HEIGHT, GRID_OFFSET_X, GRID_OFFSET_Y, GRID_WIDTH, GameClock, ImageAssets,
    MultiSiteManager, TILE_SIZE,
};
use crate::systems::scene::SecuritySystemVisual;
use crate::systems::sprite::{
    RobberLootBubble, SecurityAlertBubble, StealingSparkVfx, StolenCableSprite, TheftAlarmVfx,
};

/// Base probability of a cable theft per game-hour per site.
const BASE_THEFT_CHANCE_PER_HOUR: f32 = 0.008;

/// Night-time multiplier (10 PM - 5 AM) — 4x higher chance at night.
const NIGHT_MULTIPLIER: f32 = 4.0;

/// Speed at which the robber walks (pixels per second, faster than technician).
const ROBBER_SPEED: f32 = 80.0;

/// Minimum stealing duration in game seconds (30 minutes).
const STEAL_DURATION_MIN: f32 = 1800.0;
/// Maximum stealing duration in game seconds (1 hour 15 minutes).
const STEAL_DURATION_MAX: f32 = 4500.0;

/// How close (in pixels) the robber must be to its target to count as "arrived".
const ARRIVAL_THRESHOLD: f32 = 8.0;

/// Legacy tracker — kept as a resource for compatibility but no longer drives
/// guaranteed daily robberies. Robbery is now purely probabilistic.
#[derive(Resource, Default)]
pub struct DailyRobberyTracker {
    pub robbery_triggered_today: bool,
    pub last_day: u32,
}

// ─────────────────────────────────────────────────────
//  helpers
// ─────────────────────────────────────────────────────

/// Generate a random world-space position on one of the four map edges.
/// The position is slightly outside the visible grid so the robber walks in/out of view.
fn random_edge_position(rng: &mut impl Rng, grid_w: i32, grid_h: i32) -> Vec2 {
    let margin = 40.0;
    let grid_min_x = GRID_OFFSET_X - margin;
    let grid_max_x = GRID_OFFSET_X + (grid_w as f32) * TILE_SIZE + margin;
    let grid_min_y = GRID_OFFSET_Y - margin;
    let grid_max_y = GRID_OFFSET_Y + (grid_h as f32) * TILE_SIZE + margin;

    match rng.random_range(0..4) {
        0 => Vec2::new(rng.random_range(grid_min_x..grid_max_x), grid_max_y),
        1 => Vec2::new(rng.random_range(grid_min_x..grid_max_x), grid_min_y),
        2 => Vec2::new(grid_min_x, rng.random_range(grid_min_y..grid_max_y)),
        _ => Vec2::new(grid_max_x, rng.random_range(grid_min_y..grid_max_y)),
    }
}

// ─────────────────────────────────────────────────────
//  cable_theft_system — random trigger
// ─────────────────────────────────────────────────────

/// Periodically checks whether a robber should spawn at each site.
/// Spawn chance scales with site challenge level and is reduced by security
/// systems and full anti-theft cable coverage.
pub fn cable_theft_system(
    mut commands: Commands,
    chargers: Query<(Entity, &Charger, &GlobalTransform, &BelongsToSite)>,
    existing_robbers: Query<&Robber>,
    game_clock: Res<GameClock>,
    time: Res<Time>,
    multi_site: Res<MultiSiteManager>,
    image_assets: Res<ImageAssets>,
    images: Res<Assets<Image>>,
    mut robbery_tracker: ResMut<DailyRobberyTracker>,
) {
    if game_clock.is_paused() {
        return;
    }

    let delta_hours = (time.delta_secs() * game_clock.speed.multiplier()) / 3600.0;
    if delta_hours <= 0.0 {
        return;
    }

    // Reset tracker on new day (informational only, no forced spawn)
    if game_clock.day != robbery_tracker.last_day {
        robbery_tracker.robbery_triggered_today = false;
        robbery_tracker.last_day = game_clock.day;
    }

    // Night boost
    let hour = game_clock.hour();
    let night_mult = if !(5..22).contains(&hour) {
        NIGHT_MULTIPLIER
    } else {
        1.0
    };

    let base_chance = BASE_THEFT_CHANCE_PER_HOUR * night_mult * delta_hours;
    if base_chance <= 0.0 {
        return;
    }

    let mut rng = rand::rng();

    for site_state in multi_site.owned_sites.values() {
        let site_id = site_state.id;

        // Only one active robber globally at a time
        if existing_robbers.iter().next().is_some() {
            continue;
        }

        // Scale by challenge level: level 1 = 0.25x, level 3 = 1.0x, level 5 = 1.5x
        let challenge_mult = match site_state.challenge_level {
            0..=1 => 0.25,
            2 => 0.6,
            3 => 1.0,
            4 => 1.25,
            _ => 1.5,
        };

        let mut site_chance = base_chance * challenge_mult;

        // Security system strongly deters robbers (0.25x spawn chance)
        if site_state.grid.has_security_system() {
            site_chance *= 0.25;
        }

        // If ALL chargers at this site have anti-theft cables, further deter (0.5x)
        let site_chargers: Vec<&Charger> = chargers
            .iter()
            .filter(|(_, _, _, b)| b.site_id == site_id)
            .map(|(_, c, _, _)| c)
            .collect();
        if !site_chargers.is_empty() && site_chargers.iter().all(|c| c.anti_theft_cable) {
            site_chance *= 0.5;
        }

        // Probabilistic roll only — no forced spawn
        if rng.random::<f32>() >= site_chance {
            continue;
        }

        // Find eligible chargers at this site (not faulted, not disabled, not actively charging)
        let eligible: Vec<(Entity, &Charger, Vec3)> = chargers
            .iter()
            .filter(|(_, c, _, belongs)| {
                belongs.site_id == site_id
                    && c.current_fault.is_none()
                    && !c.is_disabled
                    && !c.is_charging
            })
            .map(|(e, c, gt, _)| (e, c, gt.translation()))
            .collect();

        if eligible.is_empty() {
            continue;
        }

        // Pick a random charger
        let idx = rng.random_range(0..eligible.len());
        let (target_entity, target_charger, charger_world_pos) = eligible[idx];
        let charger_target = charger_world_pos.truncate();

        // Pick random variant, name
        let variant = if rng.random::<bool>() {
            RobberVariant::Black
        } else {
            RobberVariant::Pink
        };
        let name = ROBBER_NAMES[rng.random_range(0..ROBBER_NAMES.len())];

        // Walking sprite for this variant
        let walking_image = match variant {
            RobberVariant::Black => image_assets.character_robber_walking.clone(),
            RobberVariant::Pink => image_assets.character_robber_walking_pink.clone(),
        };

        let robber_size = crate::resources::sprite_metadata::robber_world_size();
        let scale = if let Some(image) = images.get(&walking_image) {
            robber_size.scale_for_image(image)
        } else {
            0.35
        };

        let steal_duration = rng.random_range(STEAL_DURATION_MIN..=STEAL_DURATION_MAX);

        // Random spawn position on a map edge
        let spawn_pos =
            random_edge_position(&mut rng, site_state.grid.width, site_state.grid.height);

        // Spawn robber (NOT as child of site root — free-floating in world space)
        let charger_id = target_charger.id.clone();
        commands.spawn((
            Robber {
                target_charger: target_entity,
                phase: RobberPhase::WalkingToCharger,
                steal_timer: steal_duration,
                move_target: charger_target,
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

        robbery_tracker.robbery_triggered_today = true;

        info!(
            "\"{}\" ({:?}) spawned at ({:.0},{:.0}), targeting charger {} at ({:.0},{:.0})",
            name, variant, spawn_pos.x, spawn_pos.y, charger_id, charger_target.x, charger_target.y,
        );

        break;
    }
}

// ─────────────────────────────────────────────────────
//  robber_movement_system — direct walk (no pathfinding)
// ─────────────────────────────────────────────────────

/// Move robbers in a straight line toward their `move_target`.
/// Applies a walking animation: vertical bobbing + horizontal flip + slight sway,
/// inspired by the security camera swivel animation.
pub fn robber_movement_system(
    time: Res<Time>,
    game_clock: Res<GameClock>,
    mut query: Query<(&mut Robber, &mut Transform)>,
) {
    if game_clock.is_paused() {
        return;
    }

    let dt = time.delta_secs();
    let speed_multiplier = game_clock.speed.visual_multiplier();

    // Walk animation parameters (similar approach to SecurityCameraHead swivel)
    let bob_speed = 10.0; // oscillation frequency (rad/s)
    let bob_amplitude = 2.5; // pixels of vertical bounce
    let sway_amplitude = 0.06; // radians (~3.4°) of body tilt

    for (mut robber, mut transform) in query.iter_mut() {
        // Only move during walking / fleeing phases
        if robber.phase != RobberPhase::WalkingToCharger && robber.phase != RobberPhase::Fleeing {
            // Reset tilt when not walking (e.g. during Stealing phase)
            transform.rotation = Quat::IDENTITY;
            continue;
        }

        let current = transform.translation.truncate();
        let target = robber.move_target;
        let distance = current.distance(target);

        if distance < ARRIVAL_THRESHOLD {
            // Snap to target
            transform.translation.x = target.x;
            transform.translation.y = target.y;
            // Reset animation when arrived
            transform.rotation = Quat::IDENTITY;
            continue;
        }

        // Advance animation timer
        robber.anim_timer += dt;

        let speed = ROBBER_SPEED * speed_multiplier;
        let step = speed * dt;
        let direction = (target - current).normalize();

        // Move toward target
        transform.translation.x += direction.x * step;
        transform.translation.y += direction.y * step;

        // Update base_y to track the linear path (without bob offset)
        robber.base_y = transform.translation.y;

        // Walking bob: sinusoidal bounce using abs(sin) for a "step" feel
        let bob_offset = bevy::math::ops::sin(robber.anim_timer * bob_speed).abs() * bob_amplitude;
        transform.translation.y += bob_offset;

        // Body sway: slight tilt left/right synchronized to steps
        let sway = bevy::math::ops::sin(robber.anim_timer * bob_speed) * sway_amplitude;
        transform.rotation = Quat::from_rotation_z(sway);

        // Flip sprite to face movement direction (negative scale.x = face left)
        let abs_scale = transform.scale.x.abs();
        if direction.x < -0.1 {
            transform.scale.x = -abs_scale; // face left
        } else if direction.x > 0.1 {
            transform.scale.x = abs_scale; // face right
        }
    }
}

// ─────────────────────────────────────────────────────
//  robber_arrival_system
// ─────────────────────────────────────────────────────

/// Detect when robber reaches the charger and start stealing.
pub fn robber_arrival_system(
    mut commands: Commands,
    mut robbers: Query<(Entity, &mut Robber, &Transform)>,
    chargers: Query<&GlobalTransform, (With<Charger>, Without<Robber>)>,
    image_assets: Res<ImageAssets>,
    images: Res<Assets<Image>>,
    mut sfx_writer: MessageWriter<PlaySfx>,
) {
    for (entity, mut robber, robber_transform) in robbers.iter_mut() {
        if robber.phase != RobberPhase::WalkingToCharger {
            continue;
        }

        // Check distance to target
        let current = robber_transform.translation.truncate();
        if current.distance(robber.move_target) > ARRIVAL_THRESHOLD {
            continue;
        }

        // Transition to Stealing phase
        robber.phase = RobberPhase::Stealing;

        // Switch to stealing sprite (variant-specific)
        let stealing_image = match robber.variant {
            RobberVariant::Black => image_assets.character_robber_stealing.clone(),
            RobberVariant::Pink => image_assets.character_robber_stealing_pink.clone(),
        };
        commands
            .entity(entity)
            .insert(Sprite::from_image(stealing_image));

        // Spawn TheftAlarmVfx + StealingSparkVfx on the charger
        if let Ok(charger_gt) = chargers.get(robber.target_charger) {
            let pos = charger_gt.translation();

            // Red alarm flash
            let vfx_image = image_assets.vfx_light_pulse_red.clone();
            let vfx_size = crate::resources::sprite_metadata::vfx_world_size(
                crate::resources::VfxType::UrgentPulse,
            );
            let vfx_scale = if let Some(image) = images.get(&vfx_image) {
                vfx_size.scale_for_image(image)
            } else {
                1.0
            };

            commands.spawn((
                Sprite::from_image(vfx_image),
                Transform::from_xyz(pos.x, pos.y, 11.0).with_scale(Vec3::splat(vfx_scale)),
                TheftAlarmVfx {
                    charger_entity: robber.target_charger,
                    flash_time: 0.0,
                },
            ));

            // Yellow spark VFX on top of charger
            let spark_image = image_assets.vfx_light_pulse_yellow.clone();
            commands.spawn((
                Sprite::from_image(spark_image),
                Transform::from_xyz(pos.x, pos.y + 20.0, 12.0).with_scale(Vec3::splat(0.8)),
                StealingSparkVfx {
                    charger_entity: robber.target_charger,
                    spark_time: 0.0,
                },
            ));
        }

        // Play alarm sound (platform-abstracted via PlaySfx event)
        sfx_writer.write(PlaySfx(SfxKind::AlarmTheft));

        info!(
            "\"{}\" arrived at charger, cutting cable (stealing for {:.0}s)",
            robber.name, robber.steal_timer
        );
    }
}

// ─────────────────────────────────────────────────────
//  robber_stealing_system
// ─────────────────────────────────────────────────────

/// Count down the steal timer and inject CableTheft fault when done.
pub fn robber_stealing_system(
    mut commands: Commands,
    mut robbers: Query<(Entity, &mut Robber, &BelongsToSite)>,
    mut chargers: Query<&mut Charger>,
    game_clock: Res<GameClock>,
    time: Res<Time>,
    image_assets: Res<ImageAssets>,
    images: Res<Assets<Image>>,
    alarm_vfx: Query<(Entity, &TheftAlarmVfx)>,
    spark_vfx: Query<(Entity, &StealingSparkVfx)>,
    multi_site: Res<MultiSiteManager>,
    security_cameras: Query<(&GlobalTransform, &BelongsToSite), With<SecuritySystemVisual>>,
) {
    if game_clock.is_paused() {
        return;
    }

    let delta = time.delta_secs() * game_clock.speed.multiplier();

    let mut rng = rand::rng();

    for (entity, mut robber, belongs) in robbers.iter_mut() {
        if robber.phase != RobberPhase::Stealing {
            continue;
        }

        robber.steal_timer -= delta;

        if robber.steal_timer > 0.0 {
            continue;
        }

        // Stealing complete — check if anti-theft measures prevent success
        let mut theft_succeeded = false;
        if let Ok(mut charger) = chargers.get_mut(robber.target_charger) {
            let has_cable = charger.anti_theft_cable;
            let has_security = multi_site
                .get_site(belongs.site_id)
                .map(|s| s.grid.has_security_system())
                .unwrap_or(false);

            let success_chance = match (has_cable, has_security) {
                (true, true) => 0.10,  // Both: 10% success
                (true, false) => 0.40, // Cable only: 40% success
                (false, true) => 0.60, // Security only: 60% success
                (false, false) => 1.0, // No protection: 100% success
            };

            theft_succeeded = rng.random::<f32>() < success_chance;

            if theft_succeeded {
                charger.current_fault = Some(FaultType::CableTheft);
                charger.fault_occurred_at = Some(game_clock.total_game_time);
                charger.fault_detected_at = None;
                charger.fault_is_detected = false;
                charger.degrade_reliability_fault(&FaultType::CableTheft);
                charger.is_charging = false;
                charger.current_power_kw = 0.0;
                charger.requested_power_kw = 0.0;
                charger.allocated_power_kw = 0.0;
                charger.session_start_game_time = None;

                info!(
                    "\"{}\" stole cable from charger {}! $2k replacement needed.",
                    robber.name, charger.id
                );
            } else {
                info!(
                    "\"{}\" failed to steal cable from charger {} — anti-theft cable held!",
                    robber.name, charger.id
                );
            }
        }

        // Despawn alarm + spark VFX
        for (vfx_entity, vfx) in alarm_vfx.iter() {
            if vfx.charger_entity == robber.target_charger {
                commands.entity(vfx_entity).try_despawn();
            }
        }
        for (vfx_entity, vfx) in spark_vfx.iter() {
            if vfx.charger_entity == robber.target_charger {
                commands.entity(vfx_entity).try_despawn();
            }
        }

        // Transition to fleeing — reset walk animation
        robber.phase = RobberPhase::Fleeing;
        robber.anim_timer = 0.0;

        // Pick a random exit edge (different direction!)
        let (gw, gh) = multi_site
            .get_site(belongs.site_id)
            .map(|s| (s.grid.width, s.grid.height))
            .unwrap_or((GRID_WIDTH, GRID_HEIGHT));
        let exit_pos = random_edge_position(&mut rng, gw, gh);
        robber.move_target = exit_pos;

        // Switch back to walking sprite
        let walking_image = match robber.variant {
            RobberVariant::Black => image_assets.character_robber_walking.clone(),
            RobberVariant::Pink => image_assets.character_robber_walking_pink.clone(),
        };
        commands
            .entity(entity)
            .insert(Sprite::from_image(walking_image));

        if theft_succeeded {
            // Spawn stolen cable sprite (world-space, follows robber via update system)
            let cable_scale = if let Some(cable_img) = images.get(&image_assets.stolen_cable) {
                let w = cable_img.size().x as f32;
                18.0 / w // ~18 world units wide
            } else {
                0.01
            };
            commands.spawn((
                Sprite::from_image(image_assets.stolen_cable.clone()),
                Transform::from_xyz(0.0, 0.0, 14.5).with_scale(Vec3::splat(cable_scale)),
                GlobalTransform::default(),
                StolenCableSprite {
                    robber_entity: entity,
                },
            ));

            // Success speech bubble: "HAHA MY CABLE" in green
            commands.spawn((
                Text2d::new("HAHA MY CABLE"),
                TextFont {
                    font_size: 18.0,
                    ..default()
                },
                TextColor(Color::srgb(0.1, 1.0, 0.1)),
                Transform::from_xyz(0.0, 48.0, 20.0),
                GlobalTransform::default(),
                RobberLootBubble {
                    robber_entity: entity,
                    lifetime: 0.0,
                },
            ));
        } else {
            // Failure speech bubble: "DARN" in red
            commands.spawn((
                Text2d::new("DARN"),
                TextFont {
                    font_size: 22.0,
                    ..default()
                },
                TextColor(Color::srgb(1.0, 0.1, 0.1)),
                Transform::from_xyz(0.0, 48.0, 20.0),
                GlobalTransform::default(),
                RobberLootBubble {
                    robber_entity: entity,
                    lifetime: 0.0,
                },
            ));

            // If security system helped deter, flash an alert near the camera
            let has_security = multi_site
                .get_site(belongs.site_id)
                .map(|s| s.grid.has_security_system())
                .unwrap_or(false);
            if has_security
                && let Some((cam_gt, _)) = security_cameras
                    .iter()
                    .find(|(_, cam_site)| cam_site.site_id == belongs.site_id)
            {
                let cam_pos = cam_gt.translation();
                commands.spawn((
                    Text2d::new("SECURITY ALERT"),
                    TextFont {
                        font_size: 16.0,
                        ..default()
                    },
                    TextColor(Color::srgba(0.2, 1.0, 0.4, 1.0)),
                    Transform::from_xyz(cam_pos.x, cam_pos.y + 40.0, 20.0),
                    GlobalTransform::default(),
                    SecurityAlertBubble {
                        lifetime: 0.0,
                        max_lifetime: 2.5,
                    },
                ));
            }
        }

        info!(
            "\"{}\" fleeing to ({:.0},{:.0})",
            robber.name, exit_pos.x, exit_pos.y
        );
    }
}

// ─────────────────────────────────────────────────────
//  robber_flee_arrival_system
// ─────────────────────────────────────────────────────

/// Detect when a fleeing robber reaches the exit edge and mark for cleanup.
pub fn robber_flee_arrival_system(mut robbers: Query<(&Transform, &mut Robber)>) {
    for (transform, mut robber) in robbers.iter_mut() {
        if robber.phase != RobberPhase::Fleeing {
            continue;
        }

        let current = transform.translation.truncate();
        if current.distance(robber.move_target) <= ARRIVAL_THRESHOLD {
            robber.phase = RobberPhase::Gone;
            info!("Robber reached exit edge, ready for cleanup");
        }
    }
}

// ─────────────────────────────────────────────────────
//  robber_cleanup_system
// ─────────────────────────────────────────────────────

/// Despawn robber entities that have finished fleeing.
pub fn robber_cleanup_system(mut commands: Commands, robbers: Query<(Entity, &Robber)>) {
    for (entity, robber) in robbers.iter() {
        if robber.phase == RobberPhase::Gone {
            commands.entity(entity).try_despawn();
            info!("Robber entity cleaned up");
        }
    }
}
