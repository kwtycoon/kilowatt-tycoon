//! Game state resource for economy and win/lose tracking

use bevy::prelude::*;

use std::collections::HashMap;

use crate::resources::ledger::{self, Account, Ledger};
use crate::resources::multi_site::{SiteId, SiteState};

/// Constants from MVP spec
pub const STARTING_CASH: f32 = 1_000_000.0;
pub const STARTING_REPUTATION: i32 = 50;

pub const REPUTATION_GREEN_THRESHOLD: i32 = 80;
pub const REPUTATION_RED_THRESHOLD: i32 = 40;

/// Map an absolute reputation score to a UI color tier.
pub fn reputation_color(rep: i32) -> Color {
    if rep >= REPUTATION_GREEN_THRESHOLD {
        Color::srgb(0.4, 0.9, 0.4)
    } else if rep < REPUTATION_RED_THRESHOLD {
        Color::srgb(0.9, 0.4, 0.4)
    } else {
        Color::srgb(0.9, 0.7, 0.4)
    }
}

// ============ Reputation Tracking ============

const REPUTATION_CATEGORY_COUNT: usize = 7;

const REPUTATION_CATEGORY_NAMES: [&str; REPUTATION_CATEGORY_COUNT] = [
    "Charging Sessions",
    "Angry Drivers",
    "Charger Faults",
    "Ticket Escalations",
    "Repairs",
    "Fleet Breaches",
    "Transformer Fires",
];

/// Every reputation change carries its own score. Fixed-score variants
/// (e.g. `ChargingSession` is always +2) cannot be mis-stated at the call
/// site; variable-score variants carry the value in their payload.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ReputationSource {
    /// Successful charging session — always +2.
    ChargingSession,
    /// Driver left angry — variable penalty (e.g. -3 normal, -5 heatwave/jam).
    AngryDriver(i32),
    /// Charger faulted during an active session — always -4.
    ChargerFault,
    /// Support ticket breached its SLA — always -5.
    TicketEscalation,
    /// Technician or remote fault fix — always +1.
    Repair,
    /// Fleet contract breach — penalty defined by contract.
    FleetBreach(i32),
    /// Transformer caught fire — always -10.
    TransformerFire,
}

impl ReputationSource {
    /// The reputation delta this event represents.
    pub fn delta(self) -> i32 {
        match self {
            Self::ChargingSession => 2,
            Self::AngryDriver(d) => d,
            Self::ChargerFault => -4,
            Self::TicketEscalation => -5,
            Self::Repair => 1,
            Self::FleetBreach(d) => d,
            Self::TransformerFire => -10,
        }
    }

    fn category_index(&self) -> usize {
        match self {
            Self::ChargingSession => 0,
            Self::AngryDriver(_) => 1,
            Self::ChargerFault => 2,
            Self::TicketEscalation => 3,
            Self::Repair => 4,
            Self::FleetBreach(_) => 5,
            Self::TransformerFire => 6,
        }
    }
}

/// Per-day accumulator keyed by reputation category. Fixed-size, no allocation.
#[derive(Debug, Clone)]
pub struct ReputationBreakdown {
    totals: [i32; REPUTATION_CATEGORY_COUNT],
}

impl Default for ReputationBreakdown {
    fn default() -> Self {
        Self {
            totals: [0; REPUTATION_CATEGORY_COUNT],
        }
    }
}

impl ReputationBreakdown {
    pub fn record(&mut self, source: ReputationSource) {
        self.totals[source.category_index()] += source.delta();
    }

    pub fn net_change(&self) -> i32 {
        self.totals.iter().sum()
    }

    /// Yields `(display_name, accumulated_delta)` for every category with a non-zero total.
    pub fn iter_nonzero(&self) -> impl Iterator<Item = (&'static str, i32)> + '_ {
        self.totals
            .iter()
            .copied()
            .enumerate()
            .filter_map(|(i, v)| {
                if v != 0 {
                    Some((REPUTATION_CATEGORY_NAMES[i], v))
                } else {
                    None
                }
            })
    }
}

/// End game result
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GameResult {
    InProgress,
    Won,
    LostBankruptcy,
    LostReputation,
    LostTimeout,
}

impl GameResult {
    pub fn is_ended(&self) -> bool {
        !matches!(self, GameResult::InProgress)
    }

    pub fn title(&self) -> &'static str {
        match self {
            GameResult::InProgress => "",
            GameResult::Won => "YOU WIN!",
            GameResult::LostBankruptcy => "OUT OF CASH",
            GameResult::LostReputation => "DRIVERS AVOID YOU",
            GameResult::LostTimeout => "TIME'S UP",
        }
    }
}

/// A single day's financial record
#[derive(Debug, Clone)]
pub struct DailyRecord {
    pub day: u32,
    pub month: u32,
    pub year: u32,
    pub site_id: SiteId,

    // All financial data comes from the ledger
    pub financials: ledger::DailyFinancials,

    // Stats (not tracked by ledger)
    pub sessions: i32,
    pub sessions_failed_today: i32,
    pub dispatches: i32,
    pub reputation_change: i32,
}

impl DailyRecord {
    /// Total revenue (charging + ads + solar export + fleet retainers)
    pub fn total_revenue(&self) -> f32 {
        self.financials.charging_revenue
            + self.financials.ad_revenue
            + self.financials.solar_export_revenue
            + self.financials.fleet_contract_revenue
    }

    pub fn net_profit(&self) -> f32 {
        self.total_revenue() + self.financials.carbon_credits
            - self.financials.energy_cost
            - self.financials.demand_charge
            - self.financials.total_opex_line()
            - self.financials.cable_theft_cost
            - self.financials.warranty_cost
            + self.financials.warranty_recovery
            - self.financials.refunds
            - self.financials.penalties
            - self.financials.fleet_penalty
            - self.financials.rent
            - self.financials.upgrades
    }

    /// Build a DailyRecord from ledger financials + tracker non-financial stats.
    pub fn from_ledger(
        financials: &ledger::DailyFinancials,
        day: u32,
        month: u32,
        year: u32,
        site_id: SiteId,
        sessions: i32,
        sessions_failed_today: i32,
        dispatches: i32,
        reputation_change: i32,
    ) -> Self {
        Self {
            day,
            month,
            year,
            site_id,
            financials: financials.clone(),
            sessions,
            sessions_failed_today,
            dispatches,
            reputation_change,
        }
    }
}

/// Tracks the current in-progress day.
///
/// Financial fields that appear here are only those needed for real-time
/// display during the day. All other financial data lives exclusively in
/// the ledger and is queried via `ledger.daily_totals()` at day-end.
#[derive(Debug, Clone, Default)]
pub struct CurrentDayTracker {
    pub site_id: Option<SiteId>,

    // Revenue fields kept for real-time `total_revenue()` display
    pub charging_revenue: f32,
    pub ad_revenue: f32,
    pub solar_export_revenue: f32,

    // Non-financial stats (not tracked by ledger)
    pub sessions: i32,
    pub sessions_failed_today: i32,
    pub dispatches: i32,
    pub starting_reputation: i32,

    /// Per-source reputation deltas accumulated during the day.
    pub reputation_breakdown: ReputationBreakdown,
}

impl CurrentDayTracker {
    /// Total revenue (charging + ads + solar export) for real-time display.
    pub fn total_revenue(&self) -> f32 {
        self.charging_revenue + self.ad_revenue + self.solar_export_revenue
    }
}

/// Resource tracking financial history
#[derive(Resource, Debug, Clone, Default)]
pub struct DailyHistory {
    pub records: Vec<DailyRecord>,
    pub current_day: CurrentDayTracker,
}

/// Central game state resource
#[derive(Resource, Debug, Clone)]
pub struct GameState {
    // Economy — cash is a real-time f32 cache verified against ledger at day-end
    pub cash: f32,

    // Reputation
    pub reputation: i32,

    // Session stats
    pub sessions_completed: i32,
    pub sessions_failed: i32,
    pub tickets_resolved: i32,
    pub tickets_escalated: i32,

    // Win/lose
    pub result: GameResult,

    // Tutorial state
    pub first_fault_seen: bool,
    pub first_ticket_seen: bool,
    pub first_session_completed: bool,
    pub first_technician_fault_injected: bool,
    pub day1_fault_target_session: Option<i32>,

    // Achievement tracking
    pub total_energy_delivered_kwh: f32,
    pub fleet_sessions_without_fault: u32,
    pub grid_overload_triggered: bool,
    pub zero_grid_day_achieved: bool,

    // Daily financial history
    pub daily_history: DailyHistory,

    // Double-entry accounting ledger (authoritative source of truth)
    pub ledger: Ledger,
}

impl Default for GameState {
    fn default() -> Self {
        Self {
            cash: STARTING_CASH,
            reputation: STARTING_REPUTATION,
            sessions_completed: 0,
            sessions_failed: 0,
            tickets_resolved: 0,
            tickets_escalated: 0,
            result: GameResult::InProgress,
            first_fault_seen: false,
            first_ticket_seen: false,
            first_session_completed: false,
            first_technician_fault_injected: false,
            day1_fault_target_session: None,
            total_energy_delivered_kwh: 0.0,
            fleet_sessions_without_fault: 0,
            grid_overload_triggered: false,
            zero_grid_day_achieved: false,
            daily_history: DailyHistory::default(),
            ledger: Ledger::with_opening_balance(STARTING_CASH),
        }
    }
}

impl GameState {
    // ============ Revenue ============

    /// Add charging revenue from a completed session
    pub fn add_charging_revenue(&mut self, amount: f32) {
        self.daily_history.current_day.charging_revenue += amount;
        self.daily_history.current_day.sessions += 1;
        self.ledger
            .record_revenue(amount, Account::Charging, "Charging session");
        self.cash = self.ledger.cash_balance_f32();
    }

    /// Add ad revenue (flushed at session end, not per-frame)
    pub fn add_ad_revenue(&mut self, amount: f32) {
        self.daily_history.current_day.ad_revenue += amount;
        self.ledger
            .record_revenue(amount, Account::Ads, "Video ad revenue");
        self.cash = self.ledger.cash_balance_f32();
    }

    pub fn add_carbon_credit_revenue(&mut self, amount: f32) {
        self.ledger
            .record_revenue(amount, Account::CarbonCredits, "Carbon credits");
        self.cash = self.ledger.cash_balance_f32();
    }

    pub fn add_solar_export_revenue(&mut self, amount: f32) {
        self.daily_history.current_day.solar_export_revenue += amount;
        self.ledger
            .record_revenue(amount, Account::SolarExport, "Solar grid export");
        self.cash = self.ledger.cash_balance_f32();
    }

    pub fn add_fleet_contract_revenue(&mut self, amount: f32, company_name: &str) {
        self.ledger.record_revenue(
            amount,
            Account::FleetContract,
            &format!("Fleet retainer: {company_name}"),
        );
        self.cash = self.ledger.cash_balance_f32();
    }

    pub fn add_fleet_penalty(&mut self, amount: f32, company_name: &str) {
        self.ledger.record_expense(
            amount,
            Account::FleetPenalty,
            &format!("Fleet penalty: {company_name}"),
        );
        self.cash = self.ledger.cash_balance_f32();
    }

    // ============ Expenses ============

    pub fn add_refund(&mut self, amount: f32) {
        self.ledger
            .record_expense(amount, Account::Refunds, "Customer refund");
        self.cash = self.ledger.cash_balance_f32();
    }

    pub fn add_penalty(&mut self, amount: f32) {
        self.ledger
            .record_expense(amount, Account::Penalties, "Ticket escalation penalty");
        self.cash = self.ledger.cash_balance_f32();
    }

    pub fn add_energy_cost(&mut self, amount: f32) {
        self.ledger
            .record_expense(amount, Account::Energy, "Grid electricity");
        self.cash = self.ledger.cash_balance_f32();
    }

    pub fn add_demand_charge(&mut self, amount: f32) {
        self.ledger
            .record_expense(amount, Account::DemandCharge, "Peak demand charge");
        self.cash = self.ledger.cash_balance_f32();
    }

    pub fn add_repair_parts(&mut self, amount: f32) {
        self.ledger
            .record_expense(amount, Account::RepairParts, "Repair parts");
        self.cash = self.ledger.cash_balance_f32();
    }

    pub fn add_repair_labor(&mut self, amount: f32) {
        self.ledger
            .record_expense(amount, Account::RepairLabor, "Repair labor");
        self.cash = self.ledger.cash_balance_f32();
    }

    pub fn add_maintenance(&mut self, amount: f32) {
        self.ledger
            .record_expense(amount, Account::Maintenance, "Site maintenance");
        self.cash = self.ledger.cash_balance_f32();
    }

    pub fn add_cable_theft_cost(&mut self, amount: f32) {
        self.ledger
            .record_expense(amount, Account::CableTheft, "Cable theft replacement");
        self.cash = self.ledger.cash_balance_f32();
    }

    pub fn add_warranty_cost(&mut self, amount: f32) {
        self.ledger
            .record_expense(amount, Account::Warranty, "Warranty premium");
        self.cash = self.ledger.cash_balance_f32();
    }

    /// Record warranty recovery (reduces OPEX via the ledger as a contra-expense).
    pub fn add_warranty_recovery(&mut self, amount: f32) {
        self.ledger
            .record_contra_expense(amount, Account::WarrantyRecovery, "Warranty coverage");
        self.cash = self.ledger.cash_balance_f32();
    }

    /// Record a technician dispatch (increment daily counter)
    pub fn record_dispatch(&mut self) {
        self.daily_history.current_day.dispatches += 1;
    }

    // ============ Previously untracked mutations ============

    /// Spend money on site rent.
    pub fn spend_rent(&mut self, amount: f32) {
        self.ledger
            .record_expense(amount, Account::Rent, "Site rent");
        self.cash = self.ledger.cash_balance_f32();
    }

    /// Spend money on a site upgrade (non-equipment).
    pub fn spend_upgrade(&mut self, amount: f32) {
        self.ledger
            .record_expense(amount, Account::Upgrades, "Site upgrade");
        self.cash = self.ledger.cash_balance_f32();
    }

    /// Refund from selling a site.
    pub fn refund_site_sale(&mut self, amount: f32) {
        self.ledger.record_capex_refund(amount, "Site sale refund");
        self.cash = self.ledger.cash_balance_f32();
    }

    // ============ Reputation ============

    /// Apply a reputation change. The delta is encoded in the [`ReputationSource`]
    /// variant itself, so fixed-score events cannot be mis-stated.
    pub fn record_reputation(&mut self, source: ReputationSource) {
        self.reputation = (self.reputation + source.delta()).clamp(0, 100);
        self.daily_history
            .current_day
            .reputation_breakdown
            .record(source);
    }

    pub fn reset(&mut self) {
        *self = Self::default();
    }

    /// Reset with a specific starting cash amount (used by template picker).
    pub fn reset_with_cash(&mut self, starting_cash: f32) {
        *self = Self {
            cash: starting_cash,
            ledger: Ledger::with_opening_balance(starting_cash),
            ..Self::default()
        };
    }

    // ============ Build Phase Economy ============

    /// Try to spend money on building, returns true if successful
    pub fn try_spend_build(&mut self, amount: i32) -> bool {
        let cost = amount as f32;
        if self.cash >= cost {
            self.ledger.record_capex(cost, "Equipment purchase");
            self.cash = self.ledger.cash_balance_f32();
            true
        } else {
            false
        }
    }

    /// Refund money when bulldozing (50% of original cost)
    pub fn refund_build(&mut self, amount: i32) {
        let refund = (amount / 2) as f32;
        self.ledger.record_capex_refund(refund, "Equipment sale");
        self.cash = self.ledger.cash_balance_f32();
    }

    /// Check if can afford a build cost
    pub fn can_afford_build(&self, amount: i32) -> bool {
        self.cash >= amount as f32
    }

    // ============ Day-End: Flush Accumulated Site Costs ============

    /// Flush all per-site costs accumulated during the day to the ledger.
    /// Called once at day-end before `verify_cash` and `daily_totals`.
    pub fn flush_site_costs(&mut self, sites: &mut HashMap<SiteId, SiteState>) {
        for site in sites.values_mut() {
            let meter = &site.utility_meter;
            if meter.total_energy_cost > 0.0 {
                self.ledger.record_expense(
                    meter.total_energy_cost,
                    Account::Energy,
                    "Grid electricity",
                );
            }
            if meter.demand_charge > 0.0 {
                self.ledger.record_expense(
                    meter.demand_charge,
                    Account::DemandCharge,
                    "Peak demand charge",
                );
            }
            if meter.total_export_revenue > 0.0 {
                self.daily_history.current_day.solar_export_revenue += meter.total_export_revenue;
                self.ledger.record_revenue(
                    meter.total_export_revenue,
                    Account::SolarExport,
                    "Solar grid export",
                );
            }
            if site.pending_maintenance > 0.0 {
                self.ledger.record_expense(
                    site.pending_maintenance,
                    Account::Maintenance,
                    "Site maintenance",
                );
                site.pending_maintenance = 0.0;
            }
            if site.pending_amenity > 0.0 {
                self.ledger.record_expense(
                    site.pending_amenity,
                    Account::Amenity,
                    "Amenity operating costs",
                );
                site.pending_amenity = 0.0;
            }
            if site.pending_warranty > 0.0 {
                self.ledger.record_expense(
                    site.pending_warranty,
                    Account::Warranty,
                    "Warranty premium",
                );
                site.pending_warranty = 0.0;
            }
            site.utility_meter.reset();
        }
        self.cash = self.ledger.cash_balance_f32();
    }

    // ============ Leaderboard Score Calculation ============

    /// Calculate the score for a single day.
    ///
    /// Excludes rent and upgrades so that fixed/one-time site costs
    /// don't penalise the leaderboard — those still appear in `net_profit()`.
    pub fn calculate_daily_score(&self, daily_record: &DailyRecord) -> i64 {
        let f = &daily_record.financials;
        let score = daily_record.total_revenue() + f.carbon_credits
            - f.energy_cost
            - f.demand_charge
            - f.total_opex_line()
            - f.cable_theft_cost
            - f.warranty_cost
            + f.warranty_recovery
            - f.refunds
            - f.penalties
            - f.fleet_penalty;
        score as i64
    }

    /// Calculate the cumulative score across all days
    pub fn calculate_cumulative_score(&self) -> i64 {
        self.daily_history
            .records
            .iter()
            .map(|record| self.calculate_daily_score(record))
            .sum()
    }
}

/// Tracks which charger entity is currently selected
#[derive(Resource, Debug, Clone, Default)]
pub struct SelectedChargerEntity(pub Option<Entity>);

/// Counter for generating unique ticket IDs
#[derive(Resource, Debug, Clone, Default)]
pub struct TicketCounter(pub u32);

impl TicketCounter {
    pub fn generate_next(&mut self) -> String {
        self.0 += 1;
        format!("TKT-{:04}", self.0)
    }
}
