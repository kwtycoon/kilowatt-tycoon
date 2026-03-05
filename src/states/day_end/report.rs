use bevy::prelude::*;

use crate::resources::achievements::AchievementKind;

use crate::states::day_end::DayEndShareText;
use crate::states::day_end::helpers::{day_title, generate_pro_tip};

/// All computed values for the day-end summary screen.
///
/// Built once by [`prepare_day_end_report`] and consumed by the UI
/// spawning systems, so data extraction is fully separated from presentation.
#[derive(Resource)]
pub struct DayEndReport {
    pub day: u32,
    pub title_text: String,
    pub subtitle_text: String,
    pub pro_tip_text: String,

    // Financial line items (from DailyRecord)
    pub charging_revenue: f32,
    pub total_revenue: f32,
    pub total_income: f32,
    pub carbon_credits: f32,
    pub solar_export_revenue: f32,
    pub energy_cost: f32,
    pub demand_charge: f32,
    pub repair_parts: f32,
    pub repair_labor: f32,
    pub maintenance: f32,
    pub amenity: f32,
    pub opex: f32,
    pub cable_theft_cost: f32,
    pub warranty_cost: f32,
    pub warranty_recovery: f32,
    pub refunds: f32,
    pub penalties: f32,
    pub rent: f32,
    pub upgrades: f32,
    pub net_profit: f32,

    // Session / reputation stats
    pub sessions_delta: i32,
    pub sessions_failed_today: i32,
    pub dispatches_delta: i32,
    pub reputation_delta: i32,
    pub reputation: i32,

    // Charger / transformer stats
    pub chargers_total: i32,
    pub chargers_online: i32,
    pub pending_cable_thefts: u32,
    pub pending_cable_cost: f32,
    pub destroyed_transformers: i32,

    // Character / avatar
    pub avatar_handle: Handle<Image>,
    pub char_name: String,
    pub char_role: String,

    // Achievements
    pub top_badge_data: Option<(AchievementKind, Handle<Image>)>,

    // Aggregated KPI values
    pub total_energy: f32,
    pub total_opex: f32,
    pub total_fixed: f32,
    pub total_expenses: f32,
    pub operating_profit: f32,
    pub profit_color: Color,
    pub rep_color: Color,

    // Per-kWh pricing
    pub avg_sell_price_kwh: f32,
    pub avg_buy_price_kwh: f32,

    // Solar / grid events
    pub has_solar: bool,
    pub grid_event_revenue: f32,
    pub grid_event_import_surcharge: f32,
    pub best_spike: Option<(&'static str, f32)>,

    // Collapsed-view hints
    pub revenue_hint: Option<String>,
    pub expense_hint: Option<String>,

    // Unit economy
    pub revenue_per_session: f32,
    pub cost_per_session: f32,
}

/// Flush the ledger, build the DailyRecord, and compute every value the
/// day-end UI needs. Runs as the first system in the `OnEnter(DayEnd)` chain.
pub(crate) fn prepare_day_end_report(
    mut commands: Commands,
    game_clock: Res<crate::resources::GameClock>,
    mut game_state: ResMut<crate::resources::GameState>,
    mut multi_site: ResMut<crate::resources::MultiSiteManager>,
    carbon_market: Res<crate::resources::site_energy::CarbonCreditMarket>,
    player_profile: Res<crate::resources::PlayerProfile>,
    image_assets: Res<crate::resources::ImageAssets>,
    achievement_state: Res<crate::resources::achievements::AchievementState>,
    achievement_snapshot: Option<Res<crate::resources::achievements::AchievementSnapshot>>,
    chargers: Query<&crate::components::charger::Charger>,
    transformers: Query<&crate::components::power::Transformer>,
) {
    info!("Day {} complete!", game_clock.day);

    // Calculate carbon credits from energy delivered at the active site
    let rate_per_kwh = carbon_market.rate_per_kwh();
    let total_carbon_credits = if let Some(site_state) = multi_site.active_site_mut() {
        let carbon_credit_revenue = site_state.energy_delivered_kwh_today * rate_per_kwh;

        if site_state.energy_delivered_kwh_today > 0.0
            && site_state.utility_meter.total_imported_kwh() == 0.0
        {
            game_state.zero_grid_day_achieved = true;
        }

        site_state.energy_delivered_kwh_today = 0.0;
        site_state.sessions_today = 0;
        carbon_credit_revenue
    } else {
        0.0
    };

    // Flush all accumulated per-site costs to the ledger before verification
    game_state.flush_site_costs(&mut multi_site.owned_sites);

    // Add carbon credit revenue to game state
    game_state.add_carbon_credit_revenue(total_carbon_credits);

    info!(
        "Carbon credits: {:.1} kWh delivered at ${:.2}/kWh = ${:.2}",
        total_carbon_credits / rate_per_kwh,
        rate_per_kwh,
        total_carbon_credits
    );

    // Verify ledger cash balance matches game state
    if let Err(e) = game_state.ledger.verify_cash(game_state.cash) {
        error!("Ledger balance verification failed: {e}");
    }

    // Build DailyRecord from the ledger (financial fields) + tracker (non-financial stats)
    let site_id = game_state
        .daily_history
        .current_day
        .site_id
        .unwrap_or(crate::resources::multi_site::SiteId(1));
    let reputation_change =
        game_state.reputation - game_state.daily_history.current_day.starting_reputation;
    let financials = game_state
        .ledger
        .daily_totals(game_state.ledger.current_date);

    let daily_record = crate::resources::game_state::DailyRecord::from_ledger(
        &financials,
        game_clock.day,
        game_clock.month,
        game_clock.year,
        site_id,
        game_state.daily_history.current_day.sessions,
        game_state.daily_history.current_day.sessions_failed_today,
        game_state.daily_history.current_day.dispatches,
        reputation_change,
    );

    // Extract values for display (before pushing to history)
    let charging_revenue = daily_record.financials.charging_revenue;
    let total_revenue = daily_record.total_revenue();
    let carbon_credits = daily_record.financials.carbon_credits;
    let solar_export_revenue = daily_record.financials.solar_export_revenue;
    let energy_cost = daily_record.financials.energy_cost;
    let demand_charge = daily_record.financials.demand_charge;
    let repair_parts = daily_record.financials.repair_parts;
    let repair_labor = daily_record.financials.repair_labor;
    let maintenance = daily_record.financials.maintenance;
    let amenity = daily_record.financials.amenity;
    let opex = repair_parts + repair_labor + maintenance + amenity;
    let cable_theft_cost = daily_record.financials.cable_theft_cost;
    let warranty_cost = daily_record.financials.warranty_cost;
    let warranty_recovery = daily_record.financials.warranty_recovery;
    let refunds = daily_record.financials.refunds;
    let penalties = daily_record.financials.penalties;
    let rent = daily_record.financials.rent;
    let upgrades = daily_record.financials.upgrades;
    let net_profit = daily_record.net_profit();
    let sessions_delta = daily_record.sessions;
    let sessions_failed_today = daily_record.sessions_failed_today;
    let dispatches_delta = daily_record.dispatches;
    let reputation_delta = daily_record.reputation_change;

    // Snapshot charger online/total counts
    let chargers_total = chargers.iter().count() as i32;
    let chargers_online = chargers
        .iter()
        .filter(|c| {
            !matches!(
                c.state(),
                crate::components::charger::ChargerState::Offline
                    | crate::components::charger::ChargerState::Disabled
            )
        })
        .count() as i32;

    let pending_cable_thefts: u32 = chargers
        .iter()
        .filter(|c| {
            matches!(
                c.current_fault,
                Some(crate::components::charger::FaultType::CableTheft)
            )
        })
        .count() as u32;
    let pending_cable_cost = pending_cable_thefts as f32 * 2000.0;

    let destroyed_transformers = transformers.iter().filter(|t| t.destroyed).count() as i32;

    // Store the record in history
    game_state.daily_history.records.push(daily_record);

    // Get the character avatar handle
    let avatar_handle = match player_profile.character {
        Some(crate::resources::player_profile::CharacterKind::Ant) => {
            image_assets.character_main_ant.clone()
        }
        Some(crate::resources::player_profile::CharacterKind::Mallard) => {
            image_assets.character_main_mallard.clone()
        }
        Some(crate::resources::player_profile::CharacterKind::Raccoon) => {
            image_assets.character_main_raccoon.clone()
        }
        None => image_assets.character_main_ant.clone(),
    };

    let char_name = player_profile
        .character
        .map(|c| c.display_name())
        .unwrap_or("Player");
    let char_role = player_profile
        .character
        .map(|c| c.role())
        .unwrap_or("Operator");

    // Find achievements newly unlocked today (highest tier first)
    let empty_snapshot = std::collections::HashSet::new();
    let snapshot = achievement_snapshot
        .as_ref()
        .map(|s| &s.unlocked_at_day_start)
        .unwrap_or(&empty_snapshot);
    let new_badges = achievement_state.newly_unlocked_since(snapshot);

    // Compose LinkedIn share text
    let total_income = total_revenue + carbon_credits;
    let share_text = format!(
        "I just finished Day {} of Kilowatt Tycoon ⚡\n\n${:.0} in revenue\n{} sessions\n{} dispatches\n\nIs this really what it feels like to run an EV charging empire?\n\n#KilowattTycoon #EVCharging",
        game_clock.day, total_income, sessions_delta, dispatches_delta,
    );
    commands.insert_resource(DayEndShareText(share_text));

    // Precompute KPI aggregates
    let total_energy = financials.category_total(crate::resources::ledger::ExpenseCategory::Energy);
    let total_opex =
        financials.category_total(crate::resources::ledger::ExpenseCategory::Operations);
    let total_fixed = financials.category_total(crate::resources::ledger::ExpenseCategory::Fixed);
    let total_expenses = total_energy + total_opex;
    let operating_profit = total_income - total_expenses;

    let profit_color = if operating_profit >= 0.0 {
        Color::srgb(0.4, 0.9, 0.4)
    } else {
        Color::srgb(0.9, 0.4, 0.4)
    };
    let rep_color = if reputation_delta >= 0 {
        Color::srgb(0.4, 0.9, 0.4)
    } else {
        Color::srgb(0.9, 0.4, 0.4)
    };

    // Compute day title and pro-tip
    let (title_text, subtitle_text) = day_title(
        game_clock.day,
        net_profit,
        reputation_delta,
        sessions_delta,
        charging_revenue,
        energy_cost,
        opex,
        warranty_cost,
        warranty_recovery,
    );
    let pro_tip_text = generate_pro_tip(
        char_name,
        net_profit,
        sessions_delta,
        charging_revenue,
        energy_cost,
        opex,
        reputation_delta,
        warranty_cost,
        warranty_recovery,
        destroyed_transformers,
    );

    // Per-kWh pricing for expanded view
    let total_imported_kwh: f32 = multi_site
        .owned_sites
        .values()
        .map(|s| s.utility_meter.total_imported_kwh())
        .sum();
    let avg_sell_price_kwh: f32 = if total_imported_kwh > 0.01 {
        charging_revenue / total_imported_kwh
    } else {
        0.0
    };
    let avg_buy_price_kwh: f32 = if total_imported_kwh > 0.01 {
        energy_cost / total_imported_kwh
    } else {
        0.0
    };

    let has_solar = multi_site
        .owned_sites
        .values()
        .any(|s| s.grid.total_solar_kw > 0.0);

    // Grid event stats (only meaningful for challenge_level >= 2)
    let grid_event_revenue: f32 = multi_site
        .active_site()
        .filter(|s| s.challenge_level >= 2)
        .map(|s| s.grid_events.event_revenue_today)
        .unwrap_or(0.0);
    let grid_event_import_surcharge: f32 = multi_site
        .active_site()
        .filter(|s| s.challenge_level >= 2)
        .map(|s| s.grid_events.event_import_surcharge_today)
        .unwrap_or(0.0);
    let best_spike: Option<(&'static str, f32)> = multi_site
        .active_site()
        .filter(|s| s.challenge_level >= 2)
        .and_then(|s| {
            s.grid_events
                .best_event_type
                .map(|e| (e.name(), s.grid_events.best_event_export_multiplier))
        });

    // Revenue/cost hints for collapsed view
    let revenue_hint: Option<String> = if charging_revenue < energy_cost && energy_cost > 0.01 {
        Some("(Pricing: Too Low?)".to_string())
    } else {
        None
    };
    let expense_hint: Option<String> = if total_expenses < 0.01 {
        None
    } else {
        let repairs = repair_parts + repair_labor;
        let categories: [(&str, f32); 4] = [
            ("energy", total_energy),
            ("repairs", repairs),
            ("maintenance", maintenance),
            ("amenities", amenity),
        ];
        categories
            .iter()
            .max_by(|a, b| a.1.partial_cmp(&b.1).unwrap_or(std::cmp::Ordering::Equal))
            .filter(|(_, v)| *v > 0.01)
            .map(|(name, _)| {
                match *name {
                    "energy" => "(Feeding the Grid)",
                    "repairs" => "(Duct Tape Budget)",
                    "maintenance" => "(Keeping the Lights On)",
                    "amenities" => "(The Snack Tax)",
                    _ => "(Misc.)",
                }
                .to_string()
            })
    };

    // Unit economy (per-session)
    let revenue_per_session = if sessions_delta > 0 {
        charging_revenue / sessions_delta as f32
    } else {
        0.0
    };
    let cost_per_session = if sessions_delta > 0 {
        energy_cost / sessions_delta as f32
    } else {
        0.0
    };

    // Get the highest-tier badge earned today
    let top_badge_data: Option<(AchievementKind, Handle<Image>)> =
        new_badges.first().map(|badge| {
            let icon = match badge.tier() {
                crate::resources::achievements::AchievementTier::Bronze => {
                    image_assets.icon_medal_bronze.clone()
                }
                crate::resources::achievements::AchievementTier::Silver => {
                    image_assets.icon_medal_silver.clone()
                }
                crate::resources::achievements::AchievementTier::Gold => {
                    image_assets.icon_medal_gold.clone()
                }
            };
            (*badge, icon)
        });

    commands.insert_resource(DayEndReport {
        day: game_clock.day,
        title_text: title_text.to_string(),
        subtitle_text: subtitle_text.to_string(),
        pro_tip_text,
        charging_revenue,
        total_revenue,
        total_income,
        carbon_credits,
        solar_export_revenue,
        energy_cost,
        demand_charge,
        repair_parts,
        repair_labor,
        maintenance,
        amenity,
        opex,
        cable_theft_cost,
        warranty_cost,
        warranty_recovery,
        refunds,
        penalties,
        rent,
        upgrades,
        net_profit,
        sessions_delta,
        sessions_failed_today,
        dispatches_delta,
        reputation_delta,
        reputation: game_state.reputation,
        chargers_total,
        chargers_online,
        pending_cable_thefts,
        pending_cable_cost,
        destroyed_transformers,
        avatar_handle,
        char_name: char_name.to_string(),
        char_role: char_role.to_string(),
        top_badge_data,
        total_energy,
        total_opex,
        total_fixed,
        total_expenses,
        operating_profit,
        profit_color,
        rep_color,
        avg_sell_price_kwh,
        avg_buy_price_kwh,
        has_solar,
        grid_event_revenue,
        grid_event_import_surcharge,
        best_spike,
        revenue_hint,
        expense_hint,
        revenue_per_session,
        cost_per_session,
    });
}
