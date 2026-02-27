//! Achievement checking system
//!
//! Runs in the simulation layer (after all game state updates) to check
//! whether any achievement conditions have been newly met, and fires
//! `AchievementUnlockedEvent` messages for each new unlock.

use bevy::prelude::*;

use crate::events::AchievementUnlockedEvent;
use crate::resources::GameState;
use crate::resources::MultiSiteManager;
use crate::resources::achievements::{AchievementContext, AchievementKind, AchievementState};

/// Check all achievements against current game state and unlock any whose
/// conditions are newly met.  Fires an [`AchievementUnlockedEvent`] for each.
pub fn check_achievements(
    game_state: Res<GameState>,
    multi_site: Res<MultiSiteManager>,
    mut achievement_state: ResMut<AchievementState>,
    mut messages: MessageWriter<AchievementUnlockedEvent>,
) {
    let ctx = AchievementContext {
        game_state: &game_state,
        multi_site: &multi_site,
    };

    for kind in AchievementKind::ALL {
        if !achievement_state.is_unlocked(*kind) && kind.is_met(&ctx) {
            achievement_state.unlock(*kind);
            messages.write(AchievementUnlockedEvent { kind: *kind });
            info!("Achievement unlocked: {}", kind.name());
        }
    }
}
