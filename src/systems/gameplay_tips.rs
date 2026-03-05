//! Contextual gameplay tips shown as toasts when the player appears to be struggling
//! with specific mechanics. Each tip fires at most once per game session.

use std::collections::HashSet;

use bevy::prelude::*;

use crate::components::Charger;
use crate::resources::site_upgrades::{
    DEMAND_BOOST_DURATION_SECS, DEMAND_BOOST_MULTIPLIER, OemTier, UpgradeId, upgrade_costs,
};
use crate::resources::strategy::WarrantyTier;
use crate::resources::{GameClock, GameState, ImageAssets, MultiSiteManager};
use crate::ui::sidebar::SecondaryNav;
use crate::ui::toast::{TipToast, ToastContainer, spawn_tip_toast};

const CHECK_INTERVAL_GAME_SECS: f32 = 60.0;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum TipKind {
    OmDetect,
    OmOptimize,
    Warranty,
    Reliability,
    PriceLow,
    DemandBoost,
}

#[derive(Resource, Default)]
pub struct GameplayTipsState {
    pub shown: HashSet<TipKind>,
    pub dismissed_all: bool,
    last_check_game_time: f32,
}

struct TipCandidate {
    kind: TipKind,
    message: String,
}

pub fn check_gameplay_tips(
    mut commands: Commands,
    game_clock: Res<GameClock>,
    game_state: Res<GameState>,
    multi_site: Res<MultiSiteManager>,
    chargers: Query<&Charger>,
    image_assets: Res<ImageAssets>,
    time: Res<Time>,
    mut tips_state: ResMut<GameplayTipsState>,
    existing_tips: Query<Entity, With<TipToast>>,
    container: Single<Entity, With<ToastContainer>>,
) {
    if tips_state.dismissed_all {
        return;
    }

    let elapsed = game_clock.game_time - tips_state.last_check_game_time;
    if elapsed < CHECK_INTERVAL_GAME_SECS {
        return;
    }
    tips_state.last_check_game_time = game_clock.game_time;

    let Some(site) = multi_site.active_site() else {
        return;
    };

    let today_financials = game_state
        .ledger
        .daily_totals(game_state.ledger.current_date);
    let hour = game_clock.hour();
    let dispatches_today = game_state.daily_history.current_day.dispatches;
    let sessions_today = game_state.daily_history.current_day.sessions;
    let charging_revenue = today_financials.charging_revenue;
    let energy_cost = today_financials.energy_cost;
    let repair_parts = today_financials.repair_parts;
    let repair_labor = today_financials.repair_labor;
    let repair_total = repair_parts + repair_labor;

    let oem_tier = site.site_upgrades.oem_tier;
    let warranty_tier = site.service_strategy.warranty_tier;
    let maintenance_investment = site.service_strategy.maintenance_investment;
    let demand_boost_active = site.site_upgrades.is_demand_boost_active();

    let faulted_count = chargers
        .iter()
        .filter(|c| c.current_fault.is_some())
        .count();

    let candidate = evaluate_tips(
        &tips_state,
        faulted_count,
        oem_tier,
        dispatches_today,
        repair_total,
        warranty_tier,
        maintenance_investment,
        charging_revenue,
        energy_cost,
        sessions_today,
        hour,
        demand_boost_active,
        game_state.reputation,
    );

    if let Some(tip) = candidate {
        tips_state.shown.insert(tip.kind);
        spawn_tip_toast(
            &mut commands,
            *container,
            &tip.message,
            game_clock.game_time,
            time.elapsed_secs(),
            image_assets.icon_info.clone(),
            &existing_tips,
        );
    }
}

#[allow(clippy::too_many_arguments)]
fn evaluate_tips(
    state: &GameplayTipsState,
    faulted_count: usize,
    oem_tier: OemTier,
    dispatches_today: i32,
    repair_total: f32,
    warranty_tier: WarrantyTier,
    maintenance_investment: f32,
    charging_revenue: f32,
    energy_cost: f32,
    sessions_today: i32,
    hour: u32,
    demand_boost_active: bool,
    reputation: i32,
) -> Option<TipCandidate> {
    let upgrades_path = SecondaryNav::BuildUpgrades.nav_path();
    let opex_path = SecondaryNav::StrategyOpex.nav_path();
    let pricing_path = SecondaryNav::StrategyPricing.nav_path();

    let om_detect_name = UpgradeId::OemDetect.display_name();
    let om_optimize_name = UpgradeId::OemOptimize.display_name();
    let demand_blitz_name = UpgradeId::DemandBoost.display_name();
    let demand_blitz_cost = upgrade_costs::DEMAND_BOOST as u32;
    let demand_blitz_mult = DEMAND_BOOST_MULTIPLIER as u32;
    let demand_blitz_hours = (DEMAND_BOOST_DURATION_SECS / 3600.0) as u32;

    let tips: Vec<(TipKind, bool, String)> = vec![
        (
            TipKind::Reliability,
            faulted_count >= 2 && maintenance_investment <= 10.0,
            format!(
                "Chargers keep breaking down? Increase maintenance spending in {opex_path} to reduce failure rates."
            ),
        ),
        (
            TipKind::OmDetect,
            faulted_count >= 3 && oem_tier == OemTier::None,
            format!(
                "Struggling with faults? Buy {om_detect_name} in {upgrades_path} for instant detection and remote remediation."
            ),
        ),
        (
            TipKind::OmOptimize,
            dispatches_today >= 2 && !oem_tier.at_least(OemTier::Optimize),
            format!(
                "Dispatching techs all day? {om_optimize_name} in {upgrades_path} auto-dispatches for hardware faults and speeds up repairs by 25%."
            ),
        ),
        (
            TipKind::Warranty,
            repair_total > 2000.0 && warranty_tier == WarrantyTier::None,
            format!(
                "Repair bills adding up? An extended warranty covers parts costs \u{2014} check {opex_path}."
            ),
        ),
        (
            TipKind::PriceLow,
            charging_revenue < energy_cost && energy_cost > 10.0,
            format!("Selling electricity below cost! Raise your price in {pricing_path}."),
        ),
        (
            TipKind::DemandBoost,
            sessions_today < 3 && hour >= 6 && !demand_boost_active && reputation >= 40,
            format!(
                "Low traffic? {demand_blitz_name} ({demand_blitz_mult}x demand for {demand_blitz_hours}h) costs just ${demand_blitz_cost} in {upgrades_path}."
            ),
        ),
    ];

    for (kind, condition, message) in tips {
        if condition && !state.shown.contains(&kind) {
            return Some(TipCandidate { kind, message });
        }
    }

    None
}

#[cfg(test)]
mod tests {
    use super::*;

    fn empty_state() -> GameplayTipsState {
        GameplayTipsState::default()
    }

    #[test]
    fn om_detect_fires_when_faults_without_oem() {
        let mut state = empty_state();
        state.shown.insert(TipKind::Reliability);
        let tip = evaluate_tips(
            &state,
            3,
            OemTier::None,
            0,
            0.0,
            WarrantyTier::None,
            10.0,
            0.0,
            0.0,
            0,
            8,
            false,
            50,
        );
        assert_eq!(tip.as_ref().map(|t| t.kind), Some(TipKind::OmDetect));
    }

    #[test]
    fn om_detect_suppressed_with_oem() {
        let state = empty_state();
        let tip = evaluate_tips(
            &state,
            3,
            OemTier::Detect,
            0,
            0.0,
            WarrantyTier::None,
            20.0,
            0.0,
            0.0,
            0,
            8,
            false,
            50,
        );
        assert_ne!(tip.as_ref().map(|t| t.kind), Some(TipKind::OmDetect));
    }

    #[test]
    fn om_optimize_fires_after_two_dispatches() {
        let state = empty_state();
        let tip = evaluate_tips(
            &state,
            0,
            OemTier::Detect,
            2,
            0.0,
            WarrantyTier::None,
            10.0,
            0.0,
            0.0,
            0,
            8,
            false,
            50,
        );
        assert_eq!(tip.as_ref().map(|t| t.kind), Some(TipKind::OmOptimize));
    }

    #[test]
    fn om_optimize_suppressed_at_optimize_tier() {
        let state = empty_state();
        let tip = evaluate_tips(
            &state,
            0,
            OemTier::Optimize,
            2,
            0.0,
            WarrantyTier::None,
            10.0,
            0.0,
            0.0,
            0,
            8,
            false,
            50,
        );
        assert_ne!(tip.as_ref().map(|t| t.kind), Some(TipKind::OmOptimize));
    }

    #[test]
    fn warranty_fires_on_high_repair_costs() {
        let state = empty_state();
        let tip = evaluate_tips(
            &state,
            0,
            OemTier::Detect,
            0,
            2500.0,
            WarrantyTier::None,
            20.0,
            0.0,
            0.0,
            0,
            8,
            false,
            50,
        );
        assert_eq!(tip.as_ref().map(|t| t.kind), Some(TipKind::Warranty));
    }

    #[test]
    fn warranty_suppressed_with_existing_warranty() {
        let state = empty_state();
        let tip = evaluate_tips(
            &state,
            0,
            OemTier::Detect,
            0,
            2500.0,
            WarrantyTier::Standard,
            20.0,
            0.0,
            0.0,
            0,
            8,
            false,
            50,
        );
        assert_ne!(tip.as_ref().map(|t| t.kind), Some(TipKind::Warranty));
    }

    #[test]
    fn reliability_fires_on_frequent_faults_low_maintenance() {
        let state = empty_state();
        let tip = evaluate_tips(
            &state,
            2,
            OemTier::Detect,
            0,
            0.0,
            WarrantyTier::None,
            10.0,
            0.0,
            0.0,
            0,
            8,
            false,
            50,
        );
        assert_eq!(tip.as_ref().map(|t| t.kind), Some(TipKind::Reliability));
    }

    #[test]
    fn reliability_suppressed_with_high_maintenance() {
        let state = empty_state();
        let tip = evaluate_tips(
            &state,
            2,
            OemTier::Detect,
            0,
            0.0,
            WarrantyTier::None,
            20.0,
            0.0,
            0.0,
            0,
            8,
            false,
            50,
        );
        assert_ne!(tip.as_ref().map(|t| t.kind), Some(TipKind::Reliability));
    }

    #[test]
    fn price_low_fires_when_selling_below_cost() {
        let state = empty_state();
        let tip = evaluate_tips(
            &state,
            0,
            OemTier::Detect,
            0,
            0.0,
            WarrantyTier::None,
            20.0,
            5.0,
            50.0,
            10,
            12,
            false,
            50,
        );
        assert_eq!(tip.as_ref().map(|t| t.kind), Some(TipKind::PriceLow));
    }

    #[test]
    fn demand_boost_fires_with_low_sessions() {
        let state = empty_state();
        let tip = evaluate_tips(
            &state,
            0,
            OemTier::Detect,
            0,
            0.0,
            WarrantyTier::None,
            20.0,
            100.0,
            50.0,
            2,
            8,
            false,
            50,
        );
        assert_eq!(tip.as_ref().map(|t| t.kind), Some(TipKind::DemandBoost));
    }

    #[test]
    fn demand_boost_suppressed_when_active() {
        let state = empty_state();
        let tip = evaluate_tips(
            &state,
            0,
            OemTier::Detect,
            0,
            0.0,
            WarrantyTier::None,
            20.0,
            100.0,
            50.0,
            2,
            8,
            true,
            50,
        );
        assert_ne!(tip.as_ref().map(|t| t.kind), Some(TipKind::DemandBoost));
    }

    #[test]
    fn shown_tips_not_repeated() {
        let mut state = empty_state();
        state.shown.insert(TipKind::Reliability);
        state.shown.insert(TipKind::OmDetect);
        let tip = evaluate_tips(
            &state,
            5,
            OemTier::None,
            0,
            0.0,
            WarrantyTier::None,
            10.0,
            0.0,
            0.0,
            0,
            8,
            false,
            50,
        );
        assert_ne!(tip.as_ref().map(|t| t.kind), Some(TipKind::OmDetect));
        assert_ne!(tip.as_ref().map(|t| t.kind), Some(TipKind::Reliability));
    }

    #[test]
    fn reliability_fires_before_om_detect() {
        let state = empty_state();
        let tip = evaluate_tips(
            &state,
            3,
            OemTier::None,
            0,
            0.0,
            WarrantyTier::None,
            10.0,
            0.0,
            0.0,
            0,
            8,
            false,
            50,
        );
        assert_eq!(
            tip.as_ref().map(|t| t.kind),
            Some(TipKind::Reliability),
            "Reliability (increase maintenance) should fire before OmDetect"
        );
    }

    #[test]
    fn no_tip_when_nothing_wrong() {
        let state = empty_state();
        let tip = evaluate_tips(
            &state,
            0,
            OemTier::Detect,
            0,
            0.0,
            WarrantyTier::Standard,
            20.0,
            100.0,
            50.0,
            10,
            12,
            true,
            50,
        );
        assert!(tip.is_none());
    }

    #[test]
    fn messages_use_nav_paths() {
        let mut state = empty_state();
        state.shown.insert(TipKind::Reliability);
        let tip = evaluate_tips(
            &state,
            3,
            OemTier::None,
            0,
            0.0,
            WarrantyTier::None,
            10.0,
            0.0,
            0.0,
            0,
            8,
            false,
            50,
        );
        let msg = tip.expect("should fire om_detect").message;
        assert!(
            msg.contains(&SecondaryNav::BuildUpgrades.nav_path()),
            "expected nav path in message, got: {msg}"
        );
    }

    #[test]
    fn messages_use_upgrade_names() {
        let mut state = empty_state();
        state.shown.insert(TipKind::Reliability);
        let tip = evaluate_tips(
            &state,
            3,
            OemTier::None,
            0,
            0.0,
            WarrantyTier::None,
            10.0,
            0.0,
            0.0,
            0,
            8,
            false,
            50,
        );
        let msg = tip.expect("should fire om_detect").message;
        assert!(
            msg.contains(UpgradeId::OemDetect.display_name()),
            "expected upgrade name in message, got: {msg}"
        );
    }

    #[test]
    fn demand_boost_message_uses_constants() {
        let state = empty_state();
        let tip = evaluate_tips(
            &state,
            0,
            OemTier::Detect,
            0,
            0.0,
            WarrantyTier::None,
            20.0,
            100.0,
            50.0,
            2,
            8,
            false,
            50,
        );
        let msg = tip.expect("should fire demand_boost").message;
        assert!(
            msg.contains(UpgradeId::DemandBoost.display_name()),
            "expected Demand Blitz name in message, got: {msg}"
        );
        let cost_str = format!("${}", upgrade_costs::DEMAND_BOOST as u32);
        assert!(
            msg.contains(&cost_str),
            "expected cost {cost_str} in message, got: {msg}"
        );
    }
}
