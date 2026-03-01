//! Support ticket component and related types

use bevy::prelude::*;
use serde::{Deserialize, Serialize};

/// Types of support tickets
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TicketType {
    BillingDispute,
    SessionDidntStart,
    ConnectorStuck,
    SlowCharging,
    AppError,
}

impl TicketType {
    pub fn display_name(&self) -> &'static str {
        match self {
            TicketType::BillingDispute => "Billing Dispute",
            TicketType::SessionDidntStart => "Session Didn't Start",
            TicketType::ConnectorStuck => "Connector Stuck",
            TicketType::SlowCharging => "Slow Charging",
            TicketType::AppError => "App Error",
        }
    }

    /// Create a ticket type from a charger fault
    pub fn from_fault(fault: &crate::components::charger::FaultType) -> Self {
        use crate::components::charger::FaultType;
        match fault {
            FaultType::CommunicationError => TicketType::SessionDidntStart,
            FaultType::CableDamage => TicketType::ConnectorStuck,
            FaultType::PaymentError => TicketType::BillingDispute,
            FaultType::GroundFault => TicketType::SessionDidntStart,
            FaultType::FirmwareFault => TicketType::AppError,
            FaultType::CableTheft => TicketType::ConnectorStuck,
        }
    }

    /// SLA timer in game seconds
    pub fn sla_seconds(&self) -> f32 {
        match self {
            TicketType::BillingDispute => 300.0,    // 5 minutes
            TicketType::SessionDidntStart => 180.0, // 3 minutes
            TicketType::ConnectorStuck => 120.0,    // 2 minutes
            TicketType::SlowCharging => 600.0,      // 10 minutes
            TicketType::AppError => 300.0,          // 5 minutes
        }
    }

    /// Base priority (0-100)
    pub fn base_priority(&self) -> i32 {
        match self {
            TicketType::BillingDispute => 60,
            TicketType::SessionDidntStart => 70,
            TicketType::ConnectorStuck => 80,
            TicketType::SlowCharging => 30,
            TicketType::AppError => 40,
        }
    }
}

/// Ticket status
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum TicketStatus {
    #[default]
    Created,
    Acknowledged,
    InProgress,
    Resolved,
    Escalated,
    Closed,
}

impl TicketStatus {
    pub fn display_name(&self) -> &'static str {
        match self {
            TicketStatus::Created => "New",
            TicketStatus::Acknowledged => "Acknowledged",
            TicketStatus::InProgress => "In Progress",
            TicketStatus::Resolved => "Resolved",
            TicketStatus::Escalated => "Escalated",
            TicketStatus::Closed => "Closed",
        }
    }
}

/// Resolution action for tickets
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum TicketResolution {
    Acknowledge,
    Apologize,
    PartialRefund,
    FullRefund,
    DispatchTechnician,
    Ignore,
}

impl TicketResolution {
    /// Chance to close the ticket (0.0 - 1.0)
    pub fn close_chance(&self) -> f32 {
        match self {
            TicketResolution::Acknowledge => 0.0, // Just pauses SLA
            TicketResolution::Apologize => 0.50,
            TicketResolution::PartialRefund => 0.80,
            TicketResolution::FullRefund => 1.0,
            TicketResolution::DispatchTechnician => 1.0,
            TicketResolution::Ignore => 0.0,
        }
    }
}

/// Main ticket component
#[derive(Component, Debug, Clone)]
pub struct Ticket {
    pub id: String,
    pub ticket_type: TicketType,
    pub charger_id: String,
    pub driver_id: Option<String>,
    pub session_value: f32,
    pub created_at: f32,
    pub sla_deadline: f32,
    pub status: TicketStatus,
    pub priority: i32,
    pub driver_message: String,
    /// Time SLA was paused (for acknowledge)
    pub sla_paused_until: Option<f32>,
}

impl Default for Ticket {
    fn default() -> Self {
        Self {
            id: String::new(),
            ticket_type: TicketType::BillingDispute,
            charger_id: String::new(),
            driver_id: None,
            session_value: 0.0,
            created_at: 0.0,
            sla_deadline: 0.0,
            status: TicketStatus::Created,
            priority: 50,
            driver_message: String::new(),
            sla_paused_until: None,
        }
    }
}

impl Ticket {
    pub fn new(
        id: String,
        ticket_type: TicketType,
        charger_id: String,
        game_time: f32,
        session_value: f32,
    ) -> Self {
        Self {
            id,
            ticket_type,
            charger_id,
            session_value,
            created_at: game_time,
            sla_deadline: game_time + ticket_type.sla_seconds(),
            priority: ticket_type.base_priority(),
            driver_message: generate_driver_message(ticket_type),
            ..default()
        }
    }

    pub fn time_until_breach(&self, game_time: f32) -> f32 {
        if let Some(paused_until) = self.sla_paused_until
            && game_time < paused_until
        {
            return self.sla_deadline - self.created_at; // Full time remaining
        }
        (self.sla_deadline - game_time).max(0.0)
    }

    pub fn is_breached(&self, game_time: f32) -> bool {
        if let Some(paused_until) = self.sla_paused_until
            && game_time < paused_until
        {
            return false;
        }
        game_time >= self.sla_deadline
    }

    pub fn update_priority(&mut self, game_time: f32, driver_waiting: bool) {
        let elapsed_minutes = (game_time - self.created_at) / 60.0;
        let time_bonus = (elapsed_minutes * 10.0) as i32;
        let waiting_bonus = if driver_waiting { 20 } else { 0 };
        let sla_urgency = if self.time_until_breach(game_time) < 60.0 {
            30
        } else {
            0
        };

        self.priority = self.ticket_type.base_priority() + time_bonus + waiting_bonus + sla_urgency;
    }
}

fn generate_driver_message(ticket_type: TicketType) -> String {
    let variations: &[&str] = match ticket_type {
        TicketType::BillingDispute => &[
            "I was charged more than expected. Can you check my session?",
            "The price on my receipt doesn't match the charger. Why?",
            "I think I was double-charged for my last session.",
            "Explain this charge or I'm disputing it with my bank.",
            "Did you just make up a number? This isn't what I agreed to.",
            "I've got the screenshot. This math doesn't math.",
            "Charging me for electricity I never received. Cool cool cool.",
        ],
        TicketType::SessionDidntStart => &[
            "I've been waiting and the charger won't start. Help!",
            "I tapped my card but nothing is happening. Is it on?",
            "The app says it's charging but the car says otherwise.",
            "I'm standing here like an idiot staring at a blank screen.",
            "Is this charger decorative? Because it's not charging.",
            "Tapped, swiped, prayed. Nothing works.",
            "I drove 20 minutes for a charger that won't even turn on.",
        ],
        TicketType::ConnectorStuck => &[
            "The cable is stuck and I can't unplug my car!",
            "I finished charging but the lock won't release. I'm trapped!",
            "The connector is jammed in the port. Please help!",
            "Great, my car is now a permanent fixture at your station.",
            "I have places to be and your charger has taken my car hostage.",
            "So I just live here now? The cable won't let go.",
            "My car and your charger are in a relationship I didn't consent to.",
        ],
        TicketType::SlowCharging => &[
            "Charging is way slower than advertised. What's going on?",
            "I'm only getting 30kW on a 350kW charger. This is slow!",
            "It says 'Ultra-fast' but it's charging like a wall outlet.",
            "At this rate I'll finish charging next Tuesday.",
            "My phone charges faster than this 'ultra-fast' charger.",
            "I could run on a treadmill and generate power faster.",
            "Ultra-fast? More like ultra-nap. I fell asleep waiting.",
        ],
        TicketType::AppError => &[
            "The app keeps giving me errors. Can't authenticate.",
            "Your app is crashing every time I try to start a session.",
            "Login failed. I can't even get past the splash screen.",
            "Your app has more bugs than a picnic.",
            "I've restarted this app 6 times. SIX.",
            "Error 500, error 403, error everything. Pick a number.",
            "Did anyone actually test this app before shipping it?",
        ],
    };

    // Pick a random variation
    let idx = (rand::random::<f32>() * variations.len() as f32) as usize % variations.len();
    variations[idx].to_string()
}
