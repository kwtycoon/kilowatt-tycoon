//! ECS Components for ChargeOps Simulator

pub mod charger;
pub mod driver;
pub mod emotion;
pub mod hacker;
pub mod power;
pub mod robber;
pub mod site;
pub mod technician;
pub mod ticket;
pub mod traffic;

use bevy::prelude::*;

pub use charger::*;
pub use driver::*;
pub use emotion::*;
pub use hacker::*;
pub use power::*;
pub use robber::*;
pub use site::*;
pub use technician::*;
pub use ticket::*;
pub use traffic::*;

/// Plugin that registers all component-related systems
pub struct ComponentsPlugin;

impl Plugin for ComponentsPlugin {
    fn build(&self, _app: &mut App) {
        // Components are registered automatically when used
        // This plugin exists for organizational purposes
    }
}
