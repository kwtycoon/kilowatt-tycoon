//! Technician component and movement types
//!
//! Technicians use bevy_northstar for pathfinding, similar to vehicles.
//! The `TechnicianMovement` component tracks the current phase and speed,
//! while `AgentPos` and `Pathfind` handle the actual pathfinding.

use bevy::prelude::*;

/// Movement phase for technician animation
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum TechnicianPhase {
    #[default]
    WalkingToCharger, // Walking from entry to charger
    Working,       // Performing repair at charger
    WalkingToExit, // Walking from charger to exit
    Exited,        // Off-screen, ready for cleanup
}

/// Technician entity component (spawned when on-site)
#[derive(Component, Debug, Clone)]
pub struct Technician {
    /// Entity of the charger being repaired
    pub target_charger: Entity,
    /// Current phase of the repair job
    pub phase: TechnicianPhase,
    /// Animation timer for working animation (accumulates time in seconds)
    pub work_timer: f32,
    /// Target grid position for the charger (used for arrival detection)
    pub target_bay: Option<(i32, i32)>,
}

/// Technician movement component for smooth animations.
///
/// Pathfinding is handled by bevy_northstar's `AgentPos`, `Pathfind`, and `NextPos`.
/// This component tracks phase and speed for the movement system.
#[derive(Component, Debug, Clone)]
pub struct TechnicianMovement {
    /// Current movement phase
    pub phase: TechnicianPhase,
    /// Movement speed (pixels per second)
    pub speed: f32,
}

impl Default for TechnicianMovement {
    fn default() -> Self {
        Self {
            phase: TechnicianPhase::WalkingToCharger,
            speed: 60.0, // Slower walking speed than vehicles
        }
    }
}
