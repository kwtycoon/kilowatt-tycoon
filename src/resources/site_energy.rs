//! Site energy resources for power dispatch, utility billing, solar, and battery storage.

use bevy::math::ops;
use bevy::prelude::*;
use std::collections::VecDeque;

/// Time-of-use period enumeration
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum TouPeriod {
    #[default]
    OffPeak,
    OnPeak,
}

impl TouPeriod {
    pub fn display_name(&self) -> &'static str {
        match self {
            TouPeriod::OffPeak => "Off-Peak",
            TouPeriod::OnPeak => "On-Peak",
        }
    }
}

/// Site energy configuration - tariff rates and day cycle settings
#[derive(Resource, Debug, Clone)]
pub struct SiteEnergyConfig {
    /// Length of a simulated day in game seconds (for solar curve)
    pub day_length_game_seconds: f32,
    /// Demand averaging window in game seconds (900 = 15 minutes)
    pub demand_window_seconds: f32,
    /// Off-peak energy rate ($/kWh)
    pub off_peak_rate: f32,
    /// On-peak energy rate ($/kWh)
    pub on_peak_rate: f32,
    /// Demand charge rate ($/kW of 15-min peak)
    pub demand_rate_per_kw: f32,
    /// Start of on-peak period as fraction of day (0.0-1.0)
    pub on_peak_start: f32,
    /// End of on-peak period as fraction of day (0.0-1.0)
    pub on_peak_end: f32,
    /// Off-peak solar export buyback rate ($/kWh) -- wholesale price
    pub off_peak_export_rate: f32,
    /// On-peak solar export buyback rate ($/kWh) -- higher demand period
    pub on_peak_export_rate: f32,
}

impl Default for SiteEnergyConfig {
    fn default() -> Self {
        Self {
            day_length_game_seconds: 86400.0, // 24 hours = 86400 game seconds
            demand_window_seconds: 900.0,     // 15 minutes
            off_peak_rate: 0.12,
            on_peak_rate: 0.28,
            demand_rate_per_kw: 15.0,
            on_peak_start: 0.375, // 9am in a 24h day
            on_peak_end: 0.875,   // 9pm in a 24h day
            off_peak_export_rate: 0.04,
            on_peak_export_rate: 0.08,
        }
    }
}

impl SiteEnergyConfig {
    /// Get the current TOU period based on game time
    pub fn current_tou_period(&self, game_time: f32) -> TouPeriod {
        let day_fraction =
            (game_time % self.day_length_game_seconds) / self.day_length_game_seconds;
        if day_fraction >= self.on_peak_start && day_fraction < self.on_peak_end {
            TouPeriod::OnPeak
        } else {
            TouPeriod::OffPeak
        }
    }

    /// Get the current energy rate based on TOU period
    pub fn current_rate(&self, game_time: f32) -> f32 {
        match self.current_tou_period(game_time) {
            TouPeriod::OffPeak => self.off_peak_rate,
            TouPeriod::OnPeak => self.on_peak_rate,
        }
    }

    /// Get the current solar export buyback rate based on TOU period
    pub fn current_export_rate(&self, game_time: f32) -> f32 {
        match self.current_tou_period(game_time) {
            TouPeriod::OffPeak => self.off_peak_export_rate,
            TouPeriod::OnPeak => self.on_peak_export_rate,
        }
    }

    /// Get solar generation factor (0.0-1.0) based on time of day
    /// Uses a bell curve peaking at "noon" (day_fraction = 0.5)
    pub fn solar_generation_factor(&self, game_time: f32) -> f32 {
        let day_fraction =
            (game_time % self.day_length_game_seconds) / self.day_length_game_seconds;

        // Solar only generates during daylight (roughly 6am-6pm, or 0.25-0.75 of day)
        let sunrise = 0.25;
        let sunset = 0.75;

        if day_fraction < sunrise || day_fraction > sunset {
            return 0.0;
        }

        // Normalize to 0-1 within daylight hours
        let daylight_fraction = (day_fraction - sunrise) / (sunset - sunrise);

        // Bell curve: sin^2 for smooth rise/fall
        let angle = daylight_fraction * std::f32::consts::PI;
        let s = ops::sin(angle);
        s * s
    }
}

/// Rolling demand sample for 15-minute average tracking
#[derive(Debug, Clone)]
struct DemandSample {
    pub game_time: f32,
    pub power_kw: f32,
}

/// Days in a billing period for demand charge amortization.
/// Demand charges are monthly; we project a daily cost by dividing by this.
pub const DAYS_PER_BILLING_PERIOD: f32 = 30.0;

/// Utility meter - tracks grid import and demand peaks
#[derive(Resource, Debug, Clone)]
pub struct UtilityMeter {
    /// Rolling samples for demand window averaging
    demand_samples: VecDeque<DemandSample>,
    /// Current rolling average (kW)
    pub current_avg_kw: f32,
    /// Peak 15-minute demand this session (kW)
    pub peak_demand_kw: f32,
    /// Total energy imported during off-peak (kWh)
    pub off_peak_kwh: f32,
    /// Total energy imported during on-peak (kWh)
    pub on_peak_kwh: f32,
    /// Current grid import (kW) - set by power dispatch
    pub current_grid_import_kw: f32,
    /// Accumulated utility cost this session
    pub total_energy_cost: f32,
    /// Projected daily demand charge (peak_demand_kw * rate / 30).
    /// This is the daily amortized portion of a monthly demand charge.
    /// Flushed to ledger at day-end.
    pub demand_charge: f32,
    /// Total energy exported to the grid (kWh)
    pub total_exported_kwh: f32,
    /// Accumulated revenue from solar export this session
    pub total_export_revenue: f32,
}

impl Default for UtilityMeter {
    fn default() -> Self {
        Self {
            demand_samples: VecDeque::new(),
            current_avg_kw: 0.0,
            peak_demand_kw: 0.0,
            off_peak_kwh: 0.0,
            on_peak_kwh: 0.0,
            current_grid_import_kw: 0.0,
            total_energy_cost: 0.0,
            demand_charge: 0.0,
            total_exported_kwh: 0.0,
            total_export_revenue: 0.0,
        }
    }
}

impl UtilityMeter {
    /// Add a demand sample and update rolling average
    pub fn add_sample(&mut self, game_time: f32, power_kw: f32, window_seconds: f32) {
        self.demand_samples.push_back(DemandSample {
            game_time,
            power_kw,
        });

        // Remove samples older than the window
        let cutoff = game_time - window_seconds;
        while let Some(front) = self.demand_samples.front() {
            if front.game_time < cutoff {
                self.demand_samples.pop_front();
            } else {
                break;
            }
        }

        // Calculate rolling average
        if self.demand_samples.is_empty() {
            self.current_avg_kw = 0.0;
        } else {
            let sum: f32 = self.demand_samples.iter().map(|s| s.power_kw).sum();
            self.current_avg_kw = sum / self.demand_samples.len() as f32;
        }

        // Update peak if current average exceeds it
        if self.current_avg_kw > self.peak_demand_kw {
            self.peak_demand_kw = self.current_avg_kw;
        }
    }

    /// Add imported energy and cost
    pub fn add_energy(&mut self, kwh: f32, tou_period: TouPeriod, rate: f32) {
        match tou_period {
            TouPeriod::OffPeak => self.off_peak_kwh += kwh,
            TouPeriod::OnPeak => self.on_peak_kwh += kwh,
        }
        self.total_energy_cost += kwh * rate;
    }

    /// Update demand charge based on current peak (projected daily portion of monthly charge)
    pub fn update_demand_charge(&mut self, demand_rate_per_kw: f32, multiplier: f32) {
        self.demand_charge =
            self.peak_demand_kw * demand_rate_per_kw * multiplier / DAYS_PER_BILLING_PERIOD;
    }

    /// Add exported energy and revenue
    pub fn add_export(&mut self, kwh: f32, rate: f32) {
        self.total_exported_kwh += kwh;
        self.total_export_revenue += kwh * rate;
    }

    /// Get total imported energy (kWh)
    pub fn total_imported_kwh(&self) -> f32 {
        self.off_peak_kwh + self.on_peak_kwh
    }

    /// Get total utility cost (energy + demand)
    pub fn total_cost(&self) -> f32 {
        self.total_energy_cost + self.demand_charge
    }

    /// Reset meter (for new session)
    pub fn reset(&mut self) {
        *self = Self::default();
    }
}

/// Solar array state - tracks installed capacity and current generation
#[derive(Resource, Debug, Clone)]
pub struct SolarState {
    /// Total installed solar capacity (kW peak)
    pub installed_kw_peak: f32,
    /// Current generation (kW) - updated by power dispatch
    pub current_generation_kw: f32,
    /// Total energy generated this session (kWh)
    pub total_generated_kwh: f32,
}

impl Default for SolarState {
    fn default() -> Self {
        Self {
            installed_kw_peak: 0.0,
            current_generation_kw: 0.0,
            total_generated_kwh: 0.0,
        }
    }
}

impl SolarState {
    /// Calculate current generation based on installed capacity and time of day
    pub fn update_generation(&mut self, generation_factor: f32) {
        self.current_generation_kw = self.installed_kw_peak * generation_factor;
    }

    /// Add installed solar capacity
    pub fn add_capacity(&mut self, kw: f32) {
        self.installed_kw_peak += kw;
    }

    /// Remove installed solar capacity
    pub fn remove_capacity(&mut self, kw: f32) {
        self.installed_kw_peak = (self.installed_kw_peak - kw).max(0.0);
    }
}

/// BESS operating mode
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum BessMode {
    #[default]
    PeakShaving,
    /// Charge during off-peak, discharge during on-peak (TOU schedule driven).
    TouArbitrage,
    /// Discharge to grid when a grid event makes export profitable, charge when idle.
    /// Requires challenge_level >= 2 (grid events active).
    GridExport,
    Backup,
    Manual,
}

impl BessMode {
    pub fn display_name(&self) -> &'static str {
        match self {
            BessMode::PeakShaving => "Peak Shaving",
            BessMode::TouArbitrage => "TOU Arbitrage",
            BessMode::GridExport => "Grid Export",
            BessMode::Backup => "Backup",
            BessMode::Manual => "Manual",
        }
    }

    pub fn next(&self) -> Self {
        match self {
            BessMode::PeakShaving => BessMode::TouArbitrage,
            BessMode::TouArbitrage => BessMode::GridExport,
            BessMode::GridExport => BessMode::Backup,
            BessMode::Backup => BessMode::Manual,
            BessMode::Manual => BessMode::PeakShaving,
        }
    }

    pub fn prev(&self) -> Self {
        match self {
            BessMode::PeakShaving => BessMode::Manual,
            BessMode::TouArbitrage => BessMode::PeakShaving,
            BessMode::GridExport => BessMode::TouArbitrage,
            BessMode::Backup => BessMode::GridExport,
            BessMode::Manual => BessMode::Backup,
        }
    }

    /// Returns `(peak_shave_threshold, charge_threshold)` defaults for this mode.
    /// Only `PeakShaving` actually uses these thresholds in the dispatch system.
    pub fn default_thresholds(&self) -> (f32, f32) {
        match self {
            BessMode::PeakShaving => (0.65, 0.35),
            _ => (0.65, 0.35),
        }
    }
}

/// Battery Energy Storage System (BESS) state
#[derive(Resource, Debug, Clone)]
pub struct BessState {
    /// Total installed capacity (kWh)
    pub capacity_kwh: f32,
    /// Maximum charge rate (kW)
    pub max_charge_kw: f32,
    /// Maximum discharge rate (kW)
    pub max_discharge_kw: f32,
    /// Current state of charge (kWh)
    pub soc_kwh: f32,
    /// Round-trip efficiency (0.0-1.0)
    pub round_trip_efficiency: f32,
    /// Current charge/discharge power (positive = discharge, negative = charge)
    pub current_power_kw: f32,
    /// Operating mode
    pub mode: BessMode,
    /// Total energy discharged this session (kWh)
    pub total_discharged_kwh: f32,
    /// Total energy charged this session (kWh)
    pub total_charged_kwh: f32,
    /// Peak shaving threshold as fraction of site capacity (0.0-1.0)
    pub peak_shave_threshold: f32,
    /// Charge threshold as fraction of site capacity (below this, charge during off-peak)
    pub charge_threshold: f32,
}

impl Default for BessState {
    fn default() -> Self {
        Self {
            capacity_kwh: 0.0,
            max_charge_kw: 0.0,
            max_discharge_kw: 0.0,
            soc_kwh: 0.0,
            round_trip_efficiency: 0.90,
            current_power_kw: 0.0,
            mode: BessMode::PeakShaving,
            total_discharged_kwh: 0.0,
            total_charged_kwh: 0.0,
            peak_shave_threshold: 0.65,
            charge_threshold: 0.35,
        }
    }
}

impl BessState {
    /// Get state of charge as percentage (0-100)
    pub fn soc_percent(&self) -> f32 {
        if self.capacity_kwh > 0.0 {
            (self.soc_kwh / self.capacity_kwh * 100.0).clamp(0.0, 100.0)
        } else {
            0.0
        }
    }

    /// Calculate available discharge energy (kWh)
    pub fn available_discharge_kwh(&self) -> f32 {
        self.soc_kwh
    }

    /// Calculate available charge headroom (kWh)
    pub fn available_charge_kwh(&self) -> f32 {
        self.capacity_kwh - self.soc_kwh
    }

    /// Discharge the battery (returns actual energy discharged in kWh)
    pub fn discharge(&mut self, requested_kwh: f32, delta_seconds: f32) -> f32 {
        let max_power_kwh = self.max_discharge_kw * (delta_seconds / 3600.0);
        let actual_kwh = requested_kwh.min(max_power_kwh).min(self.soc_kwh);

        self.soc_kwh -= actual_kwh;
        self.total_discharged_kwh += actual_kwh;
        self.current_power_kw = actual_kwh / (delta_seconds / 3600.0);

        actual_kwh
    }

    /// Charge the battery (returns actual energy charged in kWh)
    pub fn charge(&mut self, requested_kwh: f32, delta_seconds: f32) -> f32 {
        let max_power_kwh = self.max_charge_kw * (delta_seconds / 3600.0);
        let headroom = self.available_charge_kwh();
        // Account for efficiency losses during charging
        let actual_kwh = requested_kwh
            .min(max_power_kwh)
            .min(headroom / self.round_trip_efficiency);

        self.soc_kwh += actual_kwh * self.round_trip_efficiency;
        self.total_charged_kwh += actual_kwh;
        self.current_power_kw = -(actual_kwh / (delta_seconds / 3600.0));

        actual_kwh
    }

    /// Add installed BESS capacity
    pub fn add_capacity(&mut self, capacity_kwh: f32, max_power_kw: f32) {
        self.capacity_kwh += capacity_kwh;
        self.max_charge_kw += max_power_kw;
        self.max_discharge_kw += max_power_kw;
        // Start at 50% SOC
        self.soc_kwh += capacity_kwh * 0.5;
    }

    /// Remove installed BESS capacity
    pub fn remove_capacity(&mut self, capacity_kwh: f32, max_power_kw: f32) {
        self.capacity_kwh = (self.capacity_kwh - capacity_kwh).max(0.0);
        self.max_charge_kw = (self.max_charge_kw - max_power_kw).max(0.0);
        self.max_discharge_kw = (self.max_discharge_kw - max_power_kw).max(0.0);
        // Clamp SOC to new capacity
        self.soc_kwh = self.soc_kwh.min(self.capacity_kwh);
    }

    /// Reset BESS for new session
    pub fn reset(&mut self) {
        self.soc_kwh = self.capacity_kwh * 0.5;
        self.current_power_kw = 0.0;
        self.total_discharged_kwh = 0.0;
        self.total_charged_kwh = 0.0;
    }
}

/// Resource tracking the computed grid import after solar/BESS contributions
/// Tracks both real power (kW) for billing and apparent power (kVA) for infrastructure.
#[derive(Resource, Debug, Clone, Default)]
pub struct GridImport {
    /// Current grid import real power (kW) - what the utility meters for billing
    pub current_kw: f32,
    /// Current grid import apparent power (kVA) - what stresses infrastructure
    pub current_kva: f32,
    /// Site load before solar/BESS - real power (kW)
    pub gross_load_kw: f32,
    /// Site load before solar/BESS - apparent power (kVA)
    pub gross_load_kva: f32,
    /// Solar contribution (kW) - solar produces real power
    pub solar_kw: f32,
    /// BESS contribution (kW, positive = discharge reducing import)
    pub bess_kw: f32,
    /// Power being exported to the grid (kW) - excess solar sold back
    pub export_kw: f32,
}

impl GridImport {
    /// Calculate grid import from components (both kW and kVA)
    /// Note: Solar and BESS inject real power (kW), which reduces grid import
    /// but the power factor of the remaining load still applies.
    pub fn calculate(&mut self) {
        // Net real power = gross load - solar - BESS discharge
        // (BESS charging adds to grid import, so bess_kw negative = charging)
        let net_kw = self.gross_load_kw - self.solar_kw - self.bess_kw;

        if net_kw >= 0.0 {
            self.current_kw = net_kw;
            self.export_kw = 0.0;
        } else {
            // Surplus generation: export the excess
            self.current_kw = 0.0;
            self.export_kw = -net_kw;
        }

        // Apparent power import scales proportionally with real power reduction
        if self.gross_load_kw > 0.0 {
            let reduction_factor = self.current_kw / self.gross_load_kw;
            self.current_kva = self.gross_load_kva * reduction_factor;
        } else {
            self.current_kva = 0.0;
        }
    }
}

/// Type of grid event that temporarily modifies import and export rates.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GridEventType {
    RecordDemand,
    GeneratorTrip,
    TransmissionConstraint,
    HeatEmergency,
    UnexpectedPlantOutage,
    RenewableShortfall,
    GridCongestion,
}

impl GridEventType {
    pub const ALL: &[GridEventType] = &[
        GridEventType::RecordDemand,
        GridEventType::GeneratorTrip,
        GridEventType::TransmissionConstraint,
        GridEventType::HeatEmergency,
        GridEventType::UnexpectedPlantOutage,
        GridEventType::RenewableShortfall,
        GridEventType::GridCongestion,
    ];

    pub fn name(&self) -> &'static str {
        match self {
            GridEventType::RecordDemand => "Record Demand",
            GridEventType::GeneratorTrip => "Generator Trip",
            GridEventType::TransmissionConstraint => "Transmission Constraint",
            GridEventType::HeatEmergency => "Heat Emergency",
            GridEventType::UnexpectedPlantOutage => "Plant Outage",
            GridEventType::RenewableShortfall => "Renewable Shortfall",
            GridEventType::GridCongestion => "Grid Congestion",
        }
    }

    pub fn headline(&self, weather: crate::resources::WeatherType) -> &'static str {
        use crate::resources::WeatherType;
        match self {
            GridEventType::RecordDemand => match weather {
                WeatherType::Heatwave | WeatherType::Sunny => {
                    "Everyone's cranking the AC - grid maxed out!"
                }
                WeatherType::Cold => "Heaters on full blast - grid maxed out!",
                _ => "Record energy demand - grid maxed out!",
            },
            GridEventType::GeneratorTrip => "Major generator tripped offline - prices spiking!",
            GridEventType::TransmissionConstraint => "Power lines jammed - local rates surging!",
            GridEventType::HeatEmergency => "Heat wave declared - grid operator calls emergency!",
            GridEventType::UnexpectedPlantOutage => "Power plant down - wholesale prices soaring!",
            GridEventType::RenewableShortfall => {
                "Wind died, clouds rolled in - renewables tanking!"
            }
            GridEventType::GridCongestion => "Grid bottleneck - congestion fees kicking in!",
        }
    }

    /// Relative weight for this event given the current weather.
    /// A weight of 0.0 means the event cannot occur in that weather.
    pub fn weather_weight(&self, weather: crate::resources::WeatherType) -> f32 {
        use crate::resources::WeatherType;
        match self {
            GridEventType::HeatEmergency => match weather {
                WeatherType::Heatwave => 3.0,
                WeatherType::Sunny => 1.0,
                WeatherType::Overcast => 0.5,
                WeatherType::Rainy | WeatherType::Cold => 0.0,
            },
            GridEventType::RecordDemand => match weather {
                WeatherType::Heatwave => 2.0,
                WeatherType::Sunny => 1.5,
                WeatherType::Overcast => 1.0,
                WeatherType::Rainy | WeatherType::Cold => 0.5,
            },
            GridEventType::RenewableShortfall => match weather {
                WeatherType::Rainy => 2.0,
                WeatherType::Overcast => 1.5,
                WeatherType::Cold => 1.0,
                WeatherType::Sunny | WeatherType::Heatwave => 0.5,
            },
            GridEventType::GeneratorTrip
            | GridEventType::TransmissionConstraint
            | GridEventType::UnexpectedPlantOutage
            | GridEventType::GridCongestion => 1.0,
        }
    }

    pub fn import_multiplier(&self) -> f32 {
        match self {
            GridEventType::RecordDemand => 1.5,
            GridEventType::GeneratorTrip => 1.3,
            GridEventType::TransmissionConstraint => 1.4,
            GridEventType::HeatEmergency => 2.0,
            GridEventType::UnexpectedPlantOutage => 1.5,
            GridEventType::RenewableShortfall => 1.2,
            GridEventType::GridCongestion => 1.3,
        }
    }

    pub fn export_multiplier(&self) -> f32 {
        match self {
            GridEventType::RecordDemand => 3.0,
            GridEventType::GeneratorTrip => 4.0,
            GridEventType::TransmissionConstraint => 3.5,
            GridEventType::HeatEmergency => 5.0,
            GridEventType::UnexpectedPlantOutage => 6.0,
            GridEventType::RenewableShortfall => 2.5,
            GridEventType::GridCongestion => 2.0,
        }
    }
}

/// Per-site grid event state.
///
/// Tracks active grid events that apply temporary multipliers to both
/// import and export TOU rates for sites with `challenge_level >= 2`.
/// Level 1 sites never tick this and use plain TOU rates.
#[derive(Debug, Clone)]
pub struct GridEventState {
    /// Active grid event (if any)
    pub active_event: Option<GridEventType>,
    /// Game time at which the active grid event expires
    pub event_end_time: f32,
    /// Game time of the last grid-event roll (to throttle rolls to ~1/hr)
    pub last_event_roll_time: f32,
    /// Export revenue accumulated during grid events today (reset at day boundary)
    pub event_revenue_today: f32,
    /// Revenue accumulated during the current grid event (reset when event ends)
    pub current_event_revenue: f32,
    /// Import surcharge cost accumulated during grid events today (reset at day boundary)
    pub event_import_surcharge_today: f32,
    /// Best (highest export multiplier) grid event seen today
    pub best_event_type: Option<GridEventType>,
    /// Export multiplier of the best grid event seen today
    pub best_event_export_multiplier: f32,
}

impl Default for GridEventState {
    fn default() -> Self {
        Self {
            active_event: None,
            event_end_time: 0.0,
            last_event_roll_time: 0.0,
            event_revenue_today: 0.0,
            current_event_revenue: 0.0,
            event_import_surcharge_today: 0.0,
            best_event_type: None,
            best_event_export_multiplier: 0.0,
        }
    }
}

impl GridEventState {
    /// Current import multiplier (1.0 when no event is active).
    pub fn current_import_multiplier(&self) -> f32 {
        self.active_event.map_or(1.0, |e| e.import_multiplier())
    }

    /// Current export multiplier (1.0 when no event is active).
    pub fn current_export_multiplier(&self) -> f32 {
        self.active_event.map_or(1.0, |e| e.export_multiplier())
    }

    /// Advance grid event state for this tick.
    ///
    /// `challenge_level`: site difficulty -- controls event probability
    /// `game_time`: current absolute game time (for event timing)
    /// `weather`: current weather, used to weight which events can occur
    /// `rng`: random number generator
    pub fn tick(
        &mut self,
        challenge_level: u8,
        game_time: f32,
        weather: crate::resources::WeatherType,
        rng: &mut impl rand::Rng,
    ) {
        if self.active_event.is_some() && game_time >= self.event_end_time {
            self.active_event = None;
        }

        let event_chance = match challenge_level {
            0..=1 => 0.0,
            2 => 0.05,
            3 => 0.12,
            4 => 0.18,
            _ => 0.25,
        };

        let hours_since_last_roll = (game_time - self.last_event_roll_time) / 3600.0;
        if self.active_event.is_none() && hours_since_last_roll >= 1.0 && event_chance > 0.0 {
            self.last_event_roll_time = game_time;

            let roll: f32 = rng.random();
            if roll < event_chance {
                let total_weight: f32 = GridEventType::ALL
                    .iter()
                    .map(|e| e.weather_weight(weather))
                    .sum();

                if total_weight > 0.0 {
                    let mut pick = rng.random::<f32>() * total_weight;
                    let mut chosen = GridEventType::ALL[0];
                    for &candidate in GridEventType::ALL {
                        let w = candidate.weather_weight(weather);
                        pick -= w;
                        if pick <= 0.0 {
                            chosen = candidate;
                            break;
                        }
                    }

                    let duration_hours: f32 = 2.0 + rng.random::<f32>() * 4.0;
                    let duration_seconds = duration_hours * 3600.0;

                    self.active_event = Some(chosen);
                    self.event_end_time = game_time + duration_seconds;
                    self.current_event_revenue = 0.0;
                }
            }
        }

        if let Some(event) = self.active_event
            && event.export_multiplier() > self.best_event_export_multiplier
        {
            self.best_event_export_multiplier = event.export_multiplier();
            self.best_event_type = Some(event);
        }
    }

    /// Reset daily tracking (call at day boundary)
    pub fn reset_daily(&mut self) {
        self.active_event = None;
        self.event_end_time = 0.0;
        self.last_event_roll_time = 0.0;
        self.event_revenue_today = 0.0;
        self.current_event_revenue = 0.0;
        self.event_import_surcharge_today = 0.0;
        self.best_event_type = None;
        self.best_event_export_multiplier = 0.0;
    }
}

/// Carbon credit market rates for renewable energy incentives
#[derive(Resource, Debug, Clone)]
pub struct CarbonCreditMarket {
    /// Current rate in dollars per 500 kWh (fluctuates 50-100)
    pub rate_per_500kwh: f32,
}

impl Default for CarbonCreditMarket {
    fn default() -> Self {
        Self {
            rate_per_500kwh: 75.0, // Start at $0.15/kWh
        }
    }
}

impl CarbonCreditMarket {
    /// Rate per kWh ($0.10-$0.20, centered at $0.15)
    pub fn rate_per_kwh(&self) -> f32 {
        self.rate_per_500kwh / 500.0
    }

    /// Randomize rate within bounds (call at start of each day)
    pub fn fluctuate(&mut self, rng: &mut impl rand::Rng) {
        // Random value between 50.0 and 100.0 ($0.10-$0.20 per kWh)
        self.rate_per_500kwh = rng.random_range(50.0..=100.0);
    }
}
