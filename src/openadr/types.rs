//! OpenADR 3.0 type re-exports and game-to-OpenADR conversion helpers.
//!
//! Canonical OpenADR types come from the `openleadr-wire` crate (crates.io).
//! This mirrors the pattern in `crate::ocpp::types` for OCPP 1.6.

use serde::Serialize;

// ─── Re-exports: OpenADR 3.0 event types ──────────────────────

pub use openleadr_wire::event::{
    EventContent, EventInterval, EventPayloadDescriptor, EventType, EventValuesMap, Priority,
};

// ─── Re-exports: OpenADR 3.0 report types ─────────────────────

pub use openleadr_wire::report::{
    ReadingType, ReportContent, ReportPayloadDescriptor, ReportResource, ReportType,
    ReportValuesMap, ResourceName,
};

// ─── Re-exports: OpenADR 3.0 VEN types ────────────────────────

pub use openleadr_wire::ven::VenContent;

// ─── Re-exports: OpenADR 3.0 program types ────────────────────

pub use openleadr_wire::program::{
    PayloadDescriptor, ProgramContent, ProgramDescription, ProgramId,
};

// ─── Re-exports: OpenADR 3.0 resource types ───────────────────

pub use openleadr_wire::resource::ResourceContent;

// ─── Re-exports: OpenADR 3.0 target types ─────────────────────

pub use openleadr_wire::target::{TargetEntry, TargetMap, TargetType};

// ─── Re-exports: intervals, units, values, and metadata ───────

pub use openleadr_wire::OperatingState;
pub use openleadr_wire::Unit;
pub use openleadr_wire::interval::IntervalPeriod;
pub use openleadr_wire::values_map::{Value as OadrValue, ValueType, ValuesMap};

// ─── Helpers ──────────────────────────────────────────────────

/// Serialize any `openleadr-wire` type to its JSON wire representation.
pub fn serialize_openadr(value: &impl Serialize) -> String {
    serde_json::to_string(value).unwrap_or_default()
}

/// Parse a string into a `ProgramId`. Returns `None` if the string is invalid.
pub fn make_program_id(s: &str) -> Option<ProgramId> {
    s.parse().ok()
}

/// Convenience: wrap an f32 game value into an OpenADR `Value::Number`.
pub fn ovalue_f32(v: f32) -> OadrValue {
    OadrValue::Number(v as f64)
}

/// Convenience: wrap a string into an OpenADR `Value::String`.
pub fn ovalue_str(s: &str) -> OadrValue {
    OadrValue::String(s.to_string())
}

/// Convenience: create a `ValuesMap` entry from a type name and single float.
pub fn values_map_f32(type_name: &str, val: f32) -> ValuesMap {
    ValuesMap {
        value_type: ValueType(type_name.to_string()),
        values: vec![ovalue_f32(val)],
    }
}

/// Convenience: create a `ValuesMap` entry from a type name and string value.
pub fn values_map_str(type_name: &str, val: &str) -> ValuesMap {
    ValuesMap {
        value_type: ValueType(type_name.to_string()),
        values: vec![ovalue_str(val)],
    }
}
