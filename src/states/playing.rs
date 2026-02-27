//! Playing state systems and utilities.
//!
//! This module provides run conditions and utilities for systems
//! that should only run during active gameplay.

use bevy::prelude::*;

use super::AppState;
use crate::resources::BuildState;

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
