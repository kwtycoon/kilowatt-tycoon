pub mod helpers;
pub mod interactions;
pub mod report;
pub mod sections;
pub mod ui;

use bevy::prelude::*;

/// Marker for the collapsed KPI view (flat summary rows).
#[derive(Component, Debug, Clone, Copy)]
pub(crate) struct KpiCollapsed;

/// Marker for the expanded KPI view (full Revenue/Energy/Operations breakdown).
#[derive(Component, Debug, Clone, Copy)]
pub(crate) struct KpiExpanded;

/// Marker for the modal container so the toggle system can change its width.
#[derive(Component, Debug, Clone, Copy)]
pub(crate) struct DayEndModalContainer;

/// Share on LinkedIn button marker.
#[derive(Component, Debug, Clone, Copy)]
pub(crate) struct LinkedInShareButton;

/// Resource storing LinkedIn share text for the current day end.
#[derive(Resource, Debug, Clone)]
pub(crate) struct DayEndShareText(pub String);
