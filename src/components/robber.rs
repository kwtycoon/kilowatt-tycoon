//! Robber component for cable theft events
//!
//! Robbers walk in a straight line to/from chargers — they don't follow
//! the road/pathfinding grid. They enter and exit from random map edges.

use bevy::prelude::*;

/// Movement phase for the robber
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum RobberPhase {
    #[default]
    WalkingToCharger, // Sneaking from edge to charger
    Stealing, // Cutting cable, alarm active
    Fleeing,  // Running to random edge after theft
    Gone,     // Off-screen, ready for cleanup
}

/// Visual variant of the robber (determines sprite set)
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum RobberVariant {
    /// Classic black outfit
    Black,
    /// Pink outfit
    Pink,
}

/// Robber entity component (spawned by the cable theft system)
#[derive(Component, Debug, Clone)]
pub struct Robber {
    /// Entity of the charger being targeted
    pub target_charger: Entity,
    /// Current phase of the robbery
    pub phase: RobberPhase,
    /// Countdown timer during stealing phase (game seconds)
    pub steal_timer: f32,
    /// World-space position the robber is currently moving toward
    pub move_target: Vec2,
    /// Display name of the robber
    pub name: &'static str,
    /// Visual variant (black or pink outfit)
    pub variant: RobberVariant,
    /// Walk animation timer (accumulates real time for bobbing/sway)
    pub anim_timer: f32,
    /// Base Y position (set when spawning/changing phase, used for bob offset)
    pub base_y: f32,
}

/// Marker for the robber's visual sprite entity
#[derive(Component, Debug)]
pub struct RobberSprite {
    /// The parent robber entity this sprite belongs to
    pub robber_entity: Entity,
}

/// Pool of robber names, randomly assigned on spawn
pub const ROBBER_NAMES: &[&str] = &[
    "Copper Jack",
    "Copperfield",
    "Sir Cuts-A-Lot",
    "OCPP-Op",
    "The Scrap King",
];
