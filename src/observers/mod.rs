//! Observer-based event handling following Bevy 0.17 patterns.
//!
//! This module implements entity-targeted observers for reactive systems.
//! Observers provide a more powerful alternative to traditional events
//! by allowing per-entity event handling.
//!
//! # Example
//!
//! ```rust,ignore
//! // When spawning a charger, attach entity-specific observers:
//! commands.spawn(Charger::new())
//!     .observe(on_charger_fault)
//!     .observe(on_charger_repair);
//! ```

pub mod charger_observers;
pub mod ticket_observers;

use bevy::prelude::*;

pub use charger_observers::*;
pub use ticket_observers::*;

use crate::components::charger::{FaultType, RemoteAction};
use crate::components::ticket::{TicketResolution, TicketType};

/// Plugin that registers all observers
pub struct ObserversPlugin;

impl Plugin for ObserversPlugin {
    fn build(&self, app: &mut App) {
        // Register global observers for charger events
        app.add_observer(on_charger_fault_global)
            .add_observer(on_charger_repair_global)
            .add_observer(on_remote_action_performed);

        // Register global observers for ticket events
        app.add_observer(on_ticket_created_global)
            .add_observer(on_ticket_resolved_global)
            .add_observer(on_ticket_escalated_global);
    }
}

// ============ Charger Entity Events ============

/// Event triggered when a charger develops a fault.
/// This is an EntityEvent that targets a specific charger.
#[derive(EntityEvent, Debug, Clone)]
pub struct ChargerFaulted {
    /// The charger entity that faulted
    pub entity: Entity,
    /// The type of fault that occurred
    pub fault_type: FaultType,
    /// The charger's ID string
    pub charger_id: String,
}

/// Event triggered when a charger fault is repaired.
#[derive(EntityEvent, Debug, Clone)]
pub struct ChargerRepaired {
    /// The charger entity that was repaired
    pub entity: Entity,
    /// The charger's ID string
    pub charger_id: String,
    /// What action repaired it
    pub repair_action: Option<RemoteAction>,
}

/// Event triggered when a remote action is performed on a charger.
#[derive(EntityEvent, Debug, Clone)]
pub struct RemoteActionPerformed {
    /// The charger entity
    pub entity: Entity,
    /// The action that was performed
    pub action: RemoteAction,
    /// Whether the action succeeded
    pub success: bool,
    /// The charger's ID string
    pub charger_id: String,
}

/// Event triggered when a charger starts a charging session.
#[derive(EntityEvent, Debug, Clone)]
pub struct ChargingStarted {
    /// The charger entity
    pub entity: Entity,
    /// The driver entity being charged
    pub driver_entity: Entity,
}

/// Event triggered when a charger completes a charging session.
#[derive(EntityEvent, Debug, Clone)]
pub struct ChargingCompleted {
    /// The charger entity
    pub entity: Entity,
    /// The driver entity
    pub driver_entity: Entity,
    /// Energy delivered in kWh
    pub energy_kwh: f32,
    /// Revenue generated
    pub revenue: f32,
}

// ============ Ticket Entity Events ============

/// Event triggered when a ticket is created.
#[derive(EntityEvent, Debug, Clone)]
pub struct TicketOpened {
    /// The ticket entity
    pub entity: Entity,
    /// The ticket ID
    pub ticket_id: String,
    /// The type of issue
    pub ticket_type: TicketType,
    /// The affected charger's ID
    pub charger_id: String,
}

/// Event triggered when a ticket is resolved.
#[derive(EntityEvent, Debug, Clone)]
pub struct TicketClosed {
    /// The ticket entity
    pub entity: Entity,
    /// The ticket ID
    pub ticket_id: String,
    /// How the ticket was resolved
    pub resolution: TicketResolution,
}

/// Event triggered when a ticket is escalated (SLA breach).
#[derive(EntityEvent, Debug, Clone)]
pub struct TicketEscalated {
    /// The ticket entity
    pub entity: Entity,
    /// The ticket ID
    pub ticket_id: String,
    /// The penalty incurred
    pub penalty: f32,
}

// ============ Driver Entity Events ============

/// Event triggered when a driver arrives at the charging station.
#[derive(EntityEvent, Debug, Clone)]
pub struct DriverArrived {
    /// The driver entity
    pub entity: Entity,
    /// The driver's ID
    pub driver_id: String,
    /// Their target charger (if any)
    pub target_charger_id: Option<String>,
}

/// Event triggered when a driver leaves the charging station.
#[derive(EntityEvent, Debug, Clone)]
pub struct DriverDeparted {
    /// The driver entity
    pub entity: Entity,
    /// The driver's ID
    pub driver_id: String,
    /// Whether they left angry
    pub left_angry: bool,
    /// Revenue (if any) from their session
    pub revenue: f32,
}
