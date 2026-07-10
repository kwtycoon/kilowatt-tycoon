//! Samples per-site power history (actual draw vs effective limit) for the
//! Stats -> Power 24h chart. Runs after utility billing on the viewed site.

use bevy::prelude::*;

use crate::components::BelongsToSite;
use crate::components::charger::Charger;
use crate::resources::{GameClock, MultiSiteManager};

/// Effective power ceiling (kW) for a single charger: its OCPP charging-profile
/// cap if one is applied, otherwise its rated (nameplate) power.
pub fn effective_charger_limit_kw(ocpp_limit_kw: Option<f32>, rated_power_kw: f32) -> f32 {
    match ocpp_limit_kw {
        Some(limit) => limit.max(0.0),
        None => rated_power_kw,
    }
}

/// Effective site power limit (kW): the sum of each enabled charger's effective
/// ceiling, clamped to the site's grid dispatch limit.
///
/// Each item is `(is_disabled, ocpp_limit_kw, rated_power_kw)`.
pub fn site_power_limit_kw<I>(chargers: I, dispatch_limit_kw: f32) -> f32
where
    I: IntoIterator<Item = (bool, Option<f32>, f32)>,
{
    let sum: f32 = chargers
        .into_iter()
        .filter(|(is_disabled, _, _)| !*is_disabled)
        .map(|(_, ocpp, rated)| effective_charger_limit_kw(ocpp, rated))
        .sum();
    sum.min(dispatch_limit_kw)
}

/// What constrains the site's effective power limit right now.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LimitSource {
    /// The grid/transformer dispatch limit is the binding constraint.
    Grid,
    /// One or more OCPP charging profiles cap the chargers below the grid limit.
    Ocpp,
    /// The chargers' own nameplate ratings bind (no OCPP caps, grid has headroom).
    Hardware,
}

impl LimitSource {
    /// Short human-readable label for UI display.
    pub fn label(self) -> &'static str {
        match self {
            LimitSource::Grid => "Grid",
            LimitSource::Ocpp => "OCPP profiles",
            LimitSource::Hardware => "Charger hardware",
        }
    }
}

/// Breakdown of how the site's effective power limit is derived, so the UI can
/// show whether it's grid-, OCPP-, or hardware-bound.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SiteLimitBreakdown {
    /// Grid/transformer dispatch limit (kW).
    pub grid_dispatch_kw: f32,
    /// Sum of every enabled charger's effective ceiling (OCPP cap or rated).
    pub charger_sum_kw: f32,
    /// Sum over only the chargers that currently have an OCPP cap applied.
    pub ocpp_capped_sum_kw: f32,
    /// Number of enabled chargers currently under an OCPP cap.
    pub ocpp_charger_count: usize,
    /// Effective site limit = min(charger_sum, grid_dispatch).
    pub effective_kw: f32,
    /// Which constraint is binding.
    pub source: LimitSource,
}

/// Compute the site limit breakdown from the per-charger data and the grid
/// dispatch limit. Each item is `(is_disabled, ocpp_limit_kw, rated_power_kw)`.
pub fn site_limit_breakdown<I>(chargers: I, dispatch_limit_kw: f32) -> SiteLimitBreakdown
where
    I: IntoIterator<Item = (bool, Option<f32>, f32)>,
{
    let mut charger_sum_kw = 0.0;
    let mut ocpp_capped_sum_kw = 0.0;
    let mut ocpp_charger_count = 0;
    let mut any_ocpp = false;

    for (is_disabled, ocpp, rated) in chargers {
        if is_disabled {
            continue;
        }
        charger_sum_kw += effective_charger_limit_kw(ocpp, rated);
        if let Some(cap) = ocpp {
            any_ocpp = true;
            ocpp_charger_count += 1;
            ocpp_capped_sum_kw += cap.max(0.0);
        }
    }

    let effective_kw = charger_sum_kw.min(dispatch_limit_kw);
    let source = if dispatch_limit_kw < charger_sum_kw {
        LimitSource::Grid
    } else if any_ocpp {
        LimitSource::Ocpp
    } else {
        LimitSource::Hardware
    };

    SiteLimitBreakdown {
        grid_dispatch_kw: dispatch_limit_kw,
        charger_sum_kw,
        ocpp_capped_sum_kw,
        ocpp_charger_count,
        effective_kw,
        source,
    }
}

/// Sample the viewed site's power draw and effective limit into its rolling
/// 24h history (throttled to one sample per game-minute by `maybe_sample`).
pub fn sample_power_history_system(
    mut multi_site: ResMut<MultiSiteManager>,
    game_clock: Res<GameClock>,
    chargers: Query<(&Charger, &BelongsToSite)>,
) {
    if game_clock.is_paused() {
        return;
    }

    let Some(viewed_id) = multi_site.viewed_site_id else {
        return;
    };

    // Sum per-charger effective ceilings for the viewed site.
    let charger_sum_kw: f32 = chargers
        .iter()
        .filter(|(_, belongs)| belongs.site_id == viewed_id)
        .filter(|(charger, _)| !charger.is_disabled)
        .map(|(charger, _)| {
            effective_charger_limit_kw(charger.ocpp_limit_kw, charger.rated_power_kw)
        })
        .sum();

    let Some(site) = multi_site.owned_sites.get_mut(&viewed_id) else {
        return;
    };

    let limit_kw = charger_sum_kw.min(site.dispatch_limit_kva());
    let draw_kw = site.grid_import.gross_load_kw;
    site.power_history
        .maybe_sample(game_clock.game_time, draw_kw, limit_kw);
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::resources::site_energy::SitePowerHistory;

    #[test]
    fn uncapped_charger_uses_rated_power() {
        assert_eq!(effective_charger_limit_kw(None, 150.0), 150.0);
    }

    #[test]
    fn ocpp_cap_overrides_rated_power() {
        assert_eq!(effective_charger_limit_kw(Some(20.0), 150.0), 20.0);
    }

    #[test]
    fn site_limit_sums_mixed_caps_then_clamps_to_dispatch() {
        // Two capped (20 + 30) + one uncapped rated 150 = 200; dispatch 500 -> 200.
        let chargers = vec![
            (false, Some(20.0), 150.0),
            (false, Some(30.0), 150.0),
            (false, None, 150.0),
        ];
        assert_eq!(site_power_limit_kw(chargers, 500.0), 200.0);
    }

    #[test]
    fn site_limit_clamps_to_dispatch_when_sum_exceeds_grid() {
        // Three uncapped 150 kW chargers = 450 potential, but grid only 300.
        let chargers = vec![
            (false, None, 150.0),
            (false, None, 150.0),
            (false, None, 150.0),
        ];
        assert_eq!(site_power_limit_kw(chargers, 300.0), 300.0);
    }

    #[test]
    fn disabled_chargers_are_excluded() {
        let chargers = vec![(true, None, 150.0), (false, Some(50.0), 150.0)];
        assert_eq!(site_power_limit_kw(chargers, 1000.0), 50.0);
    }

    #[test]
    fn breakdown_grid_bound_when_dispatch_below_charger_sum() {
        // Three uncapped 150 kW chargers = 450; grid only 300 -> Grid.
        let chargers = vec![
            (false, None, 150.0),
            (false, None, 150.0),
            (false, None, 150.0),
        ];
        let b = site_limit_breakdown(chargers, 300.0);
        assert_eq!(b.source, LimitSource::Grid);
        assert_eq!(b.effective_kw, 300.0);
        assert_eq!(b.ocpp_charger_count, 0);
    }

    #[test]
    fn breakdown_ocpp_bound_when_caps_below_grid() {
        // Caps 20 + 30, uncapped 150 = 200; grid 500 -> Ocpp (a cap exists).
        let chargers = vec![
            (false, Some(20.0), 150.0),
            (false, Some(30.0), 150.0),
            (false, None, 150.0),
        ];
        let b = site_limit_breakdown(chargers, 500.0);
        assert_eq!(b.source, LimitSource::Ocpp);
        assert_eq!(b.effective_kw, 200.0);
        assert_eq!(b.ocpp_capped_sum_kw, 50.0);
        assert_eq!(b.ocpp_charger_count, 2);
    }

    #[test]
    fn breakdown_hardware_bound_when_no_caps_and_grid_has_headroom() {
        // Two uncapped 150 kW = 300; grid 1000 -> Hardware.
        let chargers = vec![(false, None, 150.0), (false, None, 150.0)];
        let b = site_limit_breakdown(chargers, 1000.0);
        assert_eq!(b.source, LimitSource::Hardware);
        assert_eq!(b.effective_kw, 300.0);
        assert_eq!(b.ocpp_charger_count, 0);
    }

    #[test]
    fn history_samples_only_on_interval_boundaries() {
        let mut history = SitePowerHistory::default();
        history.maybe_sample(0.0, 10.0, 100.0); // first sample always taken
        history.maybe_sample(30.0, 20.0, 100.0); // < 60s later -> ignored
        history.maybe_sample(60.0, 40.0, 100.0); // >= 60s -> taken
        assert_eq!(history.samples.len(), 2);
        assert_eq!(history.samples[0].draw_kw, 10.0);
        assert_eq!(history.samples[1].draw_kw, 40.0);
        assert_eq!(history.peak_draw_kw, 40.0);
    }

    #[test]
    fn history_clear_resets_samples_and_peaks() {
        let mut history = SitePowerHistory::default();
        history.maybe_sample(0.0, 10.0, 100.0);
        history.maybe_sample(60.0, 40.0, 120.0);
        history.clear();
        assert!(history.samples.is_empty());
        assert_eq!(history.peak_draw_kw, 0.0);
        assert_eq!(history.peak_limit_kw, 0.0);
        assert_eq!(history.last_sample_game_time, 0.0);
    }
}
