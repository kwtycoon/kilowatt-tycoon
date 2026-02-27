//! OCPP message queue and per-charger state tracking.
//!
//! `OcppMessageQueue` is a Bevy [`Resource`] that holds:
//! - Configuration (endpoint URL, enabled flag)
//! - Per-charger OCPP state (last sent status, active transaction, etc.)
//! - An outbound message buffer that the connection manager drains

use std::collections::{HashMap, VecDeque};

use bevy::prelude::*;
use chrono::{DateTime, Duration, Utc};

use super::types::ChargePointStatus;

/// Maximum messages buffered before oldest are dropped.
const MAX_QUEUE_SIZE: usize = 2_000;

/// Interval between MeterValues in game-seconds (60 = once per minute of sim-time).
pub const METER_VALUES_INTERVAL_GAME_SECS: f32 = 60.0;

/// Interval between Heartbeat messages in game-seconds (300 = every 5 minutes).
pub const HEARTBEAT_INTERVAL_GAME_SECS: f32 = 300.0;

// ─────────────────────────────────────────────────────
//  OcppMessageQueue (Resource)
// ─────────────────────────────────────────────────────

/// Central OCPP state + outbound message buffer.
#[derive(Resource)]
pub struct OcppMessageQueue {
    /// WebSocket endpoint base URL, e.g. `ws://relion.example.com/ocpp`.
    /// Each charger connects to `{endpoint_url}/{charger_id}`.
    pub endpoint_url: String,

    /// Master enable switch.
    pub enabled: bool,

    /// Simulated start time: game-time 0 maps to this wall-clock instant.
    /// Defaults to today at midnight UTC.
    pub sim_start: DateTime<Utc>,

    /// Outbound message buffer: `(charger_id, serialized_json)`.
    pub messages: VecDeque<(String, String)>,

    /// Per-charger OCPP tracking state, keyed by ECS `Entity`.
    pub charger_state: HashMap<Entity, OcppChargerState>,

    /// Monotonically increasing transaction ID counter.
    next_transaction_id: i32,

    /// Game time of the last heartbeat sent.
    pub last_heartbeat_game_time: f32,

    /// Buffer of messages to write to disk (native only).
    /// Each entry is `(charger_id, serialized_json)`.
    pub disk_buffer: VecDeque<(String, String)>,

    /// Whether disk logging is enabled.
    pub disk_logging_enabled: bool,
}

impl Default for OcppMessageQueue {
    fn default() -> Self {
        // Default sim_start: today at midnight UTC
        let now = Utc::now();
        let midnight = now
            .date_naive()
            .and_hms_opt(0, 0, 0)
            .map(|naive| DateTime::<Utc>::from_naive_utc_and_offset(naive, Utc))
            .unwrap_or(now);

        Self {
            endpoint_url: String::new(),
            enabled: false,
            sim_start: midnight,
            messages: VecDeque::new(),
            charger_state: HashMap::new(),
            next_transaction_id: 1,
            last_heartbeat_game_time: 0.0,
            disk_buffer: VecDeque::new(),
            disk_logging_enabled: false,
        }
    }
}

impl OcppMessageQueue {
    /// Returns `true` if any output sink (WebSocket streaming or disk logging) is active.
    /// Message generation systems should skip work when this returns `false`.
    pub fn is_active(&self) -> bool {
        self.enabled || self.disk_logging_enabled
    }

    /// Convert a `total_game_time` value to a `DateTime<Utc>` timestamp.
    pub fn game_time_to_utc(&self, total_game_time: f32) -> DateTime<Utc> {
        self.sim_start + Duration::seconds(total_game_time as i64)
    }

    /// Allocate the next transaction ID.
    pub fn next_transaction_id(&mut self) -> i32 {
        let id = self.next_transaction_id;
        self.next_transaction_id += 1;
        id
    }

    /// Push a message onto the outbound queue.
    /// Drops oldest messages if the queue is full.
    /// Also copies to the disk buffer when disk logging is enabled.
    pub fn push(&mut self, charger_id: String, json: String) {
        if self.disk_logging_enabled {
            self.disk_buffer
                .push_back((charger_id.clone(), json.clone()));
        }
        if self.messages.len() >= MAX_QUEUE_SIZE {
            self.messages.pop_front();
        }
        self.messages.push_back((charger_id, json));
    }

    /// Drain all pending messages (used by the connection manager).
    pub fn drain_all(&mut self) -> Vec<(String, String)> {
        self.messages.drain(..).collect()
    }

    /// Drain all pending disk messages (used by the disk writer system).
    pub fn drain_disk_buffer(&mut self) -> Vec<(String, String)> {
        self.disk_buffer.drain(..).collect()
    }

    /// Get or create the per-charger state for an entity.
    pub fn get_or_create(&mut self, entity: Entity) -> &mut OcppChargerState {
        self.charger_state.entry(entity).or_default()
    }
}

// ─────────────────────────────────────────────────────
//  Per-charger OCPP state
// ─────────────────────────────────────────────────────

/// Tracks the OCPP-relevant state for a single charger entity.
#[derive(Debug, Clone)]
pub struct OcppChargerState {
    /// Last `StatusNotification` status we sent.
    pub last_status: Option<ChargePointStatus>,

    /// Active OCPP transaction ID (set by StartTransaction, cleared by StopTransaction).
    pub transaction_id: Option<i32>,

    /// Meter reading (Wh) when the current transaction started.
    pub meter_start_wh: i32,

    /// `total_game_time` of the last `MeterValues` message sent.
    pub last_meter_game_time: f32,

    /// Whether we have sent a `BootNotification` for this charger.
    pub boot_sent: bool,

    /// Driver entity associated with the active transaction (for SoC lookups).
    pub active_driver: Option<Entity>,

    /// The charger's string ID (cached for message generation).
    pub charger_id: String,
}

impl Default for OcppChargerState {
    fn default() -> Self {
        Self {
            last_status: None,
            transaction_id: None,
            meter_start_wh: 0,
            last_meter_game_time: 0.0,
            boot_sent: false,
            active_driver: None,
            charger_id: String::new(),
        }
    }
}
