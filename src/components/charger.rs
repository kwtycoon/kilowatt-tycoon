//! Charger component and related types

use bevy::prelude::*;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Charger type enum
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum ChargerType {
    #[default]
    DcFast,
    AcLevel2,
}

impl ChargerType {
    pub fn display_name(&self) -> &'static str {
        match self {
            ChargerType::DcFast => "DCFC",
            ChargerType::AcLevel2 => "L2",
        }
    }

    /// Power factor for this charger type.
    /// DC Fast Chargers typically have active power factor correction (PFC).
    /// L2 chargers are simpler and have lower power factor.
    pub fn power_factor(&self) -> f32 {
        match self {
            ChargerType::DcFast => 0.95,   // Active PFC per spec
            ChargerType::AcLevel2 => 0.90, // Simpler AC chargers
        }
    }
}

/// Charger operational state
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default, Reflect)]
pub enum ChargerState {
    #[default]
    Available,
    Charging,
    Warning,
    Offline,
    Disabled,
}

impl ChargerState {
    pub fn display_name(&self) -> &'static str {
        match self {
            ChargerState::Available => "Available",
            ChargerState::Charging => "Charging",
            ChargerState::Warning => "Warning",
            ChargerState::Offline => "Offline",
            ChargerState::Disabled => "Disabled",
        }
    }

    pub fn sprite_suffix(&self) -> &'static str {
        match self {
            ChargerState::Available => "available",
            ChargerState::Charging => "charging",
            ChargerState::Warning => "warning",
            ChargerState::Offline => "offline",
            ChargerState::Disabled => "offline",
        }
    }
}

/// Electrical phase assignment
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, Default)]
pub enum Phase {
    #[default]
    A,
    B,
    C,
}

/// Fault types that can occur on chargers
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FaultType {
    CommunicationError,
    CableDamage,
    PaymentError,
    GroundFault,
    FirmwareFault,
    CableTheft,
}

impl FaultType {
    pub fn display_name(&self) -> &'static str {
        match self {
            FaultType::CommunicationError => "Communication Error",
            FaultType::CableDamage => "Cable Damage",
            FaultType::PaymentError => "Payment Error",
            FaultType::GroundFault => "Ground Fault",
            FaultType::FirmwareFault => "Firmware Fault",
            FaultType::CableTheft => "Cable Theft",
        }
    }

    /// Reliability penalty when this fault occurs (minimum 10%, up to 25% for severe faults)
    pub fn reliability_penalty(&self) -> f32 {
        match self {
            FaultType::CommunicationError => 0.10, // Minor - 10%
            FaultType::PaymentError => 0.12,       // Minor/medium - 12%
            FaultType::FirmwareFault => 0.15,      // Medium - 15%
            FaultType::GroundFault => 0.20,        // Serious - 20%
            FaultType::CableDamage => 0.25,        // Severe - 25%
            FaultType::CableTheft => 0.25,         // Severe - 25%
        }
    }

    /// Get the repair cost for this fault type (parts + dispatch)
    pub fn repair_cost(&self) -> f32 {
        match self {
            FaultType::CommunicationError => 0.0,
            FaultType::PaymentError => 0.0,
            FaultType::FirmwareFault => 0.0,
            FaultType::GroundFault => 200.0,
            FaultType::CableDamage => 350.0,
            // Cable theft base cost is 0 here; the actual replacement cost varies
            // by charger power tier and is computed via Charger::cable_replacement_cost().
            FaultType::CableTheft => 0.0,
        }
    }

    /// Get the repair duration in game seconds (0 = instant remote fix)
    pub fn repair_duration_secs(&self) -> f32 {
        match self {
            FaultType::CommunicationError => 0.0, // Instant (remote reboot)
            FaultType::PaymentError => 0.0,       // Instant (remote reboot)
            FaultType::FirmwareFault => 0.0,      // Instant (remote reboot)
            FaultType::GroundFault => 900.0,      // 15 minutes
            FaultType::CableDamage => 1200.0,     // 20 minutes
            FaultType::CableTheft => 1200.0,      // 20 minutes (cable replacement)
        }
    }

    /// Check if this fault requires a technician (vs remote fix)
    pub fn requires_technician(&self) -> bool {
        matches!(
            self,
            FaultType::GroundFault | FaultType::CableDamage | FaultType::CableTheft
        )
    }

    /// Get the charger state corresponding to this fault type
    pub fn to_charger_state(&self) -> ChargerState {
        match self {
            FaultType::CommunicationError | FaultType::FirmwareFault | FaultType::PaymentError => {
                ChargerState::Warning
            }
            FaultType::GroundFault | FaultType::CableDamage | FaultType::CableTheft => {
                ChargerState::Offline
            }
        }
    }
}

/// Charger reliability tier - affects failure rates and efficiency
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum ChargerTier {
    /// Budget tier: higher failure rate, lower efficiency
    Value,
    /// Standard tier: average reliability
    #[default]
    Standard,
    /// Premium tier: low failure rate, high efficiency
    Premium,
}

impl ChargerTier {
    pub fn display_name(&self) -> &'static str {
        match self {
            ChargerTier::Value => "Value",
            ChargerTier::Standard => "Standard",
            ChargerTier::Premium => "Premium",
        }
    }

    /// Mean time between failures in game hours (higher = more reliable)
    pub fn mtbf_hours(&self) -> f32 {
        match self {
            ChargerTier::Value => 18.0,    // Fails multiple times per day
            ChargerTier::Standard => 50.0, // Fails roughly every 2 game days
            ChargerTier::Premium => 150.0, // Fails roughly every 6 game days
        }
    }

    /// Efficiency multiplier (affects power conversion losses)
    pub fn efficiency(&self) -> f32 {
        match self {
            ChargerTier::Value => 0.88,
            ChargerTier::Standard => 0.92,
            ChargerTier::Premium => 0.96,
        }
    }

    /// Connector jam chance multiplier
    pub fn jam_multiplier(&self) -> f32 {
        match self {
            ChargerTier::Value => 1.5,
            ChargerTier::Standard => 1.0,
            ChargerTier::Premium => 0.3,
        }
    }

    /// Remote action success rate modifier (added to base rate)
    pub fn action_success_bonus(&self) -> f32 {
        match self {
            ChargerTier::Value => -0.10, // 10% less likely to succeed
            ChargerTier::Standard => 0.0,
            ChargerTier::Premium => 0.15, // 15% more likely to succeed
        }
    }
}

/// Remote action types
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum RemoteAction {
    SoftReboot,
    HardReboot,
    ReleaseConnector,
    Disable,
    Enable,
}

impl RemoteAction {
    /// Cooldown duration in game seconds
    pub fn cooldown_seconds(&self) -> f32 {
        match self {
            RemoteAction::SoftReboot => 30.0,
            RemoteAction::HardReboot => 120.0,
            RemoteAction::ReleaseConnector => 10.0,
            RemoteAction::Disable => 0.0,
            RemoteAction::Enable => 5.0,
        }
    }

    /// Base success rate (0.0 - 1.0)
    pub fn success_rate(&self) -> f32 {
        match self {
            RemoteAction::SoftReboot => 0.70,
            RemoteAction::HardReboot => 0.90,
            RemoteAction::ReleaseConnector => 0.80,
            RemoteAction::Disable => 1.0,
            RemoteAction::Enable => 0.95,
        }
    }
}

/// Main charger component
#[derive(Component, Debug, Clone)]
pub struct Charger {
    pub id: String,
    pub name: String,
    pub charger_type: ChargerType,
    pub max_power_kw: f32,
    pub rated_power_kw: f32,
    pub phase: Phase,
    pub health: f32,
    /// Whether a charging session is currently active
    pub is_charging: bool,
    pub current_power_kw: f32,
    pub is_disabled: bool,
    pub current_fault: Option<FaultType>,
    /// Whether the current fault has been discovered (by a driver or O&M system)
    pub fault_discovered: bool,
    /// Cooldowns: action -> remaining game seconds
    pub cooldowns: HashMap<RemoteAction, f32>,
    /// Scripted fault trigger time (game seconds), if any
    pub scripted_fault_time: Option<f32>,
    pub scripted_fault_type: Option<FaultType>,
    /// Connector jam chance (0.0 - 1.0)
    pub connector_jam_chance: f32,
    /// Connector type string
    pub connector_type: String,
    /// Grid position if placed from build mode
    pub grid_position: Option<(i32, i32)>,
    /// Power requested by the charging session (kW)
    pub requested_power_kw: f32,
    /// Power allocated by site dispatch after constraints (kW)
    pub allocated_power_kw: f32,
    /// Game time when session started (for FCFS ordering)
    pub session_start_game_time: Option<f32>,
    /// Charger tier (affects reliability and efficiency)
    pub tier: ChargerTier,
    /// Cumulative operating hours (for MTBF calculation)
    pub operating_hours: f32,
    /// Hours since last random fault (reset when fault occurs)
    pub hours_since_last_fault: f32,

    // === Reliability & Fault Tracking ===
    /// Reliability score (0.0 - 1.0). Affects driver preference for this charger.
    /// Degrades on faults/downtime, recovers on successful sessions.
    pub reliability: f32,
    /// Game time when current fault actually occurred (charger broke)
    pub fault_occurred_at: Option<f32>,
    /// Game time when fault was detected (player notified)
    pub fault_detected_at: Option<f32>,
    /// Whether the current fault has been detected/notified to the player
    pub fault_is_detected: bool,

    // === KPI Tracking ===
    /// Total energy delivered by this charger (kWh)
    pub total_energy_delivered_kwh: f32,
    /// Energy delivered by this charger today (kWh), reset each day
    pub energy_delivered_kwh_today: f32,
    /// Number of completed charging sessions
    pub session_count: u32,
    /// Total revenue earned by this charger ($)
    pub total_revenue: f32,
    /// Total repair OpEx costs for this charger ($)
    pub total_repair_opex: f32,

    // === Video Advertisement ===
    /// Whether video advertisements are enabled on this charger
    pub video_ad_enabled: bool,
    /// Total ad revenue earned by this charger ($)
    pub total_ad_revenue: f32,
    /// Ad revenue accumulated during the current session, flushed to ledger at session end
    pub pending_ad_revenue: f32,

    // === Anti-theft ===
    /// Whether this charger has an anti-theft cable upgrade
    pub anti_theft_cable: bool,
}

impl Default for Charger {
    fn default() -> Self {
        Self {
            id: String::new(),
            name: String::new(),
            charger_type: ChargerType::DcFast,
            max_power_kw: 50.0,
            rated_power_kw: 50.0,
            phase: Phase::A,
            health: 1.0,
            is_charging: false,
            current_power_kw: 0.0,
            is_disabled: false,
            current_fault: None,
            fault_discovered: false,
            cooldowns: HashMap::new(),
            scripted_fault_time: None,
            scripted_fault_type: None,
            connector_jam_chance: 0.0,
            connector_type: "CCS".to_string(),
            grid_position: None,
            requested_power_kw: 0.0,
            allocated_power_kw: 0.0,
            session_start_game_time: None,
            tier: ChargerTier::Standard,
            operating_hours: 0.0,
            hours_since_last_fault: 0.0,
            reliability: 1.0,
            fault_occurred_at: None,
            fault_detected_at: None,
            fault_is_detected: true, // No fault = nothing to detect
            total_energy_delivered_kwh: 0.0,
            energy_delivered_kwh_today: 0.0,
            session_count: 0,
            total_revenue: 0.0,
            total_repair_opex: 0.0,
            video_ad_enabled: false,
            total_ad_revenue: 0.0,
            pending_ad_revenue: 0.0,
            anti_theft_cable: false,
        }
    }
}

impl Charger {
    /// Compute current state from underlying fields (single source of truth).
    /// State is derived from is_disabled, current_fault, and is_charging.
    pub fn state(&self) -> ChargerState {
        if self.is_disabled {
            ChargerState::Disabled
        } else if let Some(fault) = self.current_fault {
            fault.to_charger_state()
        } else if self.is_charging {
            ChargerState::Charging
        } else {
            ChargerState::Available
        }
    }

    pub fn is_on_cooldown(&self, action: RemoteAction) -> bool {
        self.cooldowns.get(&action).copied().unwrap_or(0.0) > 0.0
    }

    pub fn start_cooldown(&mut self, action: RemoteAction) {
        self.cooldowns.insert(action, action.cooldown_seconds());
    }

    pub fn update_cooldowns(&mut self, delta: f32) {
        for cooldown in self.cooldowns.values_mut() {
            *cooldown = (*cooldown - delta).max(0.0);
        }
    }

    pub fn can_accept_driver(&self) -> bool {
        self.state() == ChargerState::Available
    }

    /// Check if the charger hardware is capable of delivering power.
    /// This is the single source of truth for operational status.
    /// Returns false if disabled OR has any fault.
    pub fn can_deliver_power(&self) -> bool {
        !self.is_disabled && self.current_fault.is_none()
    }

    /// Check if the charger is currently operational and actively charging.
    /// Combines hardware capability with session state.
    pub fn is_operational(&self) -> bool {
        self.can_deliver_power() && self.is_charging
    }

    pub fn get_derated_power(&self) -> f32 {
        // Apply health-based derating and tier efficiency
        self.rated_power_kw * self.health * self.tier.efficiency()
    }

    /// Get the effective connector jam chance (base * tier multiplier)
    pub fn effective_jam_chance(&self) -> f32 {
        (self.connector_jam_chance * self.tier.jam_multiplier()).clamp(0.0, 1.0)
    }

    /// Get effective action success rate (base + tier bonus)
    pub fn effective_action_success(&self, action: RemoteAction) -> f32 {
        (action.success_rate() + self.tier.action_success_bonus()).clamp(0.0, 1.0)
    }

    /// Calculate probability of a random fault occurring this tick.
    /// Based on exponential distribution with MTBF, scaled by cumulative wear.
    /// Wear uses a saturating exponential curve that caps at ~3x base rate,
    /// preventing runaway fault cascades at high operating hours.
    pub fn fault_probability(&self, delta_hours: f32) -> f32 {
        let mtbf = self.tier.mtbf_hours();
        if mtbf <= 0.0 || delta_hours <= 0.0 {
            return 0.0;
        }
        let wear_ratio = self.operating_hours / mtbf;
        let wear_multiplier = 1.0 + 2.0 * (1.0 - bevy::math::ops::exp(-0.5 * wear_ratio));
        (delta_hours / mtbf * wear_multiplier).min(0.25)
    }

    /// Calculate the real power (kW) drawn from the grid for a given output power.
    /// Formula: P_input_kW = P_output_kW / efficiency
    /// This accounts for conversion losses in the charger.
    pub fn input_kw(&self, output_kw: f32) -> f32 {
        let efficiency = self.tier.efficiency();
        if efficiency > 0.0 {
            output_kw / efficiency
        } else {
            output_kw
        }
    }

    /// Session attraction multiplier based on reliability.
    /// Drivers prefer reliable chargers. A perfect charger (1.0) gets full demand.
    /// Range: 0.5x (very unreliable) to 1.0x (perfect).
    pub fn session_attraction(&self) -> f32 {
        0.5 + (self.reliability * 0.5)
    }

    /// Degrade reliability due to a fault occurring.
    /// More severe faults cause a bigger immediate reliability hit (minimum 10%).
    pub fn degrade_reliability_fault(&mut self, fault_type: &FaultType) {
        let penalty = fault_type.reliability_penalty();
        self.reliability = (self.reliability - penalty).max(0.0);
    }

    /// Degrade reliability due to ongoing downtime (per game-hour while faulted)
    pub fn degrade_reliability_downtime(&mut self, delta_hours: f32) {
        self.reliability = (self.reliability - 0.005 * delta_hours).max(0.0);
    }

    /// Recover reliability from a successful session
    pub fn recover_reliability_session(&mut self, oem_recovery_mult: f32) {
        self.reliability = (self.reliability + 0.01 * oem_recovery_mult).min(1.0);
    }

    /// Recover reliability bonus for fast fault resolution
    pub fn recover_reliability_fast_fix(&mut self, downtime_secs: f32, oem_recovery_mult: f32) {
        // Faster fixes = bigger reliability recovery (up to +0.03)
        let recovery = if downtime_secs < 120.0 {
            0.03 // Very fast fix
        } else if downtime_secs < 600.0 {
            0.02 // Reasonable fix
        } else {
            0.01 // Slow but eventually fixed
        };
        self.reliability = (self.reliability + recovery * oem_recovery_mult).min(1.0);
    }

    /// Passive reliability recovery from maintenance investment.
    /// `maintenance_rate` is 0.0 (no investment) to 1.0 (max $50/hr).
    /// At max, recovers ~0.05 reliability per game-hour (~full recovery in 20 hours).
    pub fn recover_reliability_maintenance(&mut self, maintenance_rate: f32, delta_hours: f32) {
        let recovery = 0.05 * maintenance_rate * delta_hours;
        self.reliability = (self.reliability + recovery).min(1.0);
    }

    /// Calculate the apparent power (kVA) drawn from the grid for a given output power.
    /// Formula: S_kVA = P_input_kW / PF = P_output_kW / (efficiency * PF)
    /// This is what actually loads the transformer and grid infrastructure.
    pub fn input_kva(&self, output_kw: f32) -> f32 {
        let efficiency = self.tier.efficiency();
        let power_factor = self.charger_type.power_factor();
        if efficiency > 0.0 && power_factor > 0.0 {
            output_kw / (efficiency * power_factor)
        } else {
            output_kw
        }
    }

    /// Replacement-part cost when a cable is stolen, tiered by charger power.
    /// L2: $300; DCFC 50 kW: $1,000; 100 kW: $1,500; 150 kW: $1,750; 350 kW: $3,000.
    pub fn cable_replacement_cost(&self) -> f32 {
        match self.charger_type {
            ChargerType::AcLevel2 => 300.0,
            ChargerType::DcFast => {
                if self.rated_power_kw <= 50.0 {
                    1_000.0
                } else if self.rated_power_kw <= 100.0 {
                    1_500.0
                } else if self.rated_power_kw <= 150.0 {
                    1_750.0
                } else {
                    3_000.0
                }
            }
        }
    }

    /// Price to upgrade this charger to an anti-theft cable (by type and power tier).
    /// L2: $800; DCFC 50 kW: $3,200; 150 kW: $6,000; 350 kW: $10,000.
    pub fn anti_theft_cable_price(&self) -> i32 {
        match self.charger_type {
            ChargerType::AcLevel2 => 800,
            ChargerType::DcFast => {
                if self.rated_power_kw <= 75.0 {
                    3_200
                } else if self.rated_power_kw <= 200.0 {
                    6_000
                } else {
                    10_000
                }
            }
        }
    }

    /// Refund amount when selling the anti-theft cable upgrade (50% of purchase price).
    pub fn anti_theft_cable_refund(&self) -> i32 {
        self.anti_theft_cable_price() / 2
    }

    /// Monthly warranty premium for this charger at the given tier.
    pub fn warranty_premium(&self, tier: crate::resources::WarrantyTier) -> f32 {
        tier.charger_monthly_premium(self.charger_type, self.rated_power_kw)
    }
}

/// Marker component for the currently selected charger
#[derive(Component, Debug, Clone, Copy)]
pub struct SelectedCharger;

/// Marker for charger visual sprite.
/// Always spawned as a child of the Charger entity.
#[derive(Component, Debug)]
pub struct ChargerSprite {
    /// Reference to the parent Charger entity
    pub charger_entity: Entity,
}

/// Marker for the small shield sprite shown on a charger when anti-theft cable is installed.
/// Spawned as a child of the Charger entity.
#[derive(Component, Debug, Clone, Copy)]
pub struct AntiTheftShieldIndicator;
