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

/// Maximum log entries retained in memory.
const MAX_EVENT_LOG: usize = 2_000;

// ─────────────────────────────────────────────────────
//  OCPP event log entry (kwwhat-compatible CSV shape)
// ─────────────────────────────────────────────────────

/// A single OCPP log entry in the shape kwwhat expects:
/// `timestamp, charge_point_id, action, msg`.
#[derive(Debug, Clone, serde::Serialize)]
pub struct OcppLogEntry {
    pub timestamp: String,
    #[serde(rename = "id")]
    pub charge_point_id: String,
    /// OCPP action name for Call messages; empty string for CallResult messages.
    pub action: String,
    pub msg: String,
}

/// Interval between MeterValues in game-seconds (60 = once per minute of sim-time).
pub const METER_VALUES_INTERVAL_GAME_SECS: f32 = 60.0;

/// Interval between Heartbeat messages in game-seconds (300 = every 5 minutes).
pub const HEARTBEAT_INTERVAL_GAME_SECS: f32 = 300.0;

/// Game-seconds to wait for a `StartTransaction.conf` from a real CSMS before
/// falling back to a locally allocated transaction id (so a mute CSMS can't
/// wedge a charging session indefinitely).
pub const TX_CONF_TIMEOUT_GAME_SECS: f32 = 30.0;

/// A record of an outbound OCPP Call awaiting its CallResult, so inbound
/// responses can be correlated back to the charger that sent the request.
#[derive(Debug, Clone)]
pub struct PendingCall {
    /// OCPP action name (e.g. "StartTransaction", "BootNotification").
    pub action: String,
    /// The charger entity that originated the Call.
    pub entity: Entity,
    /// `total_game_time` when the Call was sent (used to expire stale entries).
    pub sent_game_time: f32,
}

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

    /// In-memory event log for analytics (kwwhat-compatible).
    /// Accumulates all OCPP messages (Call + CallResult) in CSV-ready format.
    pub event_log: Vec<OcppLogEntry>,

    /// Whether the in-memory event log is enabled.
    /// Defaults to `true` so the log always accumulates when the `ocpp` feature is compiled in.
    pub event_log_enabled: bool,

    /// Total number of entries drained from the front of `event_log` over its
    /// lifetime. Feed systems use this to convert their monotonic
    /// `last_pushed_index` back to a relative Vec index after trimming.
    pub total_drained: usize,

    /// Outbound Calls awaiting a CallResult, keyed by OCPP unique id.
    /// Only populated when connected to a real CSMS.
    pub pending_calls: HashMap<String, PendingCall>,

    /// Heartbeat cadence in game-seconds. Defaults to
    /// [`HEARTBEAT_INTERVAL_GAME_SECS`] but is overridden by the `interval`
    /// returned in a real `BootNotification.conf`.
    pub heartbeat_interval_game_secs: f32,
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
            event_log: Vec::new(),
            event_log_enabled: true,
            total_drained: 0,
            pending_calls: HashMap::new(),
            heartbeat_interval_game_secs: HEARTBEAT_INTERVAL_GAME_SECS,
        }
    }
}

impl OcppMessageQueue {
    /// Returns `true` if any output sink is active (WebSocket, disk, or event log).
    /// Message generation systems should skip work when this returns `false`.
    pub fn is_active(&self) -> bool {
        self.enabled || self.disk_logging_enabled || self.event_log_enabled
    }

    /// Returns `true` when connected to a real CSMS over WebSocket.
    ///
    /// In this mode the game acts as a genuine OCPP client: it does not
    /// fabricate CallResults or CSMS-direction Calls, instead relying on the
    /// real responses and commands received over the wire.
    pub fn real_csms(&self) -> bool {
        self.enabled && !self.endpoint_url.is_empty()
    }

    /// Register an outbound Call as awaiting its CallResult (real-CSMS mode).
    pub fn register_pending(
        &mut self,
        unique_id: String,
        action: &str,
        entity: Entity,
        sent_game_time: f32,
    ) {
        self.pending_calls.insert(
            unique_id,
            PendingCall {
                action: action.to_string(),
                entity,
                sent_game_time,
            },
        );
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

    /// Push a message onto the outbound queue and the in-memory event log.
    ///
    /// This is the preferred entry point for message generation systems.
    /// It records the full CSV-ready metadata (timestamp, action) needed by
    /// analytics consumers while also feeding the WebSocket and disk paths.
    pub fn push_with_log(
        &mut self,
        charger_id: String,
        timestamp_iso: String,
        action: &str,
        json: String,
    ) {
        if self.event_log_enabled {
            self.event_log.push(OcppLogEntry {
                timestamp: timestamp_iso,
                charge_point_id: charger_id.clone(),
                action: action.to_string(),
                msg: json.clone(),
            });
            if self.event_log.len() > MAX_EVENT_LOG {
                let excess = self.event_log.len() - MAX_EVENT_LOG;
                self.event_log.drain(..excess);
                self.total_drained += excess;
            }
        }
        self.push(charger_id, json);
    }

    /// Record a frame in the in-memory event log and disk buffer WITHOUT
    /// enqueuing it on the outbound wire.
    ///
    /// Used for inbound frames received from a real CSMS (CSMS Calls and the
    /// CallResults answering our Calls), so analytics/logs see real traffic
    /// while the wire itself is only fed by genuine outbound messages.
    pub fn log_only(
        &mut self,
        charger_id: String,
        timestamp_iso: String,
        action: &str,
        json: String,
    ) {
        if self.event_log_enabled {
            self.event_log.push(OcppLogEntry {
                timestamp: timestamp_iso,
                charge_point_id: charger_id.clone(),
                action: action.to_string(),
                msg: json.clone(),
            });
            if self.event_log.len() > MAX_EVENT_LOG {
                let excess = self.event_log.len() - MAX_EVENT_LOG;
                self.event_log.drain(..excess);
                self.total_drained += excess;
            }
        }
        if self.disk_logging_enabled {
            self.disk_buffer.push_back((charger_id, json));
        }
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

    /// The `VID:<mac>` idTag for the active transaction (carried to StopTransaction).
    pub active_id_tag: Option<String>,

    /// The charger's string ID (cached for message generation).
    pub charger_id: String,

    /// `total_game_time` at which the current transaction started. Used as the
    /// anchor for `Relative` charging profiles and for profile evaluation.
    pub tx_start_total_game_time: Option<f32>,

    /// In real-CSMS mode, `true` while a `StartTransaction` has been sent but
    /// its `StartTransaction.conf` (carrying the CSMS-assigned transaction id)
    /// has not yet been received.
    pub awaiting_tx_conf: bool,

    /// `total_game_time` when the `StartTransaction` was sent, used to expire
    /// the wait for a `StartTransaction.conf` (see [`TX_CONF_TIMEOUT_GAME_SECS`]).
    pub tx_started_wait_game_time: f32,
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
            active_id_tag: None,
            charger_id: String::new(),
            tx_start_total_game_time: None,
            awaiting_tx_conf: false,
            tx_started_wait_game_time: 0.0,
        }
    }
}
