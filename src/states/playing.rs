//! Playing state systems and utilities.
//!
//! This module provides run conditions and utilities for systems
//! that should only run during active gameplay.

use bevy::prelude::*;

use crate::resources::BuildState;
use crate::states::AppState;

/// Run condition: returns true if the game is in the Playing state
pub fn is_playing(state: Res<State<AppState>>) -> bool {
    *state.get() == AppState::Playing
}

/// Run condition: returns true if the game is in Playing or Paused state
pub fn is_game_active(state: Res<State<AppState>>) -> bool {
    matches!(state.get(), AppState::Playing | AppState::Paused)
}

/// Run condition: returns true if the game world should be visible
pub fn is_game_visible(state: Res<State<AppState>>) -> bool {
    state.get().is_game_visible()
}

/// Run condition that combines playing state with simulation not paused
pub fn simulation_running(
    app_state: Res<State<AppState>>,
    game_clock: Res<crate::resources::GameClock>,
) -> bool {
    *app_state.get() == AppState::Playing && !game_clock.is_paused()
}

/// Run condition: returns true if the station is open for business
/// This gates simulation systems to prevent time from advancing during build phase
pub fn is_station_open(build_state: Res<BuildState>) -> bool {
    build_state.is_open
}

/// Transition to game over state when the game result is no longer in progress
pub fn check_game_over_transition(
    game_state: Res<crate::resources::GameState>,
    current_state: Res<State<AppState>>,
    mut next_state: ResMut<NextState<AppState>>,
) {
    if *current_state.get() == AppState::Playing && game_state.result.is_ended() {
        next_state.set(AppState::GameOver);
    }
}

/// Called when entering the playing state.
pub fn on_enter_playing(
    mut commands: Commands,
    mut game_state: ResMut<crate::resources::GameState>,
    mut multi_site: ResMut<crate::resources::MultiSiteManager>,
    mut game_clock: ResMut<crate::resources::GameClock>,
    achievement_state: Res<crate::resources::achievements::AchievementState>,
    mut fleet_mgr: ResMut<crate::resources::FleetContractManager>,
) {
    if game_state.result.is_ended() {
        game_state.reset();
        *multi_site = crate::resources::MultiSiteManager::default();
        game_clock.reset();
        *fleet_mgr = crate::resources::FleetContractManager::default();
    }

    if let Some(date) =
        chrono::NaiveDate::from_ymd_opt(game_clock.year as i32, game_clock.month, game_clock.day)
    {
        game_state.ledger.current_date = date;
    }

    commands.insert_resource(crate::resources::achievements::AchievementSnapshot {
        unlocked_at_day_start: achievement_state.snapshot(),
    });

    // Fleet: reset per-day counters and load contracts for the active site
    fleet_mgr.reset_daily();
    if let Some(site) = multi_site.active_site() {
        let archetype = site.archetype;
        fleet_mgr.load_for_archetype(
            crate::resources::fleet::builtin_fleet_contracts(),
            archetype,
        );
    }
}

/// Called when exiting the playing state.
pub fn on_exit_playing() {}
