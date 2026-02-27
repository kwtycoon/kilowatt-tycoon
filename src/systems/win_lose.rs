//! Win/lose condition checking
//!
//! Currently disabled - the game operates on a day-by-day model without
//! explicit win/lose conditions. Players simply manage their station
//! across multiple days.

use bevy::prelude::*;

use crate::resources::BuildState;

/// Placeholder system for future win/lose conditions
/// Currently does nothing - the game continues day by day
pub fn win_lose_system(_build_state: Res<BuildState>) {
    // No win/lose conditions - game continues day by day
    // Players can go into debt or lose reputation without triggering game over
}
