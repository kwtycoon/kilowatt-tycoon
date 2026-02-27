//! Demand charge related events

use bevy::prelude::*;

/// Event fired when peak demand increases
#[derive(Event, Message, Debug, Clone)]
pub struct PeakIncreasedEvent {
    /// Previous peak (kW)
    pub old_peak_kw: f32,
    /// New peak (kW)
    pub new_peak_kw: f32,
    /// Demand rate ($/kW)
    pub demand_rate: f32,
    /// Time when peak was set (game time)
    pub game_time: f32,
}

impl PeakIncreasedEvent {
    /// Calculate the cost increase
    pub fn cost_increase(&self) -> f32 {
        (self.new_peak_kw - self.old_peak_kw) * self.demand_rate
    }
}

/// Event fired when load approaches current peak (risk of new peak)
#[derive(Event, Message, Debug, Clone)]
pub struct PeakRiskEvent {
    /// Current load (kW)
    pub current_load_kw: f32,
    /// Current peak threshold (kW)
    pub peak_kw: f32,
    /// Percentage of peak (0.0-1.0+)
    pub percentage: f32,
}

/// Event fired when BESS successfully prevents a peak increase
#[derive(Event, Message, Debug, Clone)]
pub struct BessSavedPeakEvent {
    /// Load before BESS intervention (kW)
    pub load_before_kw: f32,
    /// Load after BESS intervention (kW)
    pub load_after_kw: f32,
    /// Peak that was prevented (kW)
    pub prevented_peak_kw: f32,
    /// Demand charge saved ($)
    pub savings: f32,
}

/// Event fired when BESS is low on charge during peak hours
#[derive(Event, Message, Debug, Clone)]
pub struct BessLowSocEvent {
    /// Current state of charge (%)
    pub soc_percent: f32,
    /// Current load (kW)
    pub current_load_kw: f32,
    /// Peak threshold (kW)
    pub peak_kw: f32,
    /// Whether BESS can still provide protection
    pub can_protect: bool,
}

/// Event fired when demand charges become a significant burden vs margin/revenue.
///
/// This is intended to be a *player-facing* alert signal (not a technical "new peak set").
#[derive(Event, Message, Debug, Clone)]
pub struct DemandBurdenEvent {
    pub site_id: crate::resources::SiteId,
    /// Current demand charge ($) for the site
    pub demand_charge: f32,
    /// Current energy cost ($) for the site
    pub energy_cost: f32,
    /// Site revenue today ($)
    pub revenue_today: f32,
    /// Margin proxy ($) = revenue_today - energy_cost (clamped >= 1)
    pub margin: f32,
    /// demand_charge / revenue_today (revenue clamped to >= 1)
    pub demand_share: f32,
    /// Current grid draw (kVA) for context
    pub grid_kva: f32,
    /// Current peak demand (kW) for context
    pub peak_kw: f32,
    /// Demand rate ($/kW)
    pub demand_rate: f32,
}
