//! Debug overlay for displaying runtime information.
//!
//! Toggle with F3 key to show FPS, entity counts, and game state.
//! Press F4 to print detailed traffic/pathfinding debug to stdout and `/tmp/kwtycoon-*.log`.
//!
//! # Usage
//!
//! The debug overlay is automatically available when the `HelpersPlugin` is added.
//! Press F3 to toggle visibility.

use std::fmt::Write as FmtWrite;

use bevy::diagnostic::{DiagnosticsStore, FrameTimeDiagnosticsPlugin};
use bevy::prelude::*;
use bevy_northstar::prelude::*;

use crate::components::BelongsToSite;
use crate::components::driver::{Driver, DriverState, VehicleMovement};
use crate::components::technician::{Technician, TechnicianMovement};
use crate::resources::{GameClock, GameState, MultiSiteManager, TileContent};
use crate::states::AppState;
use crate::systems::ambient_traffic::AmbientVehicle;
use crate::systems::northstar_movement::RerouteCooldown;
use crate::systems::site_roots::ActivePathfindingGrid;

/// Plugin for debug overlay functionality
pub struct DebugOverlayPlugin;

impl Plugin for DebugOverlayPlugin {
    fn build(&self, app: &mut App) {
        // Add frame time diagnostics if not already present
        if !app.is_plugin_added::<FrameTimeDiagnosticsPlugin>() {
            app.add_plugins(FrameTimeDiagnosticsPlugin::default());
        }

        app.init_resource::<DebugOverlayState>()
            .add_systems(Startup, setup_debug_overlay)
            .add_systems(
                Update,
                (
                    toggle_debug_overlay,
                    update_debug_overlay,
                    sync_debug_grid_visibility,
                    sync_debug_paths,
                    print_traffic_debug,
                ),
            );
    }
}

/// Resource to track debug overlay visibility
#[derive(Resource, Default)]
pub struct DebugOverlayState {
    pub visible: bool,
}

/// Marker component for debug overlay root
#[derive(Component)]
pub struct DebugOverlay;

/// Marker for FPS text
#[derive(Component)]
pub struct DebugFpsText;

/// Marker for game state text
#[derive(Component)]
pub struct DebugGameStateText;

/// Marker for entity count text
#[derive(Component)]
pub struct DebugEntityCountText;

/// Setup the debug overlay UI
fn setup_debug_overlay(mut commands: Commands) {
    commands
        .spawn((
            DebugOverlay,
            Node {
                position_type: PositionType::Absolute,
                left: Val::Px(10.0),
                top: Val::Px(10.0),
                flex_direction: FlexDirection::Column,
                row_gap: Val::Px(4.0),
                padding: UiRect::all(Val::Px(8.0)),
                ..default()
            },
            BackgroundColor(Color::srgba(0.0, 0.0, 0.0, 0.7)),
            BorderRadius::all(Val::Px(4.0)),
            Visibility::Hidden,
            GlobalZIndex(2000),
        ))
        .with_children(|parent| {
            // Title
            parent.spawn((
                Text::new("Debug Overlay (F3)"),
                TextFont {
                    font_size: 14.0,
                    ..default()
                },
                TextColor(Color::srgb(0.8, 0.8, 0.2)),
            ));

            // FPS
            parent.spawn((
                DebugFpsText,
                Text::new("FPS: --"),
                TextFont {
                    font_size: 12.0,
                    ..default()
                },
                TextColor(Color::WHITE),
            ));

            // App state
            parent.spawn((
                DebugGameStateText,
                Text::new("State: --"),
                TextFont {
                    font_size: 12.0,
                    ..default()
                },
                TextColor(Color::WHITE),
            ));

            // Entity count
            parent.spawn((
                DebugEntityCountText,
                Text::new("Entities: --"),
                TextFont {
                    font_size: 12.0,
                    ..default()
                },
                TextColor(Color::WHITE),
            ));
        });
}

/// Toggle debug overlay with F3
fn toggle_debug_overlay(
    keyboard: Res<ButtonInput<KeyCode>>,
    mut state: ResMut<DebugOverlayState>,
    mut query: Query<&mut Visibility, With<DebugOverlay>>,
) {
    if keyboard.just_pressed(KeyCode::F3) {
        state.visible = !state.visible;

        for mut visibility in &mut query {
            *visibility = if state.visible {
                Visibility::Visible
            } else {
                Visibility::Hidden
            };
        }

        info!(
            "Debug overlay: {}",
            if state.visible { "ON" } else { "OFF" }
        );
    }
}

/// Update debug overlay information
fn update_debug_overlay(
    state: Res<DebugOverlayState>,
    diagnostics: Res<DiagnosticsStore>,
    app_state: Res<State<AppState>>,
    game_clock: Option<Res<GameClock>>,
    game_state: Option<Res<GameState>>,
    entity_query: Query<Entity>,
    mut fps_query: Query<
        &mut Text,
        (
            With<DebugFpsText>,
            Without<DebugGameStateText>,
            Without<DebugEntityCountText>,
        ),
    >,
    mut state_query: Query<
        &mut Text,
        (
            With<DebugGameStateText>,
            Without<DebugFpsText>,
            Without<DebugEntityCountText>,
        ),
    >,
    mut entity_count_query: Query<
        &mut Text,
        (
            With<DebugEntityCountText>,
            Without<DebugFpsText>,
            Without<DebugGameStateText>,
        ),
    >,
) {
    if !state.visible {
        return;
    }

    // Update FPS
    if let Some(fps) = diagnostics.get(&bevy::diagnostic::FrameTimeDiagnosticsPlugin::FPS)
        && let Some(value) = fps.smoothed()
    {
        for mut text in &mut fps_query {
            *text = Text::new(format!("FPS: {value:.0}"));
        }
    }

    // Update game state info
    let game_time = game_clock.as_ref().map(|c| c.game_time).unwrap_or(0.0);
    let speed = game_clock
        .as_ref()
        .map(|c| c.speed.multiplier())
        .unwrap_or(1.0);
    let cash = game_state.as_ref().map(|s| s.cash).unwrap_or(0.0);
    let rep = game_state.as_ref().map(|s| s.reputation).unwrap_or(0);

    for mut text in &mut state_query {
        *text = Text::new(format!(
            "State: {:?}\nTime: {:.0}s | Speed: {}x\nCash: ${:.0} | Rep: {}",
            app_state.get(),
            game_time,
            speed,
            cash,
            rep
        ));
    }

    // Update entity count
    let entity_count = entity_query.iter().count();
    for mut text in &mut entity_count_query {
        *text = Text::new(format!("Entities: {entity_count}"));
    }
}

fn sync_debug_grid_visibility(
    state: Res<DebugOverlayState>,
    mut debug_grids: Query<&mut DebugGrid>,
) {
    let draw_enabled = state.visible;

    for mut grid in &mut debug_grids {
        grid.draw_cells = draw_enabled;
        grid.draw_entrances = draw_enabled;
        grid.draw_chunks = draw_enabled;
        grid.draw_cached_paths = false;
    }
}

fn sync_debug_paths(
    state: Res<DebugOverlayState>,
    grid_entity: Query<Entity, With<CardinalGrid>>,
    mut commands: Commands,
    drivers: Query<Entity, With<Driver>>,
    technicians: Query<Entity, With<Technician>>,
) {
    let draw_enabled = state.visible;

    if !draw_enabled {
        for entity in &drivers {
            commands
                .entity(entity)
                .try_remove::<DebugPath>()
                .try_remove::<AgentOfGrid>();
        }
        for entity in &technicians {
            commands
                .entity(entity)
                .try_remove::<DebugPath>()
                .try_remove::<AgentOfGrid>();
        }
        return;
    }

    let Ok(grid_entity) = grid_entity.single() else {
        return;
    };

    for entity in &drivers {
        commands.entity(entity).try_insert((
            DebugPath::new(Color::srgb(0.9, 0.2, 0.2)),
            AgentOfGrid(grid_entity),
        ));
    }

    for entity in &technicians {
        commands.entity(entity).try_insert((
            DebugPath::new(Color::srgb(0.2, 0.6, 0.9)),
            AgentOfGrid(grid_entity),
        ));
    }
}

/// Print detailed traffic/pathfinding state to stdout and log file when F4 is pressed.
/// Includes bevy_northstar pathfinding info: AgentPos, NextPos, Path, failure markers.
/// Writes output to `/tmp/kwtycoon-{timestamp}.log`.
#[allow(clippy::type_complexity)]
fn print_traffic_debug(
    keyboard: Res<ButtonInput<KeyCode>>,
    multi_site: Res<MultiSiteManager>,
    active_grid: Res<ActivePathfindingGrid>,
    game_clock: Res<GameClock>,
    blocking_map: Res<BlockingMap>,
    drivers: Query<(
        Entity,
        &Driver,
        &VehicleMovement,
        &BelongsToSite,
        Option<&AgentPos>,
        Option<&NextPos>,
        Option<&Path>,
        Option<&Pathfind>,
        Option<&PathfindingFailed>,
        Option<&RerouteFailed>,
        Option<&RerouteCooldown>,
        Option<&Blocking>,
    )>,
    ambient: Query<(
        Entity,
        &AmbientVehicle,
        &VehicleMovement,
        &BelongsToSite,
        Option<&AgentPos>,
        Option<&NextPos>,
        Option<&Path>,
        Option<&Pathfind>,
        Option<&PathfindingFailed>,
    )>,
    technicians: Query<(
        Entity,
        &Technician,
        &TechnicianMovement,
        &BelongsToSite,
        Option<&AgentPos>,
        Option<&NextPos>,
        Option<&Path>,
        Option<&Pathfind>,
        Option<&PathfindingFailed>,
        Option<&RerouteFailed>,
    )>,
) {
    if !keyboard.just_pressed(KeyCode::F4) {
        return;
    }

    // Build output as a string
    let mut out = String::with_capacity(4096);

    let _ = writeln!(out, "\n========== TRAFFIC DEBUG (F4) ==========\n");

    let _ = writeln!(
        out,
        "Pause: {} | Viewed site: {:?} | Active grid: {:?}",
        if game_clock.is_paused() { "yes" } else { "no" },
        multi_site.viewed_site_id,
        active_grid.site_id
    );
    let _ = writeln!(out);

    // Print BlockingMap
    let _ = writeln!(
        out,
        "=== BlockingMap ({} tiles blocked) ===",
        blocking_map.0.len()
    );
    if blocking_map.0.is_empty() {
        let _ = writeln!(out, "  (empty)");
    } else {
        let mut entries: Vec<_> = blocking_map.0.iter().collect();
        entries.sort_by_key(|(pos, _)| (pos.y, pos.x));
        for (pos, entity) in entries {
            let _ = writeln!(out, "  ({},{}) <- Entity {}", pos.x, pos.y, entity.index());
        }
    }
    let _ = writeln!(out);

    // Print info for each site
    for (site_id, site_state) in multi_site.owned_sites.iter() {
        let _ = writeln!(out, "=== Site {site_id:?} ===");
        let grid = &site_state.grid;

        let _ = writeln!(
            out,
            "Layout: entry=({},{}) exit=({},{}) max_vehicles={}",
            grid.entry_pos.0,
            grid.entry_pos.1,
            grid.exit_pos.0,
            grid.exit_pos.1,
            site_state.max_vehicles
        );

        let entry_content = grid.get_content(grid.entry_pos.0, grid.entry_pos.1);
        let exit_content = grid.get_content(grid.exit_pos.0, grid.exit_pos.1);
        let entry_driveable = entry_content.is_driveable() || entry_content.is_parking();
        let exit_driveable = exit_content.is_driveable() || exit_content.is_parking();
        let _ = writeln!(
            out,
            "Tiles: entry={:?} driveable={} | exit={:?} driveable={}",
            entry_content, entry_driveable, exit_content, exit_driveable
        );

        // Count drivers at this site
        let site_drivers: Vec<_> = drivers
            .iter()
            .filter(|(_, _, _, belongs, ..)| belongs.site_id == *site_id)
            .collect();

        let site_ambient: Vec<_> = ambient
            .iter()
            .filter(|(_, _, _, belongs, ..)| belongs.site_id == *site_id)
            .collect();

        let site_techs: Vec<_> = technicians
            .iter()
            .filter(|(_, _, _, belongs, ..)| belongs.site_id == *site_id)
            .collect();

        let _ = writeln!(
            out,
            "Vehicles: {} drivers, {} ambient, {} techs\n",
            site_drivers.len(),
            site_ambient.len(),
            site_techs.len()
        );

        // Collect stuck vehicles for summary
        let mut stuck_vehicles = Vec::new();

        // Print driver details
        let _ = writeln!(out, "--- Drivers ---");
        if site_drivers.is_empty() {
            let _ = writeln!(out, "  (none)");
        }

        for (
            entity,
            driver,
            movement,
            _belongs,
            agent_pos,
            next_pos,
            path,
            pathfind,
            pathfind_failed,
            reroute_failed,
            cooldown,
            blocking,
        ) in &site_drivers
        {
            // Determine destination
            let destination = match driver.state {
                DriverState::Arriving => {
                    if let Some((bx, by)) = driver.assigned_bay {
                        format!("bay({bx},{by})")
                    } else if let Some((wx, wy)) = driver.waiting_tile {
                        format!("wait({wx},{wy})")
                    } else {
                        "bay(?)".to_string()
                    }
                }
                DriverState::Leaving | DriverState::LeftAngry | DriverState::Complete => {
                    format!("exit({},{})", grid.exit_pos.0, grid.exit_pos.1)
                }
                DriverState::Queued => {
                    if let Some((wx, wy)) = driver.waiting_tile {
                        format!("queued@wait({wx},{wy})")
                    } else if let Some((bx, by)) = driver.assigned_bay {
                        format!("queued@bay({bx},{by})")
                    } else {
                        "queued".to_string()
                    }
                }
                _ => "parked".to_string(),
            };

            // Format state
            let state_str = format!("{:?}", driver.state).to_uppercase();
            let phase_str = format!("{:?}", movement.phase).to_uppercase();

            let _ = writeln!(
                out,
                "Entity {:3}: '{}' [{} -> {}]",
                entity.index(),
                driver.id,
                phase_str,
                destination
            );

            // Grid vs world position
            let world_pos = movement.current_position().unwrap_or_default();
            if let Some(agent) = agent_pos {
                let _ = writeln!(
                    out,
                    "  grid=({},{}) world=({:.0},{:.0})",
                    agent.0.x, agent.0.y, world_pos.x, world_pos.y
                );
            } else {
                let _ = writeln!(
                    out,
                    "  grid=NONE world=({:.0},{:.0})",
                    world_pos.x, world_pos.y
                );
            }

            // Path and next position
            let path_str = if let Some(p) = path {
                format!("{} steps", p.len())
            } else {
                "NONE".to_string()
            };

            let next_str = if let Some(n) = next_pos {
                format!("({},{})", n.0.x, n.0.y)
            } else {
                "NONE".to_string()
            };

            let pathfind_str = if let Some(pf) = pathfind {
                format!("goal({},{})", pf.goal.x, pf.goal.y)
            } else {
                "NONE".to_string()
            };

            let _ = writeln!(
                out,
                "  path: {path_str} | next: {next_str} | pathfind: {pathfind_str}"
            );

            // Status flags
            let mut status_parts = Vec::new();

            if pathfind_failed.is_some() {
                status_parts.push("PathfindingFailed".to_string());
            }
            if reroute_failed.is_some() {
                status_parts.push("RerouteFailed".to_string());
            }
            if let Some(cd) = cooldown {
                status_parts.push(format!(
                    "Cooldown({:.1}s, stuck {:.1}s)",
                    cd.timer, cd.total_stuck_time
                ));
            }

            let status_str = if status_parts.is_empty() {
                "OK".to_string()
            } else {
                status_parts.join(", ")
            };

            let blocking_str = if blocking.is_some() { "yes" } else { "no" };

            let _ = writeln!(out, "  status: {status_str} | blocking: {blocking_str}");

            let wait_str = driver
                .waiting_tile
                .map(|(wx, wy)| format!("({wx},{wy})"))
                .unwrap_or_else(|| "NONE".to_string());
            let charger_str = driver
                .assigned_charger
                .map(|e| format!("Entity {}", e.index()))
                .unwrap_or_else(|| "NONE".to_string());
            let bay_str = driver
                .assigned_bay
                .map(|(bx, by)| format!("({bx},{by})"))
                .unwrap_or_else(|| "NONE".to_string());

            let _ = writeln!(
                out,
                "  driver_state: {} | patience: {:.0}",
                state_str, driver.patience
            );
            let _ = writeln!(
                out,
                "  bay: {} | wait_tile: {} | charger: {}",
                bay_str, wait_str, charger_str
            );
            let _ = writeln!(out);

            // Track stuck vehicles
            let is_stuck = pathfind_failed.is_some()
                || reroute_failed.is_some()
                || cooldown.is_some()
                || (path.is_none()
                    && next_pos.is_none()
                    && !matches!(
                        driver.state,
                        DriverState::Charging
                            | DriverState::Queued
                            | DriverState::WaitingForCharger
                            | DriverState::Frustrated
                    ));

            if is_stuck {
                let stuck_time = cooldown.map(|c| c.total_stuck_time).unwrap_or(0.0);
                let reason = if pathfind_failed.is_some() {
                    "PathfindingFailed"
                } else if reroute_failed.is_some() {
                    "RerouteFailed"
                } else if path.is_none() && next_pos.is_none() {
                    "no path/next"
                } else {
                    "unknown"
                };
                stuck_vehicles.push((entity.index(), driver.id.clone(), stuck_time, reason));
            }
        }

        // Print ambient summary with pathfinding status
        let _ = writeln!(out, "--- Ambient ({}) ---", site_ambient.len());
        if site_ambient.is_empty() {
            let _ = writeln!(out, "  (none)");
        } else {
            for (entity, amb, _movement, _, agent_pos, next_pos, path, pathfind, pf_failed) in
                &site_ambient
            {
                let grid_str = agent_pos
                    .map(|a| format!("({},{})", a.0.x, a.0.y))
                    .unwrap_or_else(|| "NONE".to_string());
                let next_str = next_pos
                    .map(|n| format!("({},{})", n.0.x, n.0.y))
                    .unwrap_or_else(|| "NONE".to_string());
                let path_str = path
                    .map(|p| format!("{} steps", p.len()))
                    .unwrap_or_else(|| "NONE".to_string());
                let goal_str = pathfind
                    .map(|pf| format!("({},{})", pf.goal.x, pf.goal.y))
                    .unwrap_or_else(|| "NONE".to_string());
                let status = if pf_failed.is_some() {
                    "FAILED"
                } else if path.is_some() {
                    "OK"
                } else {
                    "NO_PATH"
                };
                let _ = writeln!(
                    out,
                    "  Entity {:3}: {:?} {:?} grid={} next={} path={} goal={} [{}]",
                    entity.index(),
                    amb.vehicle_type,
                    amb.direction,
                    grid_str,
                    next_str,
                    path_str,
                    goal_str,
                    status
                );
            }
        }
        let _ = writeln!(out);

        // Print technician summary with pathfinding status
        let _ = writeln!(out, "--- Technicians ({}) ---", site_techs.len());
        if site_techs.is_empty() {
            let _ = writeln!(out, "  (none)");
        } else {
            for (
                entity,
                tech,
                movement,
                _,
                agent_pos,
                next_pos,
                path,
                pathfind,
                pf_failed,
                reroute,
            ) in &site_techs
            {
                let grid_str = agent_pos
                    .map(|a| format!("({},{})", a.0.x, a.0.y))
                    .unwrap_or_else(|| "NONE".to_string());
                let next_str = next_pos
                    .map(|n| format!("({},{})", n.0.x, n.0.y))
                    .unwrap_or_else(|| "NONE".to_string());
                let path_str = path
                    .map(|p| format!("{} steps", p.len()))
                    .unwrap_or_else(|| "NONE".to_string());
                let goal_str = pathfind
                    .map(|pf| format!("({},{})", pf.goal.x, pf.goal.y))
                    .unwrap_or_else(|| "NONE".to_string());
                let status = if pf_failed.is_some() {
                    "FAILED"
                } else if reroute.is_some() {
                    "REROUTE_FAILED"
                } else if path.is_some() {
                    "OK"
                } else {
                    "NO_PATH"
                };
                let _ = writeln!(
                    out,
                    "  Entity {:3}: phase={:?} target={:?} grid={} next={} path={} goal={} [{}]",
                    entity.index(),
                    movement.phase,
                    tech.target_bay,
                    grid_str,
                    next_str,
                    path_str,
                    goal_str,
                    status
                );
            }
        }
        let _ = writeln!(out);

        // Print stuck vehicle summary
        if !stuck_vehicles.is_empty() {
            let _ = writeln!(out, "--- STUCK VEHICLES ({}) ---", stuck_vehicles.len());
            for (idx, id, stuck_time, reason) in &stuck_vehicles {
                if *stuck_time > 0.0 {
                    let _ = writeln!(
                        out,
                        "  Entity {idx:3}: {id} - stuck {stuck_time:.1}s ({reason})"
                    );
                } else {
                    let _ = writeln!(out, "  Entity {idx:3}: {id} - ({reason})");
                }
            }
            let _ = writeln!(out);
        }
    }

    // ASCII grid dump with vehicle positions for each site
    for (site_id, site_state) in multi_site.owned_sites.iter() {
        let grid = &site_state.grid;

        // Collect driver grid positions
        let mut vehicle_positions: std::collections::HashMap<(u32, u32), char> =
            std::collections::HashMap::new();
        for (_, driver, _, belongs, agent_pos, ..) in &drivers {
            if belongs.site_id != *site_id {
                continue;
            }
            let Some(agent) = agent_pos else { continue };
            let key = (agent.0.x, agent.0.y);
            let ch = match driver.state {
                DriverState::Arriving => 'a',
                DriverState::Charging => 'c',
                DriverState::Queued => 'q',
                DriverState::WaitingForCharger => 'w',
                DriverState::Frustrated => 'f',
                DriverState::Leaving | DriverState::Complete => 'd',
                DriverState::LeftAngry => 'x',
            };
            vehicle_positions.insert(key, ch);
        }

        let _ = writeln!(
            out,
            "=== Grid Map {:?} (vehicles: a=arriving c=charging q=queued w=waiting f=frustrated d=departing x=angry) ===",
            site_id
        );
        for y in (0..grid.height).rev() {
            let _ = write!(out, "y={:>2} | ", y);
            for x in 0..grid.width {
                if let Some(&ch) = vehicle_positions.get(&(x as u32, y as u32)) {
                    let _ = write!(out, "{}", ch.to_ascii_uppercase());
                } else if (x, y) == grid.entry_pos {
                    let _ = write!(out, "E");
                } else if (x, y) == grid.exit_pos {
                    let _ = write!(out, "X");
                } else {
                    let ch = match grid.get_content(x, y) {
                        TileContent::Road => 'r',
                        TileContent::Lot => '.',
                        TileContent::ParkingBayNorth | TileContent::ParkingBaySouth => 'p',
                        TileContent::ChargerPad => 'c',
                        TileContent::Grass | TileContent::Empty => ' ',
                        _ => '~',
                    };
                    let _ = write!(out, "{ch}");
                }
            }
            let _ = writeln!(out);
        }
        let _ = writeln!(out);

        // Queue summary
        let _ = writeln!(
            out,
            "Queue: DCFC={} L2={}",
            site_state.charger_queue.dcfc_queue_len(),
            site_state.charger_queue.l2_queue_len(),
        );

        // Charger bay occupancy
        let charger_bays = grid.get_charger_bays();
        let occupied_bays: Vec<(i32, i32)> = drivers
            .iter()
            .filter(|(_, _, _, b, ..)| b.site_id == *site_id)
            .filter(|(_, d, ..)| {
                !matches!(
                    d.state,
                    DriverState::Leaving | DriverState::LeftAngry | DriverState::Complete
                )
            })
            .filter_map(|(_, d, ..)| d.assigned_bay)
            .collect();

        let occupied_waiting: Vec<(i32, i32)> = drivers
            .iter()
            .filter(|(_, _, _, b, ..)| b.site_id == *site_id)
            .filter(|(_, d, ..)| {
                !matches!(
                    d.state,
                    DriverState::Leaving | DriverState::LeftAngry | DriverState::Complete
                )
            })
            .filter_map(|(_, d, ..)| d.waiting_tile)
            .collect();

        let _ = writeln!(
            out,
            "Charger bays: {} total, {} occupied | Waiting tiles: {} occupied",
            charger_bays.len(),
            occupied_bays.len(),
            occupied_waiting.len(),
        );
        let _ = writeln!(out);
    }

    // Grid event debug
    let _ = writeln!(out, "========== GRID EVENT DEBUG ==========\n");
    let _ = writeln!(
        out,
        "viewed_site_id={:?} | game_time={:.0} | is_paused={}\n",
        multi_site.viewed_site_id,
        game_clock.game_time,
        game_clock.is_paused()
    );
    for (site_id, site_state) in multi_site.owned_sites.iter() {
        let ge = &site_state.grid_events;
        let active = ge
            .active_event
            .map(|e| {
                format!(
                    "{} (import {:.1}x, export {:.1}x)",
                    e.name(),
                    e.import_multiplier(),
                    e.export_multiplier()
                )
            })
            .unwrap_or_else(|| "NONE".to_string());
        let _ = writeln!(
            out,
            "Site {:?} (level {}): active_event={} | event_end_time={:.0} | last_roll_time={:.0}",
            site_id, site_state.challenge_level, active, ge.event_end_time, ge.last_event_roll_time
        );
        let _ = writeln!(
            out,
            "  game_time={:.0} | hours_since_roll={:.2} | event_revenue_today=${:.2} | surcharge_today=${:.2}",
            game_clock.game_time,
            (game_clock.game_time - ge.last_event_roll_time) / 3600.0,
            ge.event_revenue_today,
            ge.event_import_surcharge_today
        );
        let best = ge
            .best_event_type
            .map(|e| format!("{} ({:.1}x)", e.name(), e.export_multiplier()))
            .unwrap_or_else(|| "NONE".to_string());
        let _ = writeln!(out, "  best_today={best}");
        let _ = writeln!(out);
    }

    let _ = writeln!(out, "========== END TRAFFIC DEBUG ==========\n");

    // Print to stdout
    print!("{out}");

    // Write to file with timestamp
    let timestamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    let log_path = format!("/tmp/kwtycoon-{timestamp}.log");

    match std::fs::write(&log_path, &out) {
        Ok(()) => info!("Traffic debug written to {log_path}"),
        Err(e) => warn!("Failed to write traffic debug to {log_path}: {e}"),
    }
}
