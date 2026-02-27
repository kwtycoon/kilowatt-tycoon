//! Site configuration resource (loaded from JSON)

use bevy::prelude::*;
use serde::{Deserialize, Serialize};

use crate::components::charger::{ChargerType, FaultType, Phase};
use crate::components::driver::{PatienceLevel, VehicleType};

/// Site configuration loaded from JSON
#[derive(Resource, Debug, Clone, Default, Serialize, Deserialize)]
pub struct SiteConfig {
    pub id: String,
    pub name: String,
    #[serde(rename = "type")]
    pub site_type: String,
    pub contracted_capacity_kva: f32,
    pub grid_voltage: f32,
    #[serde(default)]
    pub phases: Vec<String>,
    #[serde(default)]
    pub chargers: Vec<String>,
    #[serde(default)]
    pub technician: TechnicianConfig,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct TechnicianConfig {
    pub skill_level: i32,
    pub on_site: bool,
    pub hourly_rate: f32,
}

/// Charger data loaded from JSON
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChargerData {
    pub id: String,
    #[serde(rename = "type")]
    pub charger_type: ChargerType,
    pub rated_power_kw: f32,
    pub phase: Phase,
    pub health: f32,
    pub position: Position2D,
    #[serde(default)]
    pub scripted_fault: Option<ScriptedFault>,
    #[serde(default)]
    pub connector_jam_chance: f32,
    #[serde(default)]
    pub notes: String,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Position2D {
    pub x: f32,
    pub y: f32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScriptedFault {
    #[serde(rename = "type")]
    pub fault_type: FaultType,
    pub trigger_time: f32,
}

/// Driver schedule loaded from JSON
#[derive(Resource, Debug, Clone, Default, Serialize, Deserialize)]
pub struct DriverSchedule {
    #[serde(default)]
    pub scenario_id: String,
    #[serde(default)]
    pub name: String,
    #[serde(default)]
    pub duration_game_seconds: f32,
    #[serde(default)]
    pub revenue_target: f32,
    #[serde(default)]
    pub drivers: Vec<DriverData>,
    #[serde(default)]
    pub scripted_events: Vec<ScriptedEvent>,
    /// Index of next driver to spawn
    #[serde(skip)]
    pub next_driver_index: usize,
    /// Index of next scripted event to process
    #[serde(skip)]
    pub next_event_index: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DriverData {
    pub id: String,
    #[serde(default)]
    pub vehicle: VehicleType,
    #[serde(default)]
    pub vehicle_name: String,
    pub arrival_time: f32,
    pub target_charger: Option<String>,
    #[serde(default)]
    pub patience: PatienceLevel,
    #[serde(default = "default_charge_needed")]
    pub charge_needed_kwh: f32,
    #[serde(default)]
    pub notes: String,
}

fn default_charge_needed() -> f32 {
    45.0
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScriptedEvent {
    pub time: f32,
    #[serde(rename = "type")]
    pub event_type: String,
    #[serde(default)]
    pub charger_id: Option<String>,
    #[serde(default)]
    pub fault_type: Option<FaultType>,
    #[serde(default)]
    pub temp_threshold: Option<f32>,
    #[serde(default)]
    pub phase: Option<String>,
    #[serde(default)]
    pub notes: String,
}
