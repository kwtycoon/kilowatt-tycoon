//! Scenario scripted event system
//!
//! Processes `DriverSchedule.scripted_events` entries as game time advances.
//! Each event fires once at its designated time, dispatching to the appropriate
//! subsystem (charger faults, transformer warnings, monsoon floods, grid
//! brownouts, battery-swap competitor pressure, demand surges).

use bevy::prelude::*;
use rand::Rng;

use crate::components::charger::{Charger, FaultType};
use crate::resources::{GameClock, GameState, GridEventType, ImageAssets, MultiSiteManager};
use crate::ui::toast::{RealTimeToast, ToastContainer, ToastNotification};

/// Duration (game-seconds) of a monsoon flood capacity reduction.
const MONSOON_FLOOD_DURATION_SECS: f32 = 1800.0;
/// Fraction of grid capacity lost during a monsoon flood (0.0–1.0).
const MONSOON_FLOOD_CAPACITY_PENALTY: f32 = 0.30;

/// Duration (game-seconds) of a battery-swap competitor demand penalty.
const BATTERY_SWAP_DURATION_SECS: f32 = 3600.0;
/// Demand multiplier applied while a swap competitor is active.
const BATTERY_SWAP_DEMAND_MULTIPLIER: f32 = 0.65;

/// Duration (game-seconds) of a residential-ban demand surge.
const DEMAND_SURGE_DURATION_SECS: f32 = 2400.0;
/// Demand multiplier applied during a residential-ban surge.
const DEMAND_SURGE_MULTIPLIER: f32 = 1.6;

/// Processes scenario-level scripted events from the active site's driver schedule.
pub fn scenario_event_system(
    mut chargers: Query<&mut Charger>,
    mut multi_site: ResMut<MultiSiteManager>,
    game_clock: Res<GameClock>,
    time: Res<Time>,
    mut game_state: ResMut<GameState>,
    image_assets: Res<ImageAssets>,
    mut commands: Commands,
    toast_containers: Query<Entity, With<ToastContainer>>,
) {
    if game_clock.is_paused() {
        return;
    }

    let Some(viewed_id) = multi_site.viewed_site_id else {
        return;
    };
    let Some(site) = multi_site.owned_sites.get_mut(&viewed_id) else {
        return;
    };

    let delta = time.delta_secs() * game_clock.speed.multiplier();
    tick_scenario_effects(site, delta);

    let game_time = game_clock.game_time;

    // Collect events that are ready to fire (avoids double-borrow on multi_site).
    let mut pending: Vec<crate::resources::site_config::ScriptedEvent> = Vec::new();
    {
        let schedule = &site.driver_schedule;
        let mut idx = schedule.next_event_index;
        while idx < schedule.scripted_events.len() {
            let ev = &schedule.scripted_events[idx];
            if ev.time > game_time {
                break;
            }
            pending.push(ev.clone());
            idx += 1;
        }
        site.driver_schedule.next_event_index = idx;
    }

    for event in &pending {
        let notes = &event.notes;

        match event.event_type.as_str() {
            "charger_fault" => {
                if let Some(fault_type) = event.fault_type {
                    inject_fault_on_random_charger(&mut chargers, fault_type, game_time);
                    info!("Scenario event: charger_fault — {notes}");
                }
            }
            "transformer_warning" => {
                if let Some(temp) = event.temp_threshold {
                    let throttle = if temp >= 90.0 {
                        0.25
                    } else if temp >= 75.0 {
                        0.5
                    } else {
                        1.0
                    };
                    site.thermal_throttle_factor = site.thermal_throttle_factor.min(throttle);
                    info!(
                        "Scenario event: transformer_warning at {temp}°C (throttle={throttle}) — {notes}"
                    );
                }
            }
            "monsoon_flood" => {
                let severity = event.severity.unwrap_or(2).clamp(1, 3);
                let count = match severity {
                    1 => 2,
                    2 => 4,
                    _ => 6,
                };
                let faulted = inject_faults_on_n_chargers(
                    &mut chargers,
                    FaultType::GroundFault,
                    count,
                    game_time,
                );
                site.scenario_effects.monsoon_flood_remaining_secs = MONSOON_FLOOD_DURATION_SECS;
                site.scenario_effects.monsoon_flood_capacity_penalty =
                    MONSOON_FLOOD_CAPACITY_PENALTY;

                if !game_state.first_fault_seen {
                    game_state.first_fault_seen = true;
                }

                spawn_scenario_toast(
                    &mut commands,
                    &toast_containers,
                    &image_assets,
                    &format!(
                        "Monsoon flooding! Ground faults on {faulted} chargers. Capacity reduced 30%."
                    ),
                    game_time,
                    Color::srgba(0.2, 0.3, 0.7, 0.95),
                );
                info!(
                    "Scenario event: monsoon_flood severity {severity} — {faulted} chargers faulted — {notes}"
                );
            }
            "grid_brownout" => {
                site.grid_events.active_event = Some(GridEventType::Brownout);
                site.grid_events.event_end_time = game_time + 2.0 * 3600.0;

                spawn_scenario_toast(
                    &mut commands,
                    &toast_containers,
                    &image_assets,
                    "Grid brownout! Capacity reduced 40%, import prices spiking.",
                    game_time,
                    Color::srgba(0.6, 0.2, 0.1, 0.95),
                );
                info!("Scenario event: grid_brownout — {notes}");
            }
            "battery_swap_competitor" => {
                site.scenario_effects.battery_swap_remaining_secs = BATTERY_SWAP_DURATION_SECS;
                site.scenario_effects.battery_swap_demand_multiplier =
                    BATTERY_SWAP_DEMAND_MULTIPLIER;

                spawn_scenario_toast(
                    &mut commands,
                    &toast_containers,
                    &image_assets,
                    "A battery swap station opened nearby! Impatient riders are switching.",
                    game_time,
                    Color::srgba(0.7, 0.5, 0.1, 0.95),
                );
                info!("Scenario event: battery_swap_competitor — {notes}");
            }
            "demand_surge" => {
                site.scenario_effects.demand_surge_remaining_secs = DEMAND_SURGE_DURATION_SECS;
                site.scenario_effects.demand_surge_multiplier = DEMAND_SURGE_MULTIPLIER;

                spawn_scenario_toast(
                    &mut commands,
                    &toast_containers,
                    &image_assets,
                    "Apartment charging ban! Displaced riders flooding the station.",
                    game_time,
                    Color::srgba(0.1, 0.5, 0.6, 0.95),
                );
                info!("Scenario event: demand_surge — {notes}");
            }
            other => {
                warn!("Unknown scenario event type: {other}");
            }
        }
    }
}

fn tick_scenario_effects(site: &mut crate::resources::multi_site::SiteState, delta: f32) {
    let fx = &mut site.scenario_effects;

    if fx.monsoon_flood_remaining_secs > 0.0 {
        fx.monsoon_flood_remaining_secs = (fx.monsoon_flood_remaining_secs - delta).max(0.0);
        if fx.monsoon_flood_remaining_secs <= 0.0 {
            fx.monsoon_flood_capacity_penalty = 0.0;
        }
    }

    if fx.battery_swap_remaining_secs > 0.0 {
        fx.battery_swap_remaining_secs = (fx.battery_swap_remaining_secs - delta).max(0.0);
        if fx.battery_swap_remaining_secs <= 0.0 {
            fx.battery_swap_demand_multiplier = 1.0;
        }
    }

    if fx.demand_surge_remaining_secs > 0.0 {
        fx.demand_surge_remaining_secs = (fx.demand_surge_remaining_secs - delta).max(0.0);
        if fx.demand_surge_remaining_secs <= 0.0 {
            fx.demand_surge_multiplier = 1.0;
        }
    }
}

fn inject_fault_on_random_charger(
    chargers: &mut Query<&mut Charger>,
    fault_type: FaultType,
    game_time: f32,
) {
    let healthy_count = chargers
        .iter()
        .filter(|c| c.current_fault.is_none())
        .count();
    if healthy_count == 0 {
        return;
    }
    let mut rng = rand::rng();
    let pick = rng.random_range(0..healthy_count);
    let mut seen = 0;
    for mut charger in chargers.iter_mut() {
        if charger.current_fault.is_none() {
            if seen == pick {
                inject_fault(&mut charger, fault_type, game_time);
                return;
            }
            seen += 1;
        }
    }
}

fn inject_faults_on_n_chargers(
    chargers: &mut Query<&mut Charger>,
    fault_type: FaultType,
    count: usize,
    game_time: f32,
) -> usize {
    let mut healthy_indices: Vec<usize> = Vec::new();
    for (i, charger) in chargers.iter().enumerate() {
        if charger.current_fault.is_none() {
            healthy_indices.push(i);
        }
    }

    let mut rng = rand::rng();
    let n = count.min(healthy_indices.len());
    for i in 0..n {
        let j = rng.random_range(i..healthy_indices.len());
        healthy_indices.swap(i, j);
    }
    let targets: Vec<usize> = healthy_indices[..n].to_vec();

    let mut faulted = 0;
    for (i, mut charger) in chargers.iter_mut().enumerate() {
        if targets.contains(&i) {
            inject_fault(&mut charger, fault_type, game_time);
            faulted += 1;
        }
    }
    faulted
}

fn inject_fault(charger: &mut Charger, fault_type: FaultType, game_time: f32) {
    charger.current_fault = Some(fault_type);
    charger.fault_occurred_at = Some(game_time);
    charger.fault_detected_at = None;
    charger.fault_is_detected = false;
    charger.fault_discovered = false;
    charger.reboot_attempts = 0;
    charger.degrade_reliability_fault(&fault_type);
    charger.is_charging = false;
    charger.current_power_kw = 0.0;
    charger.requested_power_kw = 0.0;
    charger.allocated_power_kw = 0.0;
    charger.session_start_game_time = None;
}

fn spawn_scenario_toast(
    commands: &mut Commands,
    toast_containers: &Query<Entity, With<ToastContainer>>,
    image_assets: &ImageAssets,
    message: &str,
    game_time: f32,
    bg_color: Color,
) {
    let Ok(container) = toast_containers.single() else {
        return;
    };
    let icon = image_assets.icon_warning.clone();
    let entity = commands
        .spawn((
            Node {
                width: Val::Px(300.0),
                padding: UiRect::all(Val::Px(15.0)),
                flex_direction: FlexDirection::Row,
                column_gap: Val::Px(10.0),
                align_items: AlignItems::Center,
                ..default()
            },
            BackgroundColor(bg_color),
            BorderRadius::all(Val::Px(8.0)),
            ToastNotification {
                created_at: game_time,
                duration: 80.0,
            },
            RealTimeToast {
                created_at_real: 0.0,
                duration_real: 8.0,
            },
        ))
        .with_children(|parent| {
            parent.spawn((
                ImageNode::new(icon),
                Node {
                    width: Val::Px(24.0),
                    height: Val::Px(24.0),
                    ..default()
                },
            ));
            parent.spawn((
                Text::new(message.to_string()),
                TextFont {
                    font_size: 14.0,
                    ..default()
                },
                TextColor(Color::srgb(1.0, 1.0, 1.0)),
            ));
        })
        .id();
    commands.entity(container).add_child(entity);
}
