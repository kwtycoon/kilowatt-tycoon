//! OCPI 2.3.0 message generation systems.
//!
//! These Bevy systems observe charger/driver state and produce OCPI-shaped
//! messages (Location, Session, CDR, EVSE status) pushed into the
//! [`OcpiMessageQueue`].

use bevy::prelude::*;

use crate::components::charger::{Charger, ChargerState, ChargerType};
use crate::components::driver::Driver;
use crate::components::site::BelongsToSite;
use crate::resources::multi_site::SiteId;
use crate::resources::{GameClock, MultiSiteManager};

use super::queue::{OcpiMessageQueue, SESSION_UPDATE_INTERVAL_GAME_SECS};
use super::types::*;

// Simulated address constants (game doesn't model real-world addresses)
const SIM_ADDRESS: &str = "123 Charging Lane";
const SIM_CITY: &str = "Voltsville";
const SIM_POSTAL: &str = "90210";
const SIM_COUNTRY_ISO3: &str = "USA";
const SIM_LAT: &str = "34.0522";
const SIM_LON: &str = "-118.2437";

// ─────────────────────────────────────────────────────
//  Helpers
// ─────────────────────────────────────────────────────

fn charger_state_to_evse_status(state: ChargerState) -> EvseStatus {
    match state {
        ChargerState::Available => EvseStatus::Available,
        ChargerState::Charging => EvseStatus::Charging,
        ChargerState::Warning => EvseStatus::Inoperative,
        ChargerState::Offline => EvseStatus::OutOfOrder,
        ChargerState::Disabled => EvseStatus::Planned,
    }
}

fn connector_type_for(ct: ChargerType) -> ConnectorType {
    match ct {
        ChargerType::DcFast => ConnectorType::Iec62196T1Combo,
        ChargerType::AcLevel2 => ConnectorType::Iec62196T1,
    }
}

fn power_type_for(ct: ChargerType) -> PowerType {
    match ct {
        ChargerType::DcFast => PowerType::Dc,
        ChargerType::AcLevel2 => PowerType::Ac1Phase,
    }
}

fn max_voltage_for(ct: ChargerType) -> i32 {
    match ct {
        ChargerType::DcFast => 500,
        ChargerType::AcLevel2 => 240,
    }
}

fn make_evse_id(charger_id: &str) -> String {
    let hash: u32 = charger_id
        .bytes()
        .fold(0u32, |acc, b| acc.wrapping_mul(31).wrapping_add(b as u32));
    format!("US*KWT*E{:03}", hash % 1000)
}

fn make_location_id(site_id: SiteId) -> String {
    format!("LOC-site-{}", site_id.0)
}

fn make_cdr_token(evcc_id: &str) -> CdrToken {
    CdrToken {
        country_code: CPO_COUNTRY_CODE.to_string(),
        party_id: CPO_PARTY_ID.to_string(),
        uid: evcc_id.to_string(),
        token_type: TokenType::AppUser,
        contract_id: format!("US-KWT-{evcc_id}"),
    }
}

// ─────────────────────────────────────────────────────
//  1. Location PUT system
// ─────────────────────────────────────────────────────

pub fn ocpi_location_system(
    chargers: Query<(Entity, &Charger, Option<&BelongsToSite>)>,
    multi_site: Res<MultiSiteManager>,
    mut queue: ResMut<OcpiMessageQueue>,
    game_clock: Res<GameClock>,
) {
    if !queue.is_active() {
        return;
    }

    for (entity, charger, site_tag) in chargers.iter() {
        let state = queue.get_or_create(entity);
        if state.location_pushed {
            continue;
        }
        state.location_pushed = true;
        state.charger_id = charger.id.clone();

        let ts_iso = queue
            .game_time_to_utc(game_clock.total_game_time)
            .to_rfc3339();

        let site_id = site_tag.map(|s| s.site_id).unwrap_or(SiteId(0));
        let site_name = multi_site
            .get_site(site_id)
            .map(|s| s.name.as_str())
            .unwrap_or("Unknown Site");
        let location_id = make_location_id(site_id);

        let max_v = max_voltage_for(charger.charger_type);
        let max_w = (charger.rated_power_kw * 1000.0) as i32;
        let max_a = if max_v > 0 { max_w / max_v } else { 0 };

        let loc = Location {
            country_code: CPO_COUNTRY_CODE.to_string(),
            party_id: CPO_PARTY_ID.to_string(),
            id: location_id.clone(),
            name: site_name.to_string(),
            address: SIM_ADDRESS.to_string(),
            city: SIM_CITY.to_string(),
            postal_code: SIM_POSTAL.to_string(),
            country: SIM_COUNTRY_ISO3.to_string(),
            coordinates: GeoLocation {
                latitude: SIM_LAT.to_string(),
                longitude: SIM_LON.to_string(),
            },
            evses: vec![Evse {
                uid: charger.id.clone(),
                evse_id: make_evse_id(&charger.id),
                status: charger_state_to_evse_status(charger.state()),
                connectors: vec![Connector {
                    id: "1".to_string(),
                    standard: connector_type_for(charger.charger_type),
                    format: ConnectorFormat::Cable,
                    power_type: power_type_for(charger.charger_type),
                    max_voltage: max_v,
                    max_amperage: max_a,
                    max_electric_power: max_w,
                    last_updated: ts_iso.clone(),
                }],
                last_updated: ts_iso.clone(),
            }],
            last_updated: ts_iso.clone(),
        };

        queue.push_log(
            CPO_PARTY_ID.to_string(),
            ts_iso,
            "Location",
            "Location PUT",
            serialize_ocpi(&loc),
        );
    }
}

// ─────────────────────────────────────────────────────
//  2. EVSE Status system
// ─────────────────────────────────────────────────────

pub fn ocpi_status_system(
    chargers: Query<(Entity, &Charger, Option<&BelongsToSite>)>,
    mut queue: ResMut<OcpiMessageQueue>,
    game_clock: Res<GameClock>,
) {
    if !queue.is_active() {
        return;
    }

    for (entity, charger, site_tag) in chargers.iter() {
        let current = charger.state();
        let state = queue.get_or_create(entity);

        if !state.location_pushed {
            continue;
        }

        if state.last_status == Some(current) {
            continue;
        }
        state.last_status = Some(current);

        let ts_iso = queue
            .game_time_to_utc(game_clock.total_game_time)
            .to_rfc3339();
        let site_id = site_tag.map(|s| s.site_id).unwrap_or(SiteId(0));

        let update = EvseStatusUpdate {
            location_id: make_location_id(site_id),
            evse_uid: charger.id.clone(),
            evse_id: make_evse_id(&charger.id),
            status: charger_state_to_evse_status(current),
            last_updated: ts_iso.clone(),
        };

        queue.push_log(
            CPO_PARTY_ID.to_string(),
            ts_iso,
            "EVSE",
            "EVSE Status",
            serialize_ocpi(&update),
        );
    }
}

// ─────────────────────────────────────────────────────
//  3. Session PUT (start) system
// ─────────────────────────────────────────────────────

pub fn ocpi_session_start_system(
    chargers: Query<(Entity, &Charger, Option<&BelongsToSite>)>,
    drivers: Query<(Entity, &Driver)>,
    mut queue: ResMut<OcpiMessageQueue>,
    game_clock: Res<GameClock>,
) {
    if !queue.is_active() {
        return;
    }

    for (entity, charger, site_tag) in chargers.iter() {
        if !charger.is_charging {
            continue;
        }

        let needs_start = {
            let state = queue.get_or_create(entity);
            if !state.location_pushed {
                continue;
            }
            state.session_id.is_none()
        };

        if !needs_start {
            continue;
        }

        let driver_info = drivers
            .iter()
            .find(|(_, d)| d.assigned_charger == Some(entity))
            .map(|(de, d)| (de, d.evcc_id.clone()));

        let (driver_entity, evcc_id) = match driver_info {
            Some((de, id)) => (Some(de), id),
            None => (None, "000000000000".to_string()),
        };

        let session_num = queue.next_session_id();
        let start_game_time = charger
            .session_start_game_time
            .unwrap_or(game_clock.total_game_time);
        let start_ts = queue.game_time_to_utc(start_game_time).to_rfc3339();
        let site_id = site_tag.map(|s| s.site_id).unwrap_or(SiteId(0));

        let session = Session {
            country_code: CPO_COUNTRY_CODE.to_string(),
            party_id: CPO_PARTY_ID.to_string(),
            id: format!("SES-{session_num:05}"),
            start_date_time: start_ts.clone(),
            end_date_time: None,
            kwh: 0.0,
            cdr_token: make_cdr_token(&evcc_id),
            auth_method: AuthMethod::Whitelist,
            authorization_reference: Some(evcc_id.clone()),
            location_id: make_location_id(site_id),
            evse_uid: charger.id.clone(),
            connector_id: "1".to_string(),
            currency: CURRENCY.to_string(),
            charging_periods: Vec::new(),
            total_cost: None,
            status: SessionStatus::Active,
            last_updated: start_ts.clone(),
        };

        queue.push_log(
            CPO_PARTY_ID.to_string(),
            start_ts,
            "Session",
            "Session PUT",
            serialize_ocpi(&session),
        );

        let state = queue.get_or_create(entity);
        state.session_id = Some(session_num);
        state.session_start_game_time = start_game_time;
        state.session_kwh = 0.0;
        state.last_update_game_time = start_game_time;
        state.active_driver = driver_entity;
        state.active_id_tag = Some(evcc_id);
    }
}

// ─────────────────────────────────────────────────────
//  4. Session PATCH (update) system
// ─────────────────────────────────────────────────────

pub fn ocpi_session_update_system(
    chargers: Query<(Entity, &Charger, Option<&BelongsToSite>)>,
    drivers: Query<&Driver>,
    multi_site: Res<MultiSiteManager>,
    mut queue: ResMut<OcpiMessageQueue>,
    game_clock: Res<GameClock>,
) {
    if !queue.is_active() {
        return;
    }

    // Collect updates to avoid borrow conflicts
    let updates: Vec<(Entity, f32, f32, f32, String)> = chargers
        .iter()
        .filter_map(|(entity, charger, site_tag)| {
            if !charger.is_charging {
                return None;
            }
            let cs = queue.charger_state.get(&entity)?;
            cs.session_id?;

            let elapsed = game_clock.total_game_time - cs.last_update_game_time;
            if elapsed < SESSION_UPDATE_INTERVAL_GAME_SECS {
                return None;
            }

            let kwh = cs
                .active_driver
                .and_then(|de| drivers.get(de).ok())
                .map(|d| d.charge_received_kwh)
                .unwrap_or(0.0);

            let site_id = site_tag.map(|s| s.site_id).unwrap_or(SiteId(0));
            let price = multi_site
                .get_site(site_id)
                .map(|s| s.service_strategy.energy_price_kwh)
                .unwrap_or(0.0);

            let start_gt = cs.session_start_game_time;
            let start_ts = queue.game_time_to_utc(start_gt).to_rfc3339();

            Some((entity, kwh, price, charger.allocated_power_kw, start_ts))
        })
        .collect();

    for (entity, kwh, price, power_kw, start_ts) in updates {
        let ts_iso = queue
            .game_time_to_utc(game_clock.total_game_time)
            .to_rfc3339();

        let state = queue.get_or_create(entity);
        state.session_kwh = kwh;
        state.last_update_game_time = game_clock.total_game_time;
        let session_hours = (game_clock.total_game_time - state.session_start_game_time) / 3600.0;

        let patch = SessionPatch {
            kwh: kwh as f64,
            charging_periods: vec![ChargingPeriod {
                start_date_time: start_ts,
                dimensions: vec![
                    CdrDimension {
                        dimension_type: CdrDimensionType::Energy,
                        volume: kwh as f64,
                    },
                    CdrDimension {
                        dimension_type: CdrDimensionType::Time,
                        volume: session_hours as f64,
                    },
                    CdrDimension {
                        dimension_type: CdrDimensionType::Power,
                        volume: power_kw as f64,
                    },
                ],
                tariff_id: None,
            }],
            total_cost: Some(Price {
                before_taxes: (kwh * price) as f64,
                taxes: None,
            }),
            last_updated: ts_iso.clone(),
        };

        queue.push_log(
            CPO_PARTY_ID.to_string(),
            ts_iso,
            "Session",
            "Session PATCH",
            serialize_ocpi(&patch),
        );
    }
}

// ─────────────────────────────────────────────────────
//  5. CDR POST (stop) system
// ─────────────────────────────────────────────────────

pub fn ocpi_cdr_system(
    chargers: Query<(Entity, &Charger, Option<&BelongsToSite>)>,
    multi_site: Res<MultiSiteManager>,
    mut queue: ResMut<OcpiMessageQueue>,
    game_clock: Res<GameClock>,
) {
    if !queue.is_active() {
        return;
    }

    // Collect CDR candidates
    let to_stop: Vec<(
        Entity,
        String,
        String,
        i32,
        f32,
        f32,
        String,
        String,
        ChargerType,
    )> = chargers
        .iter()
        .filter_map(|(entity, charger, site_tag)| {
            if charger.is_charging {
                return None;
            }
            let cs = queue.charger_state.get(&entity)?;
            let session_num = cs.session_id?;
            let evcc = cs.active_id_tag.clone().unwrap_or_default();

            let site_id = site_tag.map(|s| s.site_id).unwrap_or(SiteId(0));
            let site_name = multi_site
                .get_site(site_id)
                .map(|s| s.name.clone())
                .unwrap_or_else(|| "Unknown Site".to_string());
            let price = multi_site
                .get_site(site_id)
                .map(|s| s.service_strategy.energy_price_kwh)
                .unwrap_or(0.0);
            let loc_id = make_location_id(site_id);

            Some((
                entity,
                site_name,
                loc_id,
                session_num,
                cs.session_kwh,
                price,
                evcc,
                charger.id.clone(),
                charger.charger_type,
            ))
        })
        .collect();

    for (entity, site_name, loc_id, session_num, kwh, price, evcc, charger_id, charger_type) in
        to_stop
    {
        let session_start_gt = queue
            .charger_state
            .get(&entity)
            .map(|s| s.session_start_game_time)
            .unwrap_or(0.0);

        let end_ts = queue
            .game_time_to_utc(game_clock.total_game_time)
            .to_rfc3339();
        let start_ts = queue.game_time_to_utc(session_start_gt).to_rfc3339();
        let session_hours = (game_clock.total_game_time - session_start_gt) / 3600.0;

        // Clear session state
        let state = queue.get_or_create(entity);
        state.session_id = None;
        state.active_driver = None;
        state.active_id_tag = None;
        state.session_kwh = 0.0;

        let total_cost_val = (kwh * price) as f64;

        let cdr = Cdr {
            country_code: CPO_COUNTRY_CODE.to_string(),
            party_id: CPO_PARTY_ID.to_string(),
            id: format!("CDR-{session_num:05}"),
            start_date_time: start_ts.clone(),
            end_date_time: end_ts.clone(),
            session_id: Some(format!("SES-{session_num:05}")),
            cdr_token: make_cdr_token(&evcc),
            auth_method: AuthMethod::Whitelist,
            authorization_reference: Some(evcc),
            cdr_location: CdrLocation {
                id: loc_id,
                name: Some(site_name),
                address: SIM_ADDRESS.to_string(),
                city: SIM_CITY.to_string(),
                postal_code: Some(SIM_POSTAL.to_string()),
                country: SIM_COUNTRY_ISO3.to_string(),
                coordinates: GeoLocation {
                    latitude: SIM_LAT.to_string(),
                    longitude: SIM_LON.to_string(),
                },
                evse_uid: charger_id,
                evse_id: make_evse_id(&state.charger_id),
                connector_id: "1".to_string(),
                connector_standard: connector_type_for(charger_type),
                connector_format: ConnectorFormat::Cable,
                connector_power_type: power_type_for(charger_type),
            },
            currency: CURRENCY.to_string(),
            charging_periods: vec![ChargingPeriod {
                start_date_time: start_ts,
                dimensions: vec![
                    CdrDimension {
                        dimension_type: CdrDimensionType::Energy,
                        volume: kwh as f64,
                    },
                    CdrDimension {
                        dimension_type: CdrDimensionType::Time,
                        volume: session_hours as f64,
                    },
                ],
                tariff_id: None,
            }],
            total_cost: Price {
                before_taxes: total_cost_val,
                taxes: None,
            },
            total_energy: kwh as f64,
            total_energy_cost: Some(Price {
                before_taxes: total_cost_val,
                taxes: None,
            }),
            total_time: session_hours as f64,
            total_time_cost: Some(Price {
                before_taxes: 0.0,
                taxes: None,
            }),
            last_updated: end_ts.clone(),
        };

        queue.push_log(
            CPO_PARTY_ID.to_string(),
            end_ts,
            "CDR",
            "CDR POST",
            serialize_ocpi(&cdr),
        );
    }
}
