//! Charger-related observer handlers.
//!
//! These observers react to charger lifecycle events and update
//! game state, create tickets, and handle side effects.

use bevy::prelude::*;

use crate::components::charger::Charger;
use crate::components::ticket::{Ticket, TicketType};
use crate::resources::{GameClock, GameState, TicketCounter};

use super::{ChargerFaulted, ChargerRepaired, RemoteActionPerformed, TicketOpened};

/// Global observer that handles charger fault events.
///
/// This observer:
/// - Creates a support ticket for the fault
/// - Updates game state (first fault tutorial)
/// - Logs the fault for debugging
pub fn on_charger_fault_global(
    trigger: On<ChargerFaulted>,
    mut commands: Commands,
    mut game_state: ResMut<GameState>,
    mut ticket_counter: ResMut<TicketCounter>,
    game_clock: Res<GameClock>,
) {
    let event = &*trigger;

    info!(
        "Charger {} faulted with {:?}",
        event.charger_id, event.fault_type
    );

    // Update first fault tutorial state
    if !game_state.first_fault_seen {
        game_state.first_fault_seen = true;
    }

    // Create a ticket for this fault
    let ticket_id = ticket_counter.generate_next();
    let ticket_type = TicketType::from_fault(&event.fault_type);

    let ticket = Ticket::new(
        ticket_id.clone(),
        ticket_type,
        event.charger_id.clone(),
        game_clock.game_time,
        0.0, // No session value for fault tickets
    );

    // Spawn ticket entity and trigger TicketOpened
    let ticket_entity = commands.spawn(ticket).id();
    commands.trigger(TicketOpened {
        entity: ticket_entity,
        ticket_id,
        ticket_type,
        charger_id: event.charger_id.clone(),
    });
}

/// Global observer that handles charger repair events.
///
/// This observer:
/// - Updates game state statistics
/// - Logs the repair
pub fn on_charger_repair_global(trigger: On<ChargerRepaired>, chargers: Query<&Charger>) {
    let event = &*trigger;

    info!("Charger {} repaired", event.charger_id);

    // State is computed from current_fault and is_disabled, no need to set explicitly
    if let Ok(charger) = chargers.get(event.entity) {
        debug!(
            "Charger {} state after repair: {:?}",
            charger.id,
            charger.state()
        );
    }
}

/// Global observer that handles remote action events.
///
/// This observer:
/// - Logs action results
/// - Updates reboot counters
pub fn on_remote_action_performed(
    trigger: On<RemoteActionPerformed>,
    mut chargers: Query<&mut Charger>,
) {
    let event = &*trigger;

    info!(
        "Remote action {:?} on {} - success: {}",
        event.action, event.charger_id, event.success
    );

    // Update charger state if action succeeded
    if event.success
        && let Ok(mut charger) = chargers.get_mut(event.entity)
    {
        // Clear fault on successful repair actions (state computed from current_fault)
        if charger.current_fault.is_some() {
            match event.action {
                crate::components::charger::RemoteAction::SoftReboot
                | crate::components::charger::RemoteAction::HardReboot
                | crate::components::charger::RemoteAction::ReleaseConnector => {
                    charger.current_fault = None;
                    charger.fault_discovered = false;
                }
                _ => {}
            }
        }
    }
}

/// Entity-specific observer for charger faults.
/// Attach this to individual chargers for per-entity handling.
pub fn on_entity_charger_fault(trigger: On<ChargerFaulted>, charger: &Charger) {
    let event = &*trigger;
    debug!(
        "Entity-specific fault handler for charger {}: {:?}",
        charger.id, event.fault_type
    );
}

/// Entity-specific observer for charger repairs.
pub fn on_entity_charger_repair(trigger: On<ChargerRepaired>, charger: &Charger) {
    let event = &*trigger;
    debug!(
        "Entity-specific repair handler for charger {}: action={:?}",
        charger.id, event.repair_action
    );
}
