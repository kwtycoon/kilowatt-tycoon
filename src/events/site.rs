//! Site-related events

use crate::resources::SiteId;
use bevy::prelude::*;

/// Event triggered when player wants to switch to a different site
#[derive(Event, Message, Debug, Clone, Copy)]
pub struct SiteSwitchEvent {
    pub target_site_id: SiteId,
}

/// Event triggered when a site is sold
#[derive(Event, Message, Debug, Clone, Copy)]
pub struct SiteSoldEvent {
    pub site_id: SiteId,
    pub refund_amount: f32,
}
