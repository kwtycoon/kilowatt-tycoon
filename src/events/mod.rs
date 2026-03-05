//! Custom game events (Messages in Bevy 0.17)

pub mod demand;
pub mod site;

use bevy::prelude::*;

use crate::components::charger::{FaultType, RemoteAction};
use crate::components::hacker::HackerAttackType;
use crate::components::ticket::{TicketResolution, TicketType};
use crate::resources::achievements::AchievementKind;
use crate::resources::multi_site::SiteId;
use crate::resources::site_upgrades::OemTier;

pub use demand::*;
pub use site::*;

/// Plugin that registers all events
pub struct EventsPlugin;

impl Plugin for EventsPlugin {
    fn build(&self, app: &mut App) {
        app.add_message::<ChargerFaultEvent>()
            .add_message::<ChargerFaultResolvedEvent>()
            .add_message::<DriverArrivedEvent>()
            .add_message::<DriverLeftEvent>()
            .add_message::<ChargingCompleteEvent>()
            .add_message::<TicketCreatedEvent>()
            .add_message::<TicketResolvedEvent>()
            .add_message::<TicketEscalatedEvent>()
            .add_message::<RemoteActionRequestEvent>()
            .add_message::<RemoteActionResultEvent>()
            .add_message::<CashChangedEvent>()
            .add_message::<ReputationChangedEvent>()
            .add_message::<GameEndedEvent>()
            .add_message::<ShowTooltipEvent>()
            .add_message::<HideTooltipEvent>()
            .add_message::<TransformerWarningEvent>()
            .add_message::<SpeedChangedEvent>()
            .add_message::<TechnicianDispatchEvent>()
            .add_message::<RepairCompleteEvent>()
            .add_message::<RepairFailedEvent>()
            // Site events
            .add_message::<SiteSwitchEvent>()
            .add_message::<SiteSoldEvent>()
            // Demand charge events
            .add_message::<PeakIncreasedEvent>()
            .add_message::<PeakRiskEvent>()
            .add_message::<BessSavedPeakEvent>()
            .add_message::<BessLowSocEvent>()
            .add_message::<DemandBurdenEvent>()
            // O&M events
            .add_message::<OemUpgradeEvent>()
            // Transformer fire events
            .add_message::<TransformerOverloadWarningEvent>()
            .add_message::<TransformerFireEvent>()
            // Achievement events
            .add_message::<AchievementUnlockedEvent>()
            // Hacker events
            .add_message::<HackerAttackEvent>()
            .add_message::<HackerDetectedEvent>()
            // Fleet events
            .add_message::<crate::resources::fleet::FleetContractTerminatedEvent>();
    }
}

// ============ Charger Events ============

#[derive(Event, Message, Debug, Clone)]
pub struct ChargerFaultEvent {
    pub charger_entity: Entity,
    pub charger_id: String,
    pub fault_type: FaultType,
    /// When true the fault was instantly cleared by O&M auto-remediation;
    /// UI systems should suppress the notification.
    pub auto_remediated: bool,
}

#[derive(Event, Message, Debug, Clone)]
pub struct ChargerFaultResolvedEvent {
    pub charger_entity: Entity,
    pub charger_id: String,
}

// ============ Driver Events ============

#[derive(Event, Message, Debug, Clone)]
pub struct DriverArrivedEvent {
    pub driver_entity: Entity,
    pub driver_id: String,
    pub target_charger_id: Option<String>,
}

#[derive(Event, Message, Debug, Clone)]
pub struct DriverLeftEvent {
    pub driver_entity: Entity,
    pub driver_id: String,
    pub angry: bool,
    pub revenue: f32,
}

#[derive(Event, Message, Debug, Clone)]
pub struct ChargingCompleteEvent {
    pub driver_entity: Entity,
    pub charger_entity: Entity,
    pub energy_delivered: f32,
    pub revenue: f32,
}

// ============ Ticket Events ============

#[derive(Event, Message, Debug, Clone)]
pub struct TicketCreatedEvent {
    pub ticket_entity: Entity,
    pub ticket_id: String,
    pub ticket_type: TicketType,
    pub charger_id: String,
}

#[derive(Event, Message, Debug, Clone)]
pub struct TicketResolvedEvent {
    pub ticket_entity: Entity,
    pub ticket_id: String,
    pub resolution: TicketResolution,
}

#[derive(Event, Message, Debug, Clone)]
pub struct TicketEscalatedEvent {
    pub ticket_entity: Entity,
    pub ticket_id: String,
    pub penalty: f32,
}

// ============ Action Events ============

#[derive(Event, Message, Debug, Clone)]
pub struct RemoteActionRequestEvent {
    pub charger_entity: Entity,
    pub action: RemoteAction,
}

#[derive(Event, Message, Debug, Clone)]
pub struct RemoteActionResultEvent {
    pub charger_entity: Entity,
    pub charger_id: String,
    pub action: RemoteAction,
    pub success: bool,
}

// ============ Economy Events ============

#[derive(Event, Message, Debug, Clone)]
pub struct CashChangedEvent {
    pub new_amount: f32,
    pub delta: f32,
    pub reason: String,
}

#[derive(Event, Message, Debug, Clone)]
pub struct ReputationChangedEvent {
    pub new_value: i32,
    pub delta: i32,
}

// ============ Game State Events ============

#[derive(Event, Message, Debug, Clone)]
pub struct GameEndedEvent {
    pub won: bool,
    pub reason: String,
}

#[derive(Event, Message, Debug, Clone)]
pub struct ShowTooltipEvent {
    pub message: String,
    pub auto_pause: bool,
}

#[derive(Event, Message, Debug, Clone)]
pub struct HideTooltipEvent;

#[derive(Event, Message, Debug, Clone)]
pub struct TransformerWarningEvent {
    pub temperature: f32,
    pub is_critical: bool,
}

#[derive(Event, Message, Debug, Clone)]
pub struct SpeedChangedEvent {
    pub new_speed: f32,
}

// ============ Technician Events ============

#[derive(Event, Message, Debug, Clone)]
pub struct TechnicianDispatchEvent {
    pub charger_entity: Entity,
    pub charger_id: String,
}

#[derive(Event, Message, Debug, Clone)]
pub struct RepairCompleteEvent {
    pub charger_entity: Entity,
    pub charger_id: String,
    pub repair_cost: f32,
}

#[derive(Event, Message, Debug, Clone)]
pub struct RepairFailedEvent {
    pub charger_entity: Entity,
    pub charger_id: String,
    pub repair_cost: f32,
    pub failure_reason: String,
}

// ============ Transformer Fire Events ============

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OverloadSeverity {
    Warning,
    Critical,
}

#[derive(Event, Message, Debug, Clone)]
pub struct TransformerOverloadWarningEvent {
    pub severity: OverloadSeverity,
    pub overload_pct: f32,
    pub has_power_management: bool,
}

#[derive(Event, Message, Debug, Clone)]
pub struct TransformerFireEvent {
    pub grid_pos: (i32, i32),
}

// ============ O&M Events ============

/// Emitted when an O&M package is purchased so existing faults can be
/// immediately processed (detected, auto-remediated, or auto-dispatched).
#[derive(Event, Message, Debug, Clone)]
pub struct OemUpgradeEvent {
    pub site_id: SiteId,
    pub new_tier: OemTier,
}

// ============ Achievement Events ============

#[derive(Event, Message, Debug, Clone)]
pub struct AchievementUnlockedEvent {
    pub kind: AchievementKind,
}

// ============ Hacker Events ============

/// Emitted when a hacker successfully executes a cyber-attack.
#[derive(Event, Message, Debug, Clone)]
pub struct HackerAttackEvent {
    pub site_id: SiteId,
    pub attack_type: HackerAttackType,
}

/// Emitted when a hacker attack is detected or auto-blocked by infosec systems.
#[derive(Event, Message, Debug, Clone)]
pub struct HackerDetectedEvent {
    pub site_id: SiteId,
    pub attack_type: HackerAttackType,
    /// True when the Agentic SOC auto-terminated an active attack
    pub auto_blocked: bool,
}
