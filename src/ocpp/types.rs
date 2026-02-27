//! OCPP 1.6J type re-exports and game-to-OCPP conversion helpers.
//!
//! Canonical OCPP types come from the `rust-ocpp` crate (crates.io).
//! The OCPP JSON-RPC Call envelope is handled by a thin `serialize_call`
//! helper in this module.

use serde::Serialize;

use crate::components::charger::{ChargerState, FaultType};

// ─── Re-exports: OCPP 1.6 message payloads ──────────────────────

pub use rust_ocpp::v1_6::messages::boot_notification::BootNotificationRequest;
pub use rust_ocpp::v1_6::messages::heart_beat::HeartbeatRequest;
pub use rust_ocpp::v1_6::messages::meter_values::MeterValuesRequest;
pub use rust_ocpp::v1_6::messages::start_transaction::StartTransactionRequest;
pub use rust_ocpp::v1_6::messages::status_notification::StatusNotificationRequest;
pub use rust_ocpp::v1_6::messages::stop_transaction::StopTransactionRequest;

// ─── Re-exports: OCPP 1.6 enums & shared types ──────────────────

pub use rust_ocpp::v1_6::types::{
    ChargePointErrorCode, ChargePointStatus, Location, Measurand, MeterValue, ReadingContext,
    Reason, SampledValue, UnitOfMeasure, ValueFormat,
};

// ─── OCPP 1.6J Call envelope ─────────────────────────────────────

/// OCPP message-type ID for a Call (request from Charge Point to CSMS).
const CALL: u8 = 2;

/// Serialize an OCPP 1.6J Call: `[2, "uniqueId", "Action", {payload}]`
pub fn serialize_call(unique_id: &str, action: &str, payload: &impl Serialize) -> String {
    let payload_value = serde_json::to_value(payload).unwrap_or_default();
    serde_json::to_string(&(CALL, unique_id, action, payload_value)).unwrap_or_default()
}

/// Generate a short unique ID for OCPP message correlation.
pub fn new_unique_id() -> String {
    use rand::Rng;
    let mut rng = rand::rng();
    format!("{:08x}-{:04x}", rng.random::<u32>(), rng.random::<u16>())
}

// ─── Game → OCPP conversions ─────────────────────────────────────

/// Map the game's [`ChargerState`] to an OCPP 1.6 [`ChargePointStatus`].
pub fn charger_state_to_ocpp_status(state: ChargerState) -> ChargePointStatus {
    match state {
        ChargerState::Available => ChargePointStatus::Available,
        ChargerState::Charging => ChargePointStatus::Charging,
        ChargerState::Warning => ChargePointStatus::Faulted,
        ChargerState::Offline => ChargePointStatus::Faulted,
        ChargerState::Disabled => ChargePointStatus::Unavailable,
    }
}

/// Map the game's [`FaultType`] to an OCPP 1.6 [`ChargePointErrorCode`].
pub fn fault_to_ocpp_error(fault: FaultType) -> ChargePointErrorCode {
    match fault {
        FaultType::CommunicationError => ChargePointErrorCode::EVCommunicationError,
        FaultType::PaymentError => ChargePointErrorCode::ReaderFailure,
        FaultType::FirmwareFault => ChargePointErrorCode::InternalError,
        FaultType::GroundFault => ChargePointErrorCode::GroundFailure,
        FaultType::CableDamage => ChargePointErrorCode::ConnectorLockFailure,
        FaultType::CableTheft => ChargePointErrorCode::OtherError,
    }
}
