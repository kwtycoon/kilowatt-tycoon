//! Port/connector registry for analytics consumers.
//!
//! Tracks the mapping from charge point → port → connector that analytics
//! pipelines (e.g. kwwhat) need alongside raw OCPP logs.

use bevy::prelude::*;

/// Registry of all known charger ports/connectors.
///
/// Populated during [`super::message_gen::ocpp_boot_system`] when a charger
/// is first seen. Game chargers have a single port with a single connector.
#[derive(Resource, Default)]
pub struct PortsRegistry {
    pub entries: Vec<PortEntry>,
}

/// A single port/connector record in the shape kwwhat expects.
#[derive(Debug, Clone, serde::Serialize)]
pub struct PortEntry {
    pub charge_point_id: String,
    pub location_id: String,
    pub port_id: String,
    pub connector_id: String,
    pub connector_type: String,
    pub commissioned_ts: Option<String>,
}
