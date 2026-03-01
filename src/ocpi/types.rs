//! OCPI 2.3.0 type definitions for the game's roaming feed.
//!
//! Type structures are adapted from the `ocpi` crate (v0.3.5, MIT licensed)
//! and updated to match the OCPI 2.3.0 specification:
//!   - `CiString<N>` / `CsString<N>` replaced with plain `String`
//!   - Custom `DateTime` replaced with `String` (ISO 8601 / RFC 3339)
//!   - `Price` uses v2.3.0 `before_taxes` + optional `taxes[]`
//!   - `CdrToken` includes `country_code` and `party_id`
//!   - `Session` includes `connector_id`
//!
//! Reference: <https://ocpi.fyi/ocpi/2.3.0/spec/>

use serde::Serialize;

// ─── Constants ───────────────────────────────────────

pub const CPO_COUNTRY_CODE: &str = "US";
pub const CPO_PARTY_ID: &str = "KWT";
pub const CURRENCY: &str = "USD";

pub const EMSP_COUNTRY_CODE: &str = "US";
pub const EMSP_PARTY_ID: &str = "EVC";
pub const EMSP_ISSUER: &str = "EVConnect";

// ─── Location / EVSE / Connector ─────────────────────

#[derive(Clone, Debug, Serialize)]
pub struct Location {
    pub country_code: String,
    pub party_id: String,
    pub id: String,
    pub name: String,
    pub address: String,
    pub city: String,
    pub postal_code: String,
    pub country: String,
    pub coordinates: GeoLocation,
    pub evses: Vec<Evse>,
    pub last_updated: String,
}

#[derive(Clone, Debug, Serialize)]
pub struct Evse {
    pub uid: String,
    pub evse_id: String,
    pub status: EvseStatus,
    pub connectors: Vec<Connector>,
    pub last_updated: String,
}

#[derive(Clone, Debug, Serialize)]
pub struct Connector {
    pub id: String,
    pub standard: ConnectorType,
    pub format: ConnectorFormat,
    pub power_type: PowerType,
    pub max_voltage: i32,
    pub max_amperage: i32,
    pub max_electric_power: i32,
    pub last_updated: String,
}

#[derive(Clone, Debug, Serialize)]
pub struct GeoLocation {
    pub latitude: String,
    pub longitude: String,
}

// ─── EVSE Status ─────────────────────────────────────

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum EvseStatus {
    Available,
    Blocked,
    Charging,
    Inoperative,
    OutOfOrder,
    Planned,
    Removed,
}

// ─── EVSE Status Update (lightweight PATCH) ──────────

#[derive(Clone, Debug, Serialize)]
pub struct EvseStatusUpdate {
    pub location_id: String,
    pub evse_uid: String,
    pub evse_id: String,
    pub status: EvseStatus,
    pub last_updated: String,
}

// ─── Connector enums ─────────────────────────────────

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum ConnectorType {
    /// SAE J1772 (US Level 2)
    #[serde(rename = "IEC_62196_T1")]
    Iec62196T1,
    /// CCS1 (US DC fast)
    #[serde(rename = "IEC_62196_T1_COMBO")]
    Iec62196T1Combo,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum ConnectorFormat {
    Socket,
    Cable,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum PowerType {
    #[serde(rename = "AC_1_PHASE")]
    Ac1Phase,
    #[serde(rename = "AC_3_PHASE")]
    Ac3Phase,
    Dc,
}

// ─── Session ─────────────────────────────────────────

#[derive(Clone, Debug, Serialize)]
pub struct Session {
    pub country_code: String,
    pub party_id: String,
    pub id: String,
    pub start_date_time: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub end_date_time: Option<String>,
    pub kwh: f64,
    pub cdr_token: CdrToken,
    pub auth_method: AuthMethod,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub authorization_reference: Option<String>,
    pub location_id: String,
    pub evse_uid: String,
    pub connector_id: String,
    pub currency: String,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub charging_periods: Vec<ChargingPeriod>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub total_cost: Option<Price>,
    pub status: SessionStatus,
    pub last_updated: String,
}

/// Partial session update for PATCH semantics.
#[derive(Clone, Debug, Serialize)]
pub struct SessionPatch {
    pub kwh: f64,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub charging_periods: Vec<ChargingPeriod>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub total_cost: Option<Price>,
    pub last_updated: String,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum SessionStatus {
    Active,
    Completed,
    Invalid,
    Pending,
    Reservation,
}

// ─── CDR (Charge Detail Record) ──────────────────────

#[derive(Clone, Debug, Serialize)]
pub struct Cdr {
    pub country_code: String,
    pub party_id: String,
    pub id: String,
    pub start_date_time: String,
    pub end_date_time: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub session_id: Option<String>,
    pub cdr_token: CdrToken,
    pub auth_method: AuthMethod,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub authorization_reference: Option<String>,
    pub cdr_location: CdrLocation,
    pub currency: String,
    pub charging_periods: Vec<ChargingPeriod>,
    pub total_cost: Price,
    pub total_energy: f64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub total_energy_cost: Option<Price>,
    pub total_time: f64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub total_time_cost: Option<Price>,
    pub last_updated: String,
}

// ─── CdrLocation (frozen snapshot) ───────────────────

#[derive(Clone, Debug, Serialize)]
pub struct CdrLocation {
    pub id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    pub address: String,
    pub city: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub postal_code: Option<String>,
    pub country: String,
    pub coordinates: GeoLocation,
    pub evse_uid: String,
    pub evse_id: String,
    pub connector_id: String,
    pub connector_standard: ConnectorType,
    pub connector_format: ConnectorFormat,
    pub connector_power_type: PowerType,
}

// ─── CdrToken ────────────────────────────────────────

#[derive(Clone, Debug, Serialize)]
pub struct CdrToken {
    pub country_code: String,
    pub party_id: String,
    pub uid: String,
    #[serde(rename = "type")]
    pub token_type: TokenType,
    pub contract_id: String,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum TokenType {
    AdHocUser,
    AppUser,
    Other,
    Rfid,
}

// ─── AuthMethod ──────────────────────────────────────

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum AuthMethod {
    AuthRequest,
    Command,
    Whitelist,
}

// ─── ChargingPeriod / CdrDimension ──────────────────

#[derive(Clone, Debug, Serialize)]
pub struct ChargingPeriod {
    pub start_date_time: String,
    pub dimensions: Vec<CdrDimension>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tariff_id: Option<String>,
}

#[derive(Clone, Debug, Serialize)]
pub struct CdrDimension {
    #[serde(rename = "type")]
    pub dimension_type: CdrDimensionType,
    pub volume: f64,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum CdrDimensionType {
    Current,
    Energy,
    EnergyExport,
    EnergyImport,
    MaxCurrent,
    MinCurrent,
    MaxPower,
    MinPower,
    ParkingTime,
    Power,
    ReservationTime,
    StateOfCharge,
    Time,
}

// ─── Price (v2.3.0) ─────────────────────────────────

#[derive(Clone, Debug, Serialize)]
pub struct Price {
    pub before_taxes: f64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub taxes: Option<Vec<Tax>>,
}

#[derive(Clone, Debug, Serialize)]
pub struct Tax {
    pub name: String,
    pub amount: f64,
}

// ─── Tariff (v2.3.0) ─────────────────────────────────

#[derive(Clone, Debug, Serialize)]
pub struct Tariff {
    pub country_code: String,
    pub party_id: String,
    pub id: String,
    pub currency: String,
    pub elements: Vec<TariffElement>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tariff_alt_text: Option<Vec<DisplayText>>,
    pub last_updated: String,
}

#[derive(Clone, Debug, Serialize)]
pub struct TariffElement {
    pub price_components: Vec<PriceComponent>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub restrictions: Option<TariffRestrictions>,
}

#[derive(Clone, Debug, Serialize)]
pub struct PriceComponent {
    #[serde(rename = "type")]
    pub component_type: TariffDimensionType,
    pub price: f64,
    pub step_size: i32,
}

#[derive(Clone, Copy, Debug, Serialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum TariffDimensionType {
    Energy,
    FlatRate,
    ParkingTime,
    Time,
}

#[derive(Clone, Debug, Serialize)]
pub struct TariffRestrictions {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub start_time: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub end_time: Option<String>,
}

#[derive(Clone, Debug, Serialize)]
pub struct DisplayText {
    pub language: String,
    pub text: String,
}

// ─── Commands module (eMSP → CPO) ───────────────────

#[derive(Clone, Debug, Serialize)]
pub struct Token {
    pub country_code: String,
    pub party_id: String,
    pub uid: String,
    #[serde(rename = "type")]
    pub token_type: TokenType,
    pub contract_id: String,
    pub issuer: String,
    pub valid: bool,
    pub whitelist: WhitelistType,
    pub last_updated: String,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum WhitelistType {
    Always,
    Allowed,
    AllowedOffline,
    Never,
}

#[derive(Clone, Debug, Serialize)]
pub struct StartSessionCommand {
    pub response_url: String,
    pub token: Token,
    pub location_id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub evse_uid: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub connector_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub authorization_reference: Option<String>,
}

#[derive(Clone, Debug, Serialize)]
pub struct CommandResponse {
    pub result: CommandResponseType,
    pub timeout: i32,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub message: Option<Vec<DisplayText>>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum CommandResponseType {
    NotSupported,
    Rejected,
    Accepted,
    UnknownSession,
}

#[derive(Clone, Debug, Serialize)]
pub struct CommandResult {
    pub result: CommandResultType,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub message: Option<Vec<DisplayText>>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum CommandResultType {
    Accepted,
    CanceledReservation,
    EvseOccupied,
    EvseInoperative,
    Failed,
    NotSupported,
    Rejected,
    Timeout,
    UnknownReservation,
}

// ─── Helpers ─────────────────────────────────────────

pub fn serialize_ocpi(value: &impl Serialize) -> String {
    serde_json::to_string(value).unwrap_or_default()
}
