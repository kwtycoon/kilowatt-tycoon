//! Game state resource for economy and win/lose tracking

use bevy::prelude::*;

use std::collections::HashMap;

use crate::resources::ledger::{self, Account, Ledger};
use crate::resources::multi_site::{SiteId, SiteState};

/// Constants from MVP spec
pub const STARTING_CASH: f32 = 1_000_000.0;
pub const STARTING_REPUTATION: i32 = 50;

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
    /// Total revenue (charging + ads + solar export)
    pub fn total_revenue(&self) -> f32 {
        self.financials.charging_revenue
            + self.financials.ad_revenue
            + self.financials.solar_export_revenue
    }

    pub fn net_profit(&self) -> f32 {
        self.total_revenue() + self.financials.carbon_credits
            - self.financials.energy_cost
            - self.financials.demand_charge
            - self.financials.opex
            - self.financials.cable_theft_cost
            - self.financials.warranty_cost
            + self.financials.warranty_recovery
            - self.financials.refunds
            - self.financials.penalties
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

    pub fn add_opex(&mut self, amount: f32) {
        self.ledger
            .record_expense(amount, Account::Opex, "Operating expense");
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

    pub fn change_reputation(&mut self, delta: i32) {
        self.reputation = (self.reputation + delta).clamp(0, 100);
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
            if site.pending_opex > 0.0 {
                self.ledger
                    .record_expense(site.pending_opex, Account::Opex, "Operating expense");
                site.pending_opex = 0.0;
            }
            if site.pending_warranty > 0.0 {
                self.ledger.record_expense(
                    site.pending_warranty,
                    Account::Warranty,
                    "Warranty premium",
                );
                site.pending_warranty = 0.0;
            }
        }
        self.cash = self.ledger.cash_balance_f32();
    }

    // ============ Leaderboard Score Calculation ============

    /// Calculate the score for a single day
    pub fn calculate_daily_score(&self, daily_record: &DailyRecord) -> i64 {
        daily_record.net_profit() as i64
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
