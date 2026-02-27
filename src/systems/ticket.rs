//! Ticket system

use bevy::prelude::*;

use crate::components::ticket::{Ticket, TicketStatus};
use crate::events::TicketEscalatedEvent;
use crate::resources::{GameClock, GameState};

/// Update ticket SLA timers and handle escalations
pub fn ticket_sla_system(
    mut tickets: Query<(Entity, &mut Ticket)>,
    game_clock: Res<GameClock>,
    mut game_state: ResMut<GameState>,
    mut escalated_events: MessageWriter<TicketEscalatedEvent>,
) {
    if game_clock.is_paused() {
        return;
    }

    for (entity, mut ticket) in &mut tickets {
        // Skip already resolved/escalated tickets
        if matches!(
            ticket.status,
            TicketStatus::Resolved | TicketStatus::Escalated | TicketStatus::Closed
        ) {
            continue;
        }

        // Update priority
        ticket.update_priority(game_clock.game_time, false);

        // Check for SLA breach
        let is_breached = game_clock.game_time >= ticket.sla_deadline
            && ticket
                .sla_paused_until
                .is_none_or(|paused| game_clock.game_time >= paused);

        if is_breached {
            ticket.status = TicketStatus::Escalated;

            // Calculate penalty
            let penalty = match ticket.ticket_type {
                crate::components::ticket::TicketType::BillingDispute => {
                    // Chargeback: 2x session value
                    ticket.session_value * 2.0
                }
                _ => 25.0, // Standard SLA breach penalty
            };

            game_state.add_penalty(penalty);
            game_state.change_reputation(-5);
            game_state.tickets_escalated += 1;

            info!("Ticket {} escalated! Penalty: ${:.2}", ticket.id, penalty);

            escalated_events.write(TicketEscalatedEvent {
                ticket_entity: entity,
                ticket_id: ticket.id.clone(),
                penalty,
            });
        }
    }
}

/// Create a new ticket (helper function for other systems)
pub fn create_ticket(
    commands: &mut Commands,
    ticket_type: crate::components::ticket::TicketType,
    charger_id: String,
    game_time: f32,
    session_value: f32,
    ticket_counter: &mut crate::resources::TicketCounter,
) -> Entity {
    let ticket_id = ticket_counter.generate_next();

    let ticket = Ticket::new(
        ticket_id.clone(),
        ticket_type,
        charger_id,
        game_time,
        session_value,
    );

    let entity = commands.spawn(ticket).id();

    info!("Created ticket {}: {:?}", ticket_id, ticket_type);

    entity
}
