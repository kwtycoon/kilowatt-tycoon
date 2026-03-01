//! Achievement system data model
//!
//! Defines the 10 achievements across 3 tiers (Bronze, Silver, Gold),
//! and the `AchievementState` resource that tracks which have been unlocked.

use std::collections::HashSet;

use bevy::prelude::*;

use crate::resources::game_state::GameState;
use crate::resources::multi_site::MultiSiteManager;
use crate::resources::site_upgrades::OemTier;

/// Tier grouping for achievements
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum AchievementTier {
    Bronze,
    Silver,
    Gold,
}

impl AchievementTier {
    pub fn label(&self) -> &'static str {
        match self {
            AchievementTier::Bronze => "TIER 1 - THE BASICS",
            AchievementTier::Silver => "TIER 2 - GROWING THE EMPIRE",
            AchievementTier::Gold => "TIER 3 - MASTER & META",
        }
    }

    pub fn short_label(&self) -> &'static str {
        match self {
            AchievementTier::Bronze => "Bronze",
            AchievementTier::Silver => "Silver",
            AchievementTier::Gold => "Gold",
        }
    }

    pub fn color(&self) -> Color {
        match self {
            AchievementTier::Bronze => Color::srgb(0.8, 0.5, 0.2),
            AchievementTier::Silver => Color::srgb(0.75, 0.75, 0.75),
            AchievementTier::Gold => Color::srgb(1.0, 0.84, 0.0),
        }
    }
}

/// All possible achievements in the game
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum AchievementKind {
    // Tier 1: Bronze
    PlugAndPlay,
    LemonadeStand,
    DinosaurTears,
    TheGoldenBean,

    // Tier 2: Silver
    PowerBaron,
    OnePointTwentyOneGigawatts,
    MadScientist,

    // Tier 3: Gold
    FleetPerfection,
    OopsTooManyPlugs,
    HamsterWheelTech,
}

impl AchievementKind {
    /// All achievements in display order
    pub const ALL: &'static [AchievementKind] = &[
        AchievementKind::PlugAndPlay,
        AchievementKind::LemonadeStand,
        AchievementKind::DinosaurTears,
        AchievementKind::TheGoldenBean,
        AchievementKind::PowerBaron,
        AchievementKind::OnePointTwentyOneGigawatts,
        AchievementKind::MadScientist,
        AchievementKind::FleetPerfection,
        AchievementKind::OopsTooManyPlugs,
        AchievementKind::HamsterWheelTech,
    ];

    pub fn tier(&self) -> AchievementTier {
        match self {
            AchievementKind::PlugAndPlay
            | AchievementKind::LemonadeStand
            | AchievementKind::DinosaurTears
            | AchievementKind::TheGoldenBean => AchievementTier::Bronze,

            AchievementKind::PowerBaron
            | AchievementKind::OnePointTwentyOneGigawatts
            | AchievementKind::MadScientist => AchievementTier::Silver,

            AchievementKind::FleetPerfection
            | AchievementKind::OopsTooManyPlugs
            | AchievementKind::HamsterWheelTech => AchievementTier::Gold,
        }
    }

    pub fn name(&self) -> &'static str {
        match self {
            AchievementKind::PlugAndPlay => "Plug & Play",
            AchievementKind::LemonadeStand => "Lemonade Stand",
            AchievementKind::DinosaurTears => "Dinosaur Tears",
            AchievementKind::TheGoldenBean => "The Golden Bean",
            AchievementKind::PowerBaron => "Power Baron",
            AchievementKind::OnePointTwentyOneGigawatts => "1.21 Gigawatts!",
            AchievementKind::MadScientist => "Mad Scientist",
            AchievementKind::FleetPerfection => "Fleet Perfection",
            AchievementKind::OopsTooManyPlugs => "Oops... Too Many Plugs",
            AchievementKind::HamsterWheelTech => "Hamster Wheel Tech",
        }
    }

    pub fn description(&self) -> &'static str {
        match self {
            AchievementKind::PlugAndPlay => "Build your very first charging station.",
            AchievementKind::LemonadeStand => "Earn your first $100 from charging fees.",
            AchievementKind::DinosaurTears => "Too much customer complaint.",
            AchievementKind::TheGoldenBean => "Build your first coffee shop next to a charger.",
            AchievementKind::PowerBaron => "Have 10 stations running simultaneously.",
            AchievementKind::OnePointTwentyOneGigawatts => {
                "Deliver a total of 1,000kWh across your network."
            }
            AchievementKind::MadScientist => "Upgrade a site to the ultimate O&M platform.",
            AchievementKind::FleetPerfection => {
                "Complete 50 commercial fleet charges without a single delay."
            }
            AchievementKind::OopsTooManyPlugs => {
                "Crash the local city grid by building too many fast chargers at once."
            }
            AchievementKind::HamsterWheelTech => {
                "Power a site entirely from solar + battery for a full day."
            }
        }
    }

    pub fn quote(&self) -> &'static str {
        match self {
            AchievementKind::PlugAndPlay => "\"And so it begins...\"",
            AchievementKind::LemonadeStand => "\"It's not much, but it's honest work.\"",
            AchievementKind::DinosaurTears => "\"Fossil fuels send their regards.\"",
            AchievementKind::TheGoldenBean => "\"Come for the volts, stay for the caffeine.\"",
            AchievementKind::PowerBaron => "\"You're starting to look like a CEO.\"",
            AchievementKind::OnePointTwentyOneGigawatts => "\"Great Scott!\"",
            AchievementKind::MadScientist => "\"Doc Volt would be proud (and terrified).\"",
            AchievementKind::FleetPerfection => "\"Efficiency is its own reward.\"",
            AchievementKind::OopsTooManyPlugs => "\"Whoops. Dark mode engaged.\"",
            AchievementKind::HamsterWheelTech => {
                "\"Zero grid import. The hamsters would be proud.\""
            }
        }
    }

    /// Emoji fallback for the icon (used when image assets are not available)
    pub fn icon_emoji(&self) -> &'static str {
        match self {
            AchievementKind::PlugAndPlay => "P",
            AchievementKind::LemonadeStand => "$",
            AchievementKind::DinosaurTears => "!",
            AchievementKind::TheGoldenBean => "C",
            AchievementKind::PowerBaron => "K",
            AchievementKind::OnePointTwentyOneGigawatts => "G",
            AchievementKind::MadScientist => "M",
            AchievementKind::FleetPerfection => "F",
            AchievementKind::OopsTooManyPlugs => "X",
            AchievementKind::HamsterWheelTech => "H",
        }
    }

    /// Returns true if this achievement's unlock condition is currently met.
    pub fn is_met(&self, ctx: &AchievementContext) -> bool {
        match self {
            // Bronze
            AchievementKind::PlugAndPlay => ctx.game_state.sessions_completed >= 1,
            AchievementKind::LemonadeStand => ctx.game_state.ledger.gross_revenue_f32() >= 100.0,
            AchievementKind::DinosaurTears => ctx.game_state.reputation <= 10,
            AchievementKind::TheGoldenBean => ctx
                .multi_site
                .owned_sites
                .values()
                .any(|site| !site.grid.amenities.is_empty()),

            // Silver
            AchievementKind::PowerBaron => ctx.multi_site.owned_sites.len() >= 5,
            AchievementKind::OnePointTwentyOneGigawatts => {
                ctx.game_state.total_energy_delivered_kwh >= 1000.0
            }
            AchievementKind::MadScientist => ctx
                .multi_site
                .owned_sites
                .values()
                .any(|site| site.site_upgrades.oem_tier == OemTier::Optimize),

            // Gold
            AchievementKind::FleetPerfection => ctx.game_state.fleet_sessions_without_fault >= 50,
            AchievementKind::OopsTooManyPlugs => ctx.game_state.grid_overload_triggered,
            AchievementKind::HamsterWheelTech => ctx.game_state.zero_grid_day_achieved,
        }
    }

    /// Returns achievements grouped by tier, in display order.
    pub fn by_tier() -> Vec<(AchievementTier, Vec<AchievementKind>)> {
        let tiers = [
            AchievementTier::Bronze,
            AchievementTier::Silver,
            AchievementTier::Gold,
        ];

        tiers
            .iter()
            .map(|tier| {
                let achievements: Vec<AchievementKind> = Self::ALL
                    .iter()
                    .filter(|a| a.tier() == *tier)
                    .copied()
                    .collect();
                (*tier, achievements)
            })
            .collect()
    }
}

/// Snapshot of game state needed by achievement condition checks.
pub struct AchievementContext<'a> {
    pub game_state: &'a GameState,
    pub multi_site: &'a MultiSiteManager,
}

/// Resource tracking which achievements the player has unlocked
#[derive(Resource, Debug, Clone, Default)]
pub struct AchievementState {
    unlocked: HashSet<AchievementKind>,
}

impl AchievementState {
    /// Unlock an achievement. Returns `true` if it was newly unlocked.
    pub fn unlock(&mut self, kind: AchievementKind) -> bool {
        self.unlocked.insert(kind)
    }

    /// Check whether an achievement has been unlocked.
    pub fn is_unlocked(&self, kind: AchievementKind) -> bool {
        self.unlocked.contains(&kind)
    }

    /// Total number of unlocked achievements.
    pub fn unlocked_count(&self) -> usize {
        self.unlocked.len()
    }

    /// Total number of achievements that exist.
    pub fn total_count(&self) -> usize {
        AchievementKind::ALL.len()
    }

    /// Progress as a fraction in `[0.0, 1.0]`.
    pub fn progress(&self) -> f32 {
        if self.total_count() == 0 {
            return 0.0;
        }
        self.unlocked_count() as f32 / self.total_count() as f32
    }

    /// Snapshot the current set of unlocked achievements (for diffing later).
    pub fn snapshot(&self) -> HashSet<AchievementKind> {
        self.unlocked.clone()
    }

    /// Return achievements unlocked since the given snapshot, ordered by tier (highest first).
    pub fn newly_unlocked_since(
        &self,
        snapshot: &HashSet<AchievementKind>,
    ) -> Vec<AchievementKind> {
        let mut new: Vec<AchievementKind> = self.unlocked.difference(snapshot).copied().collect();
        // Sort by tier descending so highest-tier badge comes first
        new.sort_by_key(|b| std::cmp::Reverse(b.tier() as u8));
        new
    }
}

/// Resource storing a snapshot of unlocked achievements at the start of a day.
/// Used to compute which badges were newly earned during the day.
#[derive(Resource, Debug, Clone, Default)]
pub struct AchievementSnapshot {
    pub unlocked_at_day_start: HashSet<AchievementKind>,
}

#[cfg(test)]
mod tests {
    use crate::resources::achievements::*;
    use crate::resources::multi_site::MultiSiteManager;

    /// Helper to build an `AchievementContext` with default `MultiSiteManager`.
    fn ctx_with_gs(gs: &GameState) -> AchievementContext<'_> {
        // We leak a default MultiSiteManager to get a stable reference inside the test.
        // This is a test-only convenience; the leaked memory is tiny and freed at process exit.
        let ms: &'static MultiSiteManager = Box::leak(Box::new(MultiSiteManager::default()));
        AchievementContext {
            game_state: gs,
            multi_site: ms,
        }
    }

    // ---- AchievementTier ----

    #[test]
    fn test_tier_labels() {
        assert_eq!(AchievementTier::Bronze.label(), "TIER 1 - THE BASICS");
        assert_eq!(
            AchievementTier::Silver.label(),
            "TIER 2 - GROWING THE EMPIRE"
        );
        assert_eq!(AchievementTier::Gold.label(), "TIER 3 - MASTER & META");
    }

    // ---- AchievementKind ----

    #[test]
    fn test_all_achievements_count() {
        assert_eq!(AchievementKind::ALL.len(), 10);
    }

    #[test]
    fn test_by_tier_grouping() {
        let grouped = AchievementKind::by_tier();
        assert_eq!(grouped.len(), 3);
        assert_eq!(grouped[0].0, AchievementTier::Bronze);
        assert_eq!(grouped[0].1.len(), 4);
        assert_eq!(grouped[1].0, AchievementTier::Silver);
        assert_eq!(grouped[1].1.len(), 3);
        assert_eq!(grouped[2].0, AchievementTier::Gold);
        assert_eq!(grouped[2].1.len(), 3);
    }

    #[test]
    fn test_achievement_metadata_not_empty() {
        for kind in AchievementKind::ALL {
            assert!(!kind.name().is_empty(), "{:?} has empty name", kind);
            assert!(
                !kind.description().is_empty(),
                "{:?} has empty description",
                kind
            );
            assert!(!kind.quote().is_empty(), "{:?} has empty quote", kind);
        }
    }

    // ---- AchievementState ----

    #[test]
    fn test_default_state_is_empty() {
        let state = AchievementState::default();
        assert_eq!(state.unlocked_count(), 0);
        assert_eq!(state.total_count(), 10);
        assert!((state.progress() - 0.0).abs() < f32::EPSILON);
    }

    #[test]
    fn test_unlock_returns_true_on_first_unlock() {
        let mut state = AchievementState::default();
        assert!(state.unlock(AchievementKind::PlugAndPlay));
    }

    #[test]
    fn test_unlock_returns_false_on_duplicate() {
        let mut state = AchievementState::default();
        state.unlock(AchievementKind::PlugAndPlay);
        assert!(!state.unlock(AchievementKind::PlugAndPlay));
    }

    #[test]
    fn test_is_unlocked() {
        let mut state = AchievementState::default();
        assert!(!state.is_unlocked(AchievementKind::LemonadeStand));
        state.unlock(AchievementKind::LemonadeStand);
        assert!(state.is_unlocked(AchievementKind::LemonadeStand));
    }

    #[test]
    fn test_progress_partial() {
        let mut state = AchievementState::default();
        state.unlock(AchievementKind::PlugAndPlay);
        state.unlock(AchievementKind::LemonadeStand);
        assert_eq!(state.unlocked_count(), 2);
        let expected = 2.0 / 10.0;
        assert!((state.progress() - expected).abs() < f32::EPSILON);
    }

    #[test]
    fn test_is_met_plug_and_play() {
        let gs = GameState::default();
        let ctx = ctx_with_gs(&gs);
        assert!(!AchievementKind::PlugAndPlay.is_met(&ctx));

        let gs = GameState {
            sessions_completed: 1,
            ..Default::default()
        };
        let ctx = ctx_with_gs(&gs);
        assert!(AchievementKind::PlugAndPlay.is_met(&ctx));
    }

    #[test]
    fn test_is_met_defaults_return_false() {
        let gs = GameState::default();
        let ctx = ctx_with_gs(&gs);
        // With default GameState and empty MultiSiteManager, no achievement should be met
        for kind in AchievementKind::ALL {
            assert!(
                !kind.is_met(&ctx),
                "{:?} should not be met with defaults",
                kind
            );
        }
    }

    #[test]
    fn test_is_met_lemonade_stand() {
        let mut gs = GameState::default();
        gs.add_charging_revenue(100.0);
        let ctx = ctx_with_gs(&gs);
        assert!(AchievementKind::LemonadeStand.is_met(&ctx));
    }

    #[test]
    fn test_is_met_dinosaur_tears() {
        let gs = GameState {
            reputation: 10,
            ..Default::default()
        };
        let ctx = ctx_with_gs(&gs);
        assert!(AchievementKind::DinosaurTears.is_met(&ctx));
    }

    #[test]
    fn test_is_met_one_point_twenty_one_gigawatts() {
        let gs = GameState {
            total_energy_delivered_kwh: 999.9,
            ..Default::default()
        };
        let ctx = ctx_with_gs(&gs);
        assert!(!AchievementKind::OnePointTwentyOneGigawatts.is_met(&ctx));

        let gs = GameState {
            total_energy_delivered_kwh: 1000.0,
            ..Default::default()
        };
        let ctx = ctx_with_gs(&gs);
        assert!(AchievementKind::OnePointTwentyOneGigawatts.is_met(&ctx));
    }

    #[test]
    fn test_is_met_fleet_perfection() {
        let gs = GameState {
            fleet_sessions_without_fault: 49,
            ..Default::default()
        };
        let ctx = ctx_with_gs(&gs);
        assert!(!AchievementKind::FleetPerfection.is_met(&ctx));

        let gs = GameState {
            fleet_sessions_without_fault: 50,
            ..Default::default()
        };
        let ctx = ctx_with_gs(&gs);
        assert!(AchievementKind::FleetPerfection.is_met(&ctx));
    }

    #[test]
    fn test_is_met_oops_too_many_plugs() {
        let gs = GameState::default();
        let ctx = ctx_with_gs(&gs);
        assert!(!AchievementKind::OopsTooManyPlugs.is_met(&ctx));

        let gs = GameState {
            grid_overload_triggered: true,
            ..Default::default()
        };
        let ctx = ctx_with_gs(&gs);
        assert!(AchievementKind::OopsTooManyPlugs.is_met(&ctx));
    }

    #[test]
    fn test_is_met_hamster_wheel_tech() {
        let gs = GameState::default();
        let ctx = ctx_with_gs(&gs);
        assert!(!AchievementKind::HamsterWheelTech.is_met(&ctx));

        let gs = GameState {
            zero_grid_day_achieved: true,
            ..Default::default()
        };
        let ctx = ctx_with_gs(&gs);
        assert!(AchievementKind::HamsterWheelTech.is_met(&ctx));
    }

    #[test]
    fn test_progress_full() {
        let mut state = AchievementState::default();
        for kind in AchievementKind::ALL {
            state.unlock(*kind);
        }
        assert_eq!(state.unlocked_count(), 10);
        assert!((state.progress() - 1.0).abs() < f32::EPSILON);
    }
}
