//! Driver component and related types

use bevy::prelude::*;
use serde::{Deserialize, Serialize};

use crate::components::charger::ChargerType;

/// Driver patience level (affects wait tolerance)
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum PatienceLevel {
    VeryLow,
    Low,
    #[default]
    Medium,
    High,
}

impl PatienceLevel {
    /// Initial patience value (0-100)
    pub fn initial_patience(&self) -> f32 {
        match self {
            PatienceLevel::VeryLow => 25.0,
            PatienceLevel::Low => 50.0,
            PatienceLevel::Medium => 75.0,
            PatienceLevel::High => 100.0,
        }
    }

    /// Patience depletion rate per game minute while waiting
    pub fn depletion_rate(&self) -> f32 {
        match self {
            PatienceLevel::VeryLow => 20.0,
            PatienceLevel::Low => 15.0,
            PatienceLevel::Medium => 10.0,
            PatienceLevel::High => 5.0,
        }
    }
}

/// Vehicle type for visual variety
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum VehicleType {
    Compact,
    #[default]
    Sedan,
    Suv,
    Crossover,
    Pickup,
    Bus,
    Semi,
    Tractor,
    Scooter,
    Motorcycle,
}

impl VehicleType {
    pub fn sprite_name(&self) -> &'static str {
        match self {
            VehicleType::Compact => "vehicle_compact",
            VehicleType::Sedan => "vehicle_sedan",
            VehicleType::Suv => "vehicle_suv",
            VehicleType::Crossover => "vehicle_crossover",
            VehicleType::Pickup => "vehicle_pickup",
            VehicleType::Bus => "vehicle_bus",
            VehicleType::Semi => "vehicle_semi",
            VehicleType::Tractor => "vehicle_tractor",
            VehicleType::Scooter => "vehicle_scooter",
            VehicleType::Motorcycle => "vehicle_motorcycle",
        }
    }

    /// Vehicle length in tiles for traffic occupancy.
    ///
    /// This is intentionally 1-tile wide for now; long vehicles occupy multiple tiles
    /// along their travel direction.
    pub fn footprint_length_tiles(&self) -> u8 {
        match self {
            VehicleType::Bus => 2,
            VehicleType::Semi => 3,
            // Everything else behaves like a single-tile vehicle.
            _ => 1,
        }
    }

    /// Returns which charger types this vehicle can use.
    ///
    /// - Scooters/Motorcycles: L2 only (small batteries, typically no CCS port)
    /// - Bus/Semi/Tractor: DCFC only (L2 would take 30-70+ hours)
    /// - All others: Both DCFC and L2, with DCFC preferred (listed first)
    pub fn compatible_charger_types(&self) -> &'static [ChargerType] {
        match self {
            VehicleType::Scooter | VehicleType::Motorcycle => &[ChargerType::AcLevel2],
            VehicleType::Bus | VehicleType::Semi | VehicleType::Tractor => &[ChargerType::DcFast],
            _ => &[ChargerType::DcFast, ChargerType::AcLevel2],
        }
    }

    /// Returns the preferred charger type (first in compatibility list).
    pub fn preferred_charger_type(&self) -> ChargerType {
        self.compatible_charger_types()[0]
    }

    /// Check if this vehicle is compatible with the given charger type.
    pub fn is_compatible_with(&self, charger_type: ChargerType) -> bool {
        self.compatible_charger_types().contains(&charger_type)
    }
}

/// Driver behavioral state
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default, Reflect)]
pub enum DriverState {
    #[default]
    Arriving,
    WaitingForCharger,
    Queued, // Waiting in queue for a charger to become available
    Charging,
    Frustrated, // Arrived at broken charger
    Complete,
    Leaving,
    LeftAngry,
}

/// Driver mood for sprite selection
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum DriverMood {
    #[default]
    Neutral,
    Impatient,
    Angry,
    Happy,
}

impl DriverMood {
    pub fn sprite_suffix(&self) -> &'static str {
        match self {
            DriverMood::Neutral => "neutral",
            DriverMood::Impatient => "impatient",
            DriverMood::Angry => "angry",
            DriverMood::Happy => "happy",
        }
    }
}

/// Main driver component
#[derive(Component, Debug, Clone)]
pub struct Driver {
    pub id: String,
    pub vehicle_name: String,
    pub vehicle_type: VehicleType,
    pub patience_level: PatienceLevel,
    pub patience: f32,
    pub charge_needed_kwh: f32,
    pub charge_received_kwh: f32,
    pub state: DriverState,
    pub target_charger_id: Option<String>,
    pub assigned_charger: Option<Entity>,
    pub assigned_bay: Option<(i32, i32)>,
    pub mood: DriverMood,
}

impl Default for Driver {
    fn default() -> Self {
        Self {
            id: String::new(),
            vehicle_name: String::new(),
            vehicle_type: VehicleType::Sedan,
            patience_level: PatienceLevel::Medium,
            patience: 75.0,
            charge_needed_kwh: 30.0,
            charge_received_kwh: 0.0,
            state: DriverState::Arriving,
            target_charger_id: None,
            assigned_charger: None,
            assigned_bay: None,
            mood: DriverMood::Neutral,
        }
    }
}

impl Driver {
    pub fn update_mood(&mut self) {
        let patience_pct = (self.patience / self.patience_level.initial_patience()) * 100.0;
        self.mood = match patience_pct {
            p if p >= 75.0 => DriverMood::Neutral,
            p if p >= 50.0 => DriverMood::Impatient,
            p if p >= 25.0 => DriverMood::Angry,
            _ => DriverMood::Angry,
        };
    }

    pub fn is_charging_complete(&self) -> bool {
        self.charge_received_kwh >= self.charge_needed_kwh
    }

    pub fn charge_progress(&self) -> f32 {
        (self.charge_received_kwh / self.charge_needed_kwh).clamp(0.0, 1.0)
    }
}

/// Component linking a driver entity to their charging session
#[derive(Component, Debug, Clone)]
pub struct ChargingSession {
    pub driver_entity: Entity,
    pub charger_entity: Entity,
    pub energy_delivered_kwh: f32,
    pub revenue_earned: f32,
    pub start_time: f32,
}

/// Marker for drivers in queue
#[derive(Component, Debug, Clone, Copy)]
pub struct InQueue {
    pub position: usize,
    pub target_charger: Option<Entity>,
}

// ============ Vehicle Movement ============

/// Movement phase for vehicle animation
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum MovementPhase {
    #[default]
    Arriving, // Driving from entry toward parking spot
    Parked,         // Stationary at charger
    DepartingHappy, // Reversing out and driving away (normal)
    DepartingAngry, // Quick departure (frustrated)
    Exited,         // Off-screen, ready for cleanup
}

/// Vehicle movement component for smooth animations.
///
/// # Dual Usage
///
/// This component is used in two different ways:
///
/// ## Customer Vehicles (Drivers)
/// Pathfinding is handled by bevy_northstar (`AgentPos`, `Pathfind`, `NextPos`).
/// Only `phase`, `speed`, and `target_rotation` are actively used.
/// The waypoint fields are set for backward compatibility but not used for movement.
///
/// ## Ambient Traffic
/// Uses manual waypoint interpolation. All fields are actively used:
/// `waypoints`, `current_waypoint`, and `progress` track movement along the path.
#[derive(Component, Debug, Clone)]
pub struct VehicleMovement {
    /// Current movement phase
    pub phase: MovementPhase,
    /// Waypoints to follow (world positions) - used by ambient traffic only
    pub waypoints: Vec<Vec2>,
    /// Current waypoint index - used by ambient traffic only
    pub current_waypoint: usize,
    /// Progress toward current waypoint (0.0 - 1.0) - used by ambient traffic only
    pub progress: f32,
    /// Movement speed (pixels per second)
    pub speed: f32,
    /// Target rotation (radians)
    pub target_rotation: f32,
}

impl Default for VehicleMovement {
    fn default() -> Self {
        Self {
            phase: MovementPhase::Arriving,
            waypoints: Vec::new(),
            current_waypoint: 0,
            progress: 0.0,
            speed: 150.0, // Default driving speed
            target_rotation: 0.0,
        }
    }
}

impl VehicleMovement {
    /// Check if movement is complete (for ambient traffic)
    pub fn is_complete(&self) -> bool {
        self.current_waypoint >= self.waypoints.len().saturating_sub(1) && self.progress >= 1.0
    }

    /// Get current position interpolated between waypoints (for ambient traffic)
    pub fn current_position(&self) -> Option<Vec2> {
        if self.waypoints.is_empty() {
            return None;
        }

        let from_idx = self.current_waypoint.min(self.waypoints.len() - 1);
        let to_idx = (self.current_waypoint + 1).min(self.waypoints.len() - 1);

        let from = self.waypoints[from_idx];
        let to = self.waypoints[to_idx];

        Some(from.lerp(to, self.progress))
    }

    /// Calculate rotation to face movement direction (for ambient traffic)
    pub fn calculate_rotation(&self) -> f32 {
        if self.waypoints.len() < 2 {
            return 0.0;
        }

        let from_idx = self.current_waypoint.min(self.waypoints.len() - 1);
        let to_idx = (self.current_waypoint + 1).min(self.waypoints.len() - 1);

        if from_idx == to_idx {
            return self.target_rotation;
        }

        let from = self.waypoints[from_idx];
        let to = self.waypoints[to_idx];
        let direction = to - from;

        if direction.length_squared() < 0.01 {
            return self.target_rotation;
        }

        // Calculate angle (0 = facing up, which is default for top-down cars)
        direction.x.atan2(direction.y)
    }
}
