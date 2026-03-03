//! OpenADR message queue and per-site DER state tracking.
//!
//! `OpenAdrMessageQueue` is a Bevy [`Resource`] that holds:
//! - An in-memory event log for the JS overlay feed
//! - Per-site DER state (solar/BESS registration, last telemetry time, etc.)

use std::collections::HashMap;

use bevy::prelude::*;
use chrono::{DateTime, Duration, Utc};

use crate::resources::multi_site::SiteId;
use crate::resources::site_energy::TouPeriod;

/// Maximum log entries retained in memory.
const MAX_EVENT_LOG: usize = 2_000;

/// Interval between telemetry reports in game-seconds.
pub const TELEMETRY_INTERVAL_GAME_SECS: f32 = 60.0;

// ─────────────────────────────────────────────────────
//  OpenADR log entry (JSON-serializable for JS overlay)
// ─────────────────────────────────────────────────────

/// A single OpenADR log entry for the JS overlay feed.
#[derive(Debug, Clone, serde::Serialize)]
pub struct OpenAdrLogEntry {
    pub timestamp: String,
    /// VEN or VTN identifier, e.g. "solar-site-1", "bess-site-1", "grid-site-1"
    pub ven_id: String,
    /// OpenADR 3.0 object type: "Event", "Report", "Ven"
    pub message_type: String,
    /// Human-readable action label for the overlay, e.g. "Solar Telemetry", "Price Signal"
    pub action: String,
    /// Full JSON of the serialized openleadr-wire struct
    pub msg: String,
}

// ─────────────────────────────────────────────────────
//  Per-site DER state
// ─────────────────────────────────────────────────────

/// Tracks OpenADR-relevant state for DER assets at a single site.
#[derive(Debug, Clone)]
pub struct OpenAdrSiteState {
    /// Whether we have sent a VEN registration for the solar array.
    pub solar_registered: bool,
    /// Whether we have sent a VEN registration for the BESS.
    pub bess_registered: bool,
    /// Last known solar installed capacity (to detect new arrays placed).
    pub last_solar_kw_peak: f32,
    /// Last known BESS capacity (to detect new batteries placed).
    pub last_bess_kwh: f32,
    /// Last TOU period we signaled (to detect transitions).
    pub last_tou_period: Option<TouPeriod>,
    /// Game time of the last telemetry report sent.
    pub last_telemetry_game_time: f32,
    /// Game time of the last demand-limit event sent.
    pub last_demand_event_game_time: f32,
    /// Whether a demand-limit event is currently active.
    pub demand_event_active: bool,
    /// Whether a solar export event is currently active.
    pub export_active: bool,
    /// Whether the BESS is currently discharging (for start/stop transition events).
    pub bess_discharging: bool,
    /// Last emitted customer-facing price (to detect changes).
    pub last_customer_price: Option<f32>,
    /// Whether a grid event signal is currently active.
    pub grid_event_active: bool,
    /// Whether a grid alert event has been emitted for the current grid event.
    pub grid_alert_emitted: bool,
    /// Last emitted carbon credit rate (quantised, to detect changes).
    pub last_carbon_rate: Option<f32>,
    /// Whether BESS is actively exporting to the grid via GridExport mode.
    pub bess_grid_export_active: bool,
    /// Whether a transformer fire/overload alert is currently active.
    pub fire_alert_active: bool,
    /// Whether a fire-induced ImportCapacityLimit=0 event has been emitted.
    pub fire_capacity_zeroed: bool,
    /// Last emitted effective capacity during fire tracking (to detect restoration).
    pub last_fire_capacity_kva: Option<f32>,
    /// Monotonically increasing interval counter for reports.
    pub next_interval_id: i32,
}

impl Default for OpenAdrSiteState {
    fn default() -> Self {
        Self {
            solar_registered: false,
            bess_registered: false,
            last_solar_kw_peak: 0.0,
            last_bess_kwh: 0.0,
            last_tou_period: None,
            last_telemetry_game_time: 0.0,
            last_demand_event_game_time: 0.0,
            demand_event_active: false,
            export_active: false,
            bess_discharging: false,
            last_customer_price: None,
            grid_event_active: false,
            grid_alert_emitted: false,
            bess_grid_export_active: false,
            last_carbon_rate: None,
            fire_alert_active: false,
            fire_capacity_zeroed: false,
            last_fire_capacity_kva: None,
            next_interval_id: 1,
        }
    }
}

impl OpenAdrSiteState {
    pub fn next_interval(&mut self) -> i32 {
        let id = self.next_interval_id;
        self.next_interval_id += 1;
        id
    }
}

// ─────────────────────────────────────────────────────
//  OpenAdrMessageQueue (Resource)
// ─────────────────────────────────────────────────────

/// Central OpenADR state + log buffer.
#[derive(Resource)]
pub struct OpenAdrMessageQueue {
    /// Simulated start time: game-time 0 maps to this wall-clock instant.
    pub sim_start: DateTime<Utc>,

    /// In-memory event log for the JS overlay (accumulates all messages).
    pub event_log: Vec<OpenAdrLogEntry>,

    /// Whether the event log is enabled.
    pub event_log_enabled: bool,

    /// Total number of entries drained from the front of `event_log` over its
    /// lifetime. Feed systems use this to convert their monotonic
    /// `last_pushed_index` back to a relative Vec index after trimming.
    pub total_drained: usize,

    /// Whether the DR program definition has been emitted.
    pub program_registered: bool,

    /// Per-site DER state, keyed by `SiteId`.
    pub site_state: HashMap<SiteId, OpenAdrSiteState>,
}

impl Default for OpenAdrMessageQueue {
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
            total_drained: 0,
            program_registered: false,
            site_state: HashMap::new(),
        }
    }
}

impl OpenAdrMessageQueue {
    /// Whether the feed is active (log accumulation enabled).
    pub fn is_active(&self) -> bool {
        self.event_log_enabled
    }

    /// Convert a `total_game_time` value to a `DateTime<Utc>` timestamp.
    pub fn game_time_to_utc(&self, total_game_time: f32) -> DateTime<Utc> {
        self.sim_start + Duration::seconds(total_game_time as i64)
    }

    /// Get or create the per-site state for a site.
    pub fn get_or_create(&mut self, site_id: SiteId) -> &mut OpenAdrSiteState {
        self.site_state.entry(site_id).or_default()
    }

    /// Push a log entry to the event log, trimming old entries if over capacity.
    pub fn push_log(
        &mut self,
        ven_id: String,
        timestamp_iso: String,
        message_type: &str,
        action: &str,
        json: String,
    ) {
        if !self.event_log_enabled {
            return;
        }
        self.event_log.push(OpenAdrLogEntry {
            timestamp: timestamp_iso,
            ven_id,
            message_type: message_type.to_string(),
            action: action.to_string(),
            msg: json,
        });
        if self.event_log.len() > MAX_EVENT_LOG {
            let excess = self.event_log.len() - MAX_EVENT_LOG;
            self.event_log.drain(..excess);
            self.total_drained += excess;
        }
    }
}
