//! Hacker component for cyber-attack events
//!
//! Hackers walk in a straight line to/from infrastructure — they don't follow
//! the road/pathfinding grid. They enter and exit from random map edges.

use bevy::prelude::*;

use crate::resources::multi_site::SiteId;

/// Movement phase for the hacker
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum HackerPhase {
    #[default]
    Infiltrating,
    Hacking,
    Fleeing,
    Gone,
}

/// Type of cyber-attack the hacker will execute
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum HackerAttackType {
    /// Reconfigure power to overload transformer and trigger fire
    TransformerOverload,
    /// Slash charging price to $0.01/kWh — "power to the people"
    PriceSlash,
}

/// Visual variant of the hacker (determines sprite tint)
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum HackerVariant {
    Green,
    Purple,
}

/// Hacker entity component (spawned by the hacker spawn system)
#[derive(Component, Debug, Clone)]
pub struct Hacker {
    /// Which site this hacker is targeting
    pub target_site: SiteId,
    /// World-space position the hacker is targeting (transformer or charger)
    pub target_pos: Vec2,
    /// Current phase of the hack
    pub phase: HackerPhase,
    /// Type of attack
    pub attack_type: HackerAttackType,
    /// Countdown timer during hacking phase (game seconds)
    pub hack_timer: f32,
    /// World-space position the hacker is currently moving toward
    pub move_target: Vec2,
    /// Display name of the hacker
    pub name: &'static str,
    /// Visual variant
    pub variant: HackerVariant,
    /// Walk animation timer (accumulates real time for bobbing/sway)
    pub anim_timer: f32,
    /// Base Y position (set when spawning/changing phase, used for bob offset)
    pub base_y: f32,
}

/// Marker for the hacker's visual sprite entity
#[derive(Component, Debug)]
pub struct HackerSprite {
    pub hacker_entity: Entity,
}

/// Pool of hacker names, randomly assigned on spawn
pub const HACKER_NAMES: &[&str] = &[
    "Zero Cool",
    "Crash Override",
    "The Phantom",
    "Script Kiddie",
    "Root Access",
    "Pwn3d",
];
