//! OCPI message queue and per-charger roaming state tracking.
//!
//! `OcpiMessageQueue` is a Bevy [`Resource`] that holds:
//! - An in-memory event log for the JS overlay feed
//! - Per-charger OCPI state (location pushed, active session, etc.)

use std::collections::HashMap;

use bevy::prelude::*;
use chrono::{DateTime, Duration, Utc};

use crate::components::charger::ChargerState;

const MAX_EVENT_LOG: usize = 2_000;

/// Interval between Session PATCH updates in game-seconds.
pub const SESSION_UPDATE_INTERVAL_GAME_SECS: f32 = 60.0;

// ─────────────────────────────────────────────────────
//  OCPI log entry (JSON-serializable for JS overlay)
// ─────────────────────────────────────────────────────

#[derive(Debug, Clone, serde::Serialize)]
pub struct OcpiLogEntry {
    pub timestamp: String,
    pub party_id: String,
    pub object_type: String,
    pub action: String,
    pub msg: String,
}

// ─────────────────────────────────────────────────────
//  Per-charger OCPI state
// ─────────────────────────────────────────────────────

#[derive(Debug, Clone)]
pub struct OcpiChargerState {
    pub charger_id: String,
    pub location_pushed: bool,
    pub last_status: Option<ChargerState>,

    pub session_id: Option<i32>,
    pub session_kwh: f32,
    pub session_start_game_time: f32,
    pub last_update_game_time: f32,
    pub active_driver: Option<Entity>,
    pub active_id_tag: Option<String>,
}

impl Default for OcpiChargerState {
    fn default() -> Self {
        Self {
            charger_id: String::new(),
            location_pushed: false,
            last_status: None,
            session_id: None,
            session_kwh: 0.0,
            session_start_game_time: 0.0,
            last_update_game_time: 0.0,
            active_driver: None,
            active_id_tag: None,
        }
    }
}

// ─────────────────────────────────────────────────────
//  Tariff change-detection fingerprint
// ─────────────────────────────────────────────────────

/// Snapshot of the last emitted OCPI tariff for a site, used to detect when
/// the tariff content has actually changed (mode switch, player config edit,
/// or dynamic price fluctuation).
#[derive(Debug, Clone, PartialEq)]
pub struct LastEmittedTariff {
    pub tariff_id: String,
    /// Price of each tariff element quantized to whole cents to avoid
    /// floating-point jitter from triggering spurious emissions.
    pub price_cents: Vec<i32>,
}

// ─────────────────────────────────────────────────────
//  OcpiMessageQueue (Resource)
// ─────────────────────────────────────────────────────

#[derive(Resource)]
pub struct OcpiMessageQueue {
    pub sim_start: DateTime<Utc>,
    pub event_log: Vec<OcpiLogEntry>,
    pub event_log_enabled: bool,
    pub charger_state: HashMap<Entity, OcpiChargerState>,
    next_session_id: i32,
    /// Last emitted tariff per site (content-aware change detection).
    pub last_emitted_tariff: HashMap<crate::resources::SiteId, LastEmittedTariff>,
}

impl Default for OcpiMessageQueue {
    fn default() -> Self {
        let now = Utc::now();
        let midnight = now
            .date_naive()
            .and_hms_opt(0, 0, 0)
            .map(|naive| DateTime::<Utc>::from_naive_utc_and_offset(naive, Utc))
            .unwrap_or(now);

        Self {
            sim_start: midnight,
            event_log: Vec::new(),
            event_log_enabled: true,
            charger_state: HashMap::new(),
            next_session_id: 1,
            last_emitted_tariff: HashMap::new(),
        }
    }
}

impl OcpiMessageQueue {
    pub fn is_active(&self) -> bool {
        self.event_log_enabled
    }

    pub fn game_time_to_utc(&self, total_game_time: f32) -> DateTime<Utc> {
        self.sim_start + Duration::seconds(total_game_time as i64)
    }

    pub fn get_or_create(&mut self, entity: Entity) -> &mut OcpiChargerState {
        self.charger_state.entry(entity).or_default()
    }

    pub fn next_session_id(&mut self) -> i32 {
        let id = self.next_session_id;
        self.next_session_id += 1;
        id
    }

    pub fn push_log(
        &mut self,
        party_id: String,
        timestamp_iso: String,
        object_type: &str,
        action: &str,
        json: String,
    ) {
        if !self.event_log_enabled {
            return;
        }
        self.event_log.push(OcpiLogEntry {
            timestamp: timestamp_iso,
            party_id,
            object_type: object_type.to_string(),
            action: action.to_string(),
            msg: json,
        });
        if self.event_log.len() > MAX_EVENT_LOG {
            let excess = self.event_log.len() - MAX_EVENT_LOG;
            self.event_log.drain(..excess);
        }
    }
}
