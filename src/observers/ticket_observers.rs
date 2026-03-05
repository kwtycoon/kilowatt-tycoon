//! Ticket-related observer handlers.
//!
//! These observers react to ticket lifecycle events and update
//! game state, handle penalties, and manage UI feedback.

use bevy::prelude::*;

use crate::components::ticket::Ticket;
use crate::resources::GameState;

use super::{TicketClosed, TicketEscalated, TicketOpened};

/// Global observer that handles ticket creation events.
///
/// This observer:
/// - Updates first ticket tutorial state
/// - Logs ticket creation
pub fn on_ticket_created_global(trigger: On<TicketOpened>, mut game_state: ResMut<GameState>) {
    let event = &*trigger;

    info!(
        "Ticket {} created for charger {} - type: {:?}",
        event.ticket_id, event.charger_id, event.ticket_type
    );

    // Update first ticket tutorial state
    if !game_state.first_ticket_seen {
        game_state.first_ticket_seen = true;
    }
}

/// Global observer that handles ticket resolution events.
///
/// This observer:
/// - Updates game state statistics
/// - Logs resolution
pub fn on_ticket_resolved_global(trigger: On<TicketClosed>, mut game_state: ResMut<GameState>) {
    let event = &*trigger;

    info!(
        "Ticket {} resolved with {:?}",
        event.ticket_id, event.resolution
    );

    // Update resolved count
    game_state.tickets_resolved += 1;
}

/// Global observer that handles ticket escalation events.
///
/// This observer:
/// - Applies the penalty to game state
/// - Updates escalation statistics
/// - May affect reputation
pub fn on_ticket_escalated_global(trigger: On<TicketEscalated>, mut game_state: ResMut<GameState>) {
    let event = &*trigger;

    warn!(
        "Ticket {} escalated! Penalty: ${:.2}",
        event.ticket_id, event.penalty
    );

    // Apply penalty
    game_state.add_penalty(event.penalty);

    // Update escalation count
    game_state.tickets_escalated += 1;

    game_state.record_reputation(crate::resources::ReputationSource::TicketEscalation);
}

/// Entity-specific observer for ticket creation.
pub fn on_entity_ticket_opened(trigger: On<TicketOpened>, ticket: &Ticket) {
    let event = &*trigger;
    debug!(
        "Entity-specific handler for ticket {} on charger {}",
        ticket.id, event.charger_id
    );
}

/// Entity-specific observer for ticket resolution.
pub fn on_entity_ticket_closed(trigger: On<TicketClosed>, ticket: &Ticket) {
    let event = &*trigger;
    debug!(
        "Entity-specific handler for ticket {} closure: {:?}",
        ticket.id, event.resolution
    );
}
