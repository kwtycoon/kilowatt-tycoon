//! Game state resource for economy and win/lose tracking

use bevy::prelude::*;

use crate::resources::multi_site::SiteId;

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

    // Income (detailed breakdown)
    pub charging_revenue: f32, // Revenue from charging sessions (kWh * price)
    pub ad_revenue: f32,       // Revenue from video advertisements
    pub carbon_credits: f32,   // Carbon credit revenue
    pub solar_export_revenue: f32, // Revenue from selling solar back to the grid

    // Expenses
    pub energy_cost: f32,      // TOU energy costs
    pub demand_charge: f32,    // Peak demand charge
    pub opex: f32,             // Technician dispatch, repairs, etc.
    pub cable_theft_cost: f32, // Cable replacement costs from theft
    pub refunds: f32,          // Customer refunds
    pub penalties: f32,        // Ticket escalation penalties

    // Stats
    pub sessions: i32,
    pub sessions_failed_today: i32, // Daily session failures (angry drivers)
    pub dispatches: i32,
    pub reputation_change: i32,
}

impl DailyRecord {
    /// Total revenue (charging + ads + solar export)
    pub fn total_revenue(&self) -> f32 {
        self.charging_revenue + self.ad_revenue + self.solar_export_revenue
    }

    pub fn net_profit(&self) -> f32 {
        self.total_revenue() + self.carbon_credits
            - self.energy_cost
            - self.demand_charge
            - self.opex
            - self.cable_theft_cost
            - self.refunds
            - self.penalties
    }
}

/// Tracks the current in-progress day
#[derive(Debug, Clone, Default)]
pub struct CurrentDayTracker {
    pub site_id: Option<SiteId>,
    pub charging_revenue: f32, // Revenue from charging sessions
    pub ad_revenue: f32,       // Revenue from video advertisements
    pub carbon_credits: f32,
    pub solar_export_revenue: f32, // Revenue from selling solar back to the grid
    pub energy_cost: f32,
    pub demand_charge: f32,
    pub opex: f32,
    pub cable_theft_cost: f32,
    pub refunds: f32,
    pub penalties: f32,
    pub sessions: i32,
    pub sessions_failed_today: i32, // Daily session failures (angry drivers)
    pub dispatches: i32,
    pub starting_reputation: i32,
}

impl CurrentDayTracker {
    /// Total revenue (charging + ads + solar export)
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
    // Economy
    pub cash: f32,
    pub net_revenue: f32,
    pub gross_revenue: f32,
    pub total_refunds: f32,
    pub total_penalties: f32,
    pub total_opex: f32,

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
    /// Whether the guaranteed day 1 technician fault has been injected
    pub first_technician_fault_injected: bool,
    /// Random session count target for day 1 fault (None = not yet determined)
    pub day1_fault_target_session: Option<i32>,

    // Achievement tracking
    /// Total energy delivered across all chargers (kWh) -- for 1.21 Gigawatts achievement
    pub total_energy_delivered_kwh: f32,
    /// Consecutive commercial fleet sessions completed without a charger fault
    pub fleet_sessions_without_fault: u32,
    /// Whether demand ever exceeded grid capacity (before throttling)
    pub grid_overload_triggered: bool,
    /// Whether any site ran a full day on solar+battery with zero grid import
    pub zero_grid_day_achieved: bool,

    // Daily financial history
    pub daily_history: DailyHistory,
}

impl Default for GameState {
    fn default() -> Self {
        Self {
            cash: STARTING_CASH,
            net_revenue: 0.0,
            gross_revenue: 0.0,
            total_refunds: 0.0,
            total_penalties: 0.0,
            total_opex: 0.0,
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
        }
    }
}

impl GameState {
    /// Add charging revenue from a completed session
    pub fn add_charging_revenue(&mut self, amount: f32) {
        self.gross_revenue += amount;
        self.net_revenue += amount;
        self.cash += amount;
        self.daily_history.current_day.charging_revenue += amount;
        self.daily_history.current_day.sessions += 1;
    }

    /// Add ad revenue (accumulated during charging, no session increment)
    pub fn add_ad_revenue(&mut self, amount: f32) {
        self.gross_revenue += amount;
        self.net_revenue += amount;
        self.cash += amount;
        self.daily_history.current_day.ad_revenue += amount;
    }

    pub fn add_refund(&mut self, amount: f32) {
        self.total_refunds += amount;
        self.net_revenue -= amount;
        self.cash -= amount;
        self.daily_history.current_day.refunds += amount;
    }

    pub fn add_penalty(&mut self, amount: f32) {
        self.total_penalties += amount;
        self.net_revenue -= amount;
        self.cash -= amount;
        self.daily_history.current_day.penalties += amount;
    }

    pub fn add_energy_cost(&mut self, amount: f32) {
        self.net_revenue -= amount;
        self.cash -= amount;
        self.daily_history.current_day.energy_cost += amount;
    }

    pub fn add_demand_charge(&mut self, amount: f32) {
        self.net_revenue -= amount;
        self.cash -= amount;
        self.daily_history.current_day.demand_charge += amount;
    }

    pub fn add_opex(&mut self, amount: f32) {
        self.total_opex += amount;
        self.net_revenue -= amount;
        self.cash -= amount;
        self.daily_history.current_day.opex += amount;
    }

    pub fn add_cable_theft_cost(&mut self, amount: f32) {
        self.total_opex += amount;
        self.net_revenue -= amount;
        self.cash -= amount;
        self.daily_history.current_day.cable_theft_cost += amount;
    }

    /// Record a technician dispatch (increment daily counter)
    pub fn record_dispatch(&mut self) {
        self.daily_history.current_day.dispatches += 1;
    }

    pub fn add_carbon_credit_revenue(&mut self, amount: f32) {
        self.net_revenue += amount;
        self.cash += amount;
        self.daily_history.current_day.carbon_credits += amount;
    }

    pub fn add_solar_export_revenue(&mut self, amount: f32) {
        self.net_revenue += amount;
        self.cash += amount;
        self.daily_history.current_day.solar_export_revenue += amount;
    }

    pub fn change_reputation(&mut self, delta: i32) {
        self.reputation = (self.reputation + delta).clamp(0, 100);
    }

    pub fn reset(&mut self) {
        *self = Self::default();
    }

    // ============ Build Phase Economy ============

    /// Try to spend money on building, returns true if successful
    pub fn try_spend_build(&mut self, amount: i32) -> bool {
        let cost = amount as f32;
        if self.cash >= cost {
            self.cash -= cost;
            true
        } else {
            false
        }
    }

    /// Refund money when bulldozing (50% of original cost)
    pub fn refund_build(&mut self, amount: i32) {
        self.cash += (amount / 2) as f32;
    }

    /// Check if can afford a build cost
    pub fn can_afford_build(&self, amount: i32) -> bool {
        self.cash >= amount as f32
    }

    // ============ Leaderboard Score Calculation ============

    /// Calculate the score for a single day
    /// Score is simply the net profit/loss for the day
    pub fn calculate_daily_score(&self, daily_record: &DailyRecord) -> i64 {
        // Score = net profit (positive for profit, negative for loss)
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
