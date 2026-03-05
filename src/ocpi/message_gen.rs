//! OCPI 2.3.0 message generation systems.
//!
//! These Bevy systems observe charger/driver state and produce OCPI-shaped
//! messages (Location, Session, CDR, EVSE status) pushed into the
//! [`OcpiMessageQueue`].

use bevy::prelude::*;

use crate::components::charger::{Charger, ChargerState, ChargerType};
use crate::components::driver::Driver;
use crate::components::hacker::HackerAttackType;
use crate::components::site::BelongsToSite;
use crate::events::{HackerAttackEvent, HackerDetectedEvent};
use crate::resources::multi_site::{SiteArchetype, SiteId};
use crate::resources::{GameClock, MultiSiteManager};

use super::queue::{LastEmittedTariff, OcpiMessageQueue, SESSION_UPDATE_INTERVAL_GAME_SECS};
use super::types::*;

// ─────────────────────────────────────────────────────
//  Helpers
// ─────────────────────────────────────────────────────

/// Maps charger state to OCPI EVSE status. Faulted and offline stations are
/// advertised as unavailable (OutOfOrder) so eMSPs do not offer them to drivers.
fn charger_state_to_evse_status(state: ChargerState) -> EvseStatus {
    match state {
        ChargerState::Available => EvseStatus::Available,
        ChargerState::Charging => EvseStatus::Charging,
        ChargerState::Warning | ChargerState::Offline => EvseStatus::OutOfOrder,
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

fn make_evse_id(country_code: &str, charger_id: &str) -> String {
    let hash: u32 = charger_id
        .bytes()
        .fold(0u32, |acc, b| acc.wrapping_mul(31).wrapping_add(b as u32));
    format!("{country_code}*KWT*E{:03}", hash % 1000)
}

fn site_archetype(multi_site: &MultiSiteManager, site_id: SiteId) -> SiteArchetype {
    multi_site
        .get_site(site_id)
        .map(|s| s.archetype)
        .unwrap_or(SiteArchetype::ParkingLot)
}

fn make_location_id(site_id: SiteId) -> String {
    format!("LOC-site-{}", site_id.0)
}

fn make_tariff_id(site_id: SiteId, mode: crate::resources::PricingMode) -> String {
    use crate::resources::PricingMode;
    let prefix = match mode {
        PricingMode::Flat => "KWT-FLAT",
        PricingMode::TouLinked => "KWT-TOU",
        PricingMode::CostPlus => "KWT-COSTPLUS",
        PricingMode::DemandResponsive => "KWT-SURGE",
    };
    format!("{prefix}-{}", site_id.0)
}

fn make_cdr_token(evcc_id: &str, is_roaming: bool, is_fleet: bool) -> CdrToken {
    if is_roaming {
        CdrToken {
            country_code: EMSP_COUNTRY_CODE.to_string(),
            party_id: EMSP_PARTY_ID.to_string(),
            uid: evcc_id.to_string(),
            token_type: TokenType::AppUser,
            contract_id: format!("US-EVC-{evcc_id}"),
        }
    } else if is_fleet {
        CdrToken {
            country_code: CPO_COUNTRY_CODE.to_string(),
            party_id: "FLT".to_string(),
            uid: evcc_id.to_string(),
            token_type: TokenType::AppUser,
            contract_id: format!("US-FLT-{evcc_id}"),
        }
    } else {
        CdrToken {
            country_code: CPO_COUNTRY_CODE.to_string(),
            party_id: CPO_PARTY_ID.to_string(),
            uid: evcc_id.to_string(),
            token_type: TokenType::AppUser,
            contract_id: format!("US-KWT-{evcc_id}"),
        }
    }
}

fn make_fleet_tariff_id(contract_id: &str) -> String {
    format!("KWT-FLEET-{contract_id}")
}

fn make_roaming_token(evcc_id: &str, ts_iso: &str) -> Token {
    Token {
        country_code: EMSP_COUNTRY_CODE.to_string(),
        party_id: EMSP_PARTY_ID.to_string(),
        uid: evcc_id.to_string(),
        token_type: TokenType::AppUser,
        contract_id: format!("US-EVC-{evcc_id}"),
        issuer: EMSP_ISSUER.to_string(),
        valid: true,
        whitelist: WhitelistType::Never,
        last_updated: ts_iso.to_string(),
    }
}

fn make_authorization_reference(session_num: i32) -> String {
    format!("AUTH-EVC-{session_num:05}")
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
        let geo = site_archetype(&multi_site, site_id).geo();
        let location_id = make_location_id(site_id);

        let max_v = max_voltage_for(charger.charger_type);
        let max_w = (charger.rated_power_kw * 1000.0) as i32;
        let max_a = if max_v > 0 { max_w / max_v } else { 0 };

        let loc = Location {
            country_code: CPO_COUNTRY_CODE.to_string(),
            party_id: CPO_PARTY_ID.to_string(),
            id: location_id.clone(),
            name: site_name.to_string(),
            address: geo.address.to_string(),
            city: geo.city.to_string(),
            postal_code: geo.postal_code.to_string(),
            country: geo.country.to_string(),
            coordinates: GeoLocation {
                latitude: geo.latitude.to_string(),
                longitude: geo.longitude.to_string(),
            },
            evses: vec![Evse {
                uid: charger.id.clone(),
                evse_id: make_evse_id(geo.country_code, &charger.id),
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
    multi_site: Res<MultiSiteManager>,
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
        let geo = site_archetype(&multi_site, site_id).geo();

        let update = EvseStatusUpdate {
            location_id: make_location_id(site_id),
            evse_uid: charger.id.clone(),
            evse_id: make_evse_id(geo.country_code, &charger.id),
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
    drivers: Query<(
        Entity,
        &Driver,
        Option<&crate::resources::fleet::FleetVehicle>,
    )>,
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
            .find(|(_, d, _)| d.assigned_charger == Some(entity))
            .map(|(de, d, fv)| {
                let fleet_cid = fv.map(|f| f.contract_id.clone());
                (de, d.evcc_id.clone(), d.is_roaming, fv.is_some(), fleet_cid)
            });

        let (driver_entity, evcc_id, is_roaming, is_fleet, fleet_contract_id) = match driver_info {
            Some((de, id, roaming, fleet, fcid)) => (Some(de), id, roaming, fleet, fcid),
            None => (None, "000000000000".to_string(), false, false, None),
        };

        let session_num = queue.next_session_id();
        let start_game_time = charger
            .session_start_game_time
            .unwrap_or(game_clock.total_game_time);
        let start_ts = queue.game_time_to_utc(start_game_time).to_rfc3339();
        let site_id = site_tag.map(|s| s.site_id).unwrap_or(SiteId(0));
        let location_id = make_location_id(site_id);

        let (auth_method, auth_ref) = if is_roaming {
            (
                AuthMethod::Command,
                make_authorization_reference(session_num),
            )
        } else {
            (AuthMethod::Whitelist, evcc_id.clone())
        };

        // For roaming sessions, emit the eMSP → CPO command chain first
        if is_roaming {
            let cmd_ts = queue.game_time_to_utc(start_game_time - 3.0).to_rfc3339();

            let start_cmd = StartSessionCommand {
                response_url: format!(
                    "https://emsp.evconnect.com/ocpi/2.3.0/commands/START_SESSION/{}",
                    session_num
                ),
                token: make_roaming_token(&evcc_id, &cmd_ts),
                location_id: location_id.clone(),
                evse_uid: Some(charger.id.clone()),
                connector_id: Some("1".to_string()),
                authorization_reference: Some(auth_ref.clone()),
            };
            queue.push_log(
                EMSP_PARTY_ID.to_string(),
                cmd_ts.clone(),
                "Command",
                "START_SESSION POST",
                serialize_ocpi(&start_cmd),
            );

            let cmd_response = CommandResponse {
                result: CommandResponseType::Accepted,
                timeout: 30,
                message: None,
            };
            queue.push_log(
                CPO_PARTY_ID.to_string(),
                cmd_ts.clone(),
                "Command",
                "CommandResponse",
                serialize_ocpi(&cmd_response),
            );

            let cmd_result = CommandResult {
                result: CommandResultType::Accepted,
                message: None,
            };
            queue.push_log(
                CPO_PARTY_ID.to_string(),
                cmd_ts,
                "Command",
                "CommandResult POST",
                serialize_ocpi(&cmd_result),
            );
        }

        let session = Session {
            country_code: CPO_COUNTRY_CODE.to_string(),
            party_id: CPO_PARTY_ID.to_string(),
            id: format!("SES-{session_num:05}"),
            start_date_time: start_ts.clone(),
            end_date_time: None,
            kwh: 0.0,
            cdr_token: make_cdr_token(&evcc_id, is_roaming, is_fleet),
            auth_method,
            authorization_reference: Some(auth_ref),
            location_id,
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
        state.is_roaming = is_roaming;
        state.is_fleet = is_fleet;
        state.fleet_contract_id = fleet_contract_id;
    }
}

// ─────────────────────────────────────────────────────
//  4. Session PATCH (update) system
// ─────────────────────────────────────────────────────

pub fn ocpi_session_update_system(
    chargers: Query<(Entity, &Charger, Option<&BelongsToSite>)>,
    drivers: Query<&Driver>,
    multi_site: Res<MultiSiteManager>,
    fleet_mgr: Res<crate::resources::FleetContractManager>,
    mut queue: ResMut<OcpiMessageQueue>,
    game_clock: Res<GameClock>,
) {
    if !queue.is_active() {
        return;
    }

    // Collect updates to avoid borrow conflicts
    let updates: Vec<(Entity, f32, f32, f32, String, String)> = chargers
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

            // Fleet sessions use their contracted price and fleet tariff
            let (price, tariff) = if cs.is_fleet {
                let fleet_price = cs
                    .fleet_contract_id
                    .as_ref()
                    .and_then(|cid| {
                        fleet_mgr
                            .active
                            .iter()
                            .find(|c| &c.def.id == cid)
                            .map(|c| c.def.contracted_price_per_kwh)
                    })
                    .unwrap_or(0.0);
                let fleet_tariff = cs
                    .fleet_contract_id
                    .as_ref()
                    .map(|cid| make_fleet_tariff_id(cid))
                    .unwrap_or_default();
                (fleet_price, fleet_tariff)
            } else {
                multi_site
                    .get_site(site_id)
                    .map(|s| {
                        let p = s.service_strategy.pricing.effective_price(
                            game_clock.game_time,
                            &s.site_energy_config,
                            s.charger_utilization,
                        );
                        let t = make_tariff_id(site_id, s.service_strategy.pricing.mode);
                        (p, t)
                    })
                    .unwrap_or((0.0, String::new()))
            };

            let start_gt = cs.session_start_game_time;
            let start_ts = queue.game_time_to_utc(start_gt).to_rfc3339();

            Some((
                entity,
                kwh,
                price,
                charger.allocated_power_kw,
                start_ts,
                tariff,
            ))
        })
        .collect();

    for (entity, kwh, price, power_kw, start_ts, tariff) in updates {
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
                tariff_id: if tariff.is_empty() {
                    None
                } else {
                    Some(tariff)
                },
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
    fleet_mgr: Res<crate::resources::FleetContractManager>,
    mut queue: ResMut<OcpiMessageQueue>,
    game_clock: Res<GameClock>,
) {
    if !queue.is_active() {
        return;
    }

    struct CdrCandidate {
        entity: Entity,
        site_name: String,
        loc_id: String,
        archetype: SiteArchetype,
        session_num: i32,
        kwh: f32,
        price: f32,
        evcc: String,
        charger_id: String,
        charger_type: ChargerType,
        tariff: String,
        is_roaming: bool,
        is_fleet: bool,
    }

    let to_stop: Vec<CdrCandidate> = chargers
        .iter()
        .filter_map(|(entity, charger, site_tag)| {
            if charger.is_charging {
                return None;
            }
            let cs = queue.charger_state.get(&entity)?;
            let session_num = cs.session_id?;
            let evcc = cs.active_id_tag.clone().unwrap_or_default();
            let is_roaming = cs.is_roaming;
            let is_fleet = cs.is_fleet;

            let site_id = site_tag.map(|s| s.site_id).unwrap_or(SiteId(0));

            let (site_name, price, tariff) = if is_fleet {
                let fleet_price = cs
                    .fleet_contract_id
                    .as_ref()
                    .and_then(|cid| {
                        fleet_mgr
                            .active
                            .iter()
                            .find(|c| &c.def.id == cid)
                            .map(|c| c.def.contracted_price_per_kwh)
                    })
                    .unwrap_or(0.0);
                let fleet_tariff = cs
                    .fleet_contract_id
                    .as_ref()
                    .map(|cid| make_fleet_tariff_id(cid))
                    .unwrap_or_default();
                let name = multi_site
                    .get_site(site_id)
                    .map(|s| s.name.clone())
                    .unwrap_or_else(|| "Unknown Site".to_string());
                (name, fleet_price, fleet_tariff)
            } else {
                multi_site
                    .get_site(site_id)
                    .map(|s| {
                        let p = s.service_strategy.pricing.effective_price(
                            game_clock.game_time,
                            &s.site_energy_config,
                            s.charger_utilization,
                        );
                        let t = make_tariff_id(site_id, s.service_strategy.pricing.mode);
                        (s.name.clone(), p, t)
                    })
                    .unwrap_or_else(|| ("Unknown Site".to_string(), 0.0, String::new()))
            };

            let loc_id = make_location_id(site_id);
            let archetype = site_archetype(&multi_site, site_id);

            Some(CdrCandidate {
                entity,
                site_name,
                loc_id,
                archetype,
                session_num,
                kwh: cs.session_kwh,
                price,
                evcc,
                charger_id: charger.id.clone(),
                charger_type: charger.charger_type,
                tariff,
                is_roaming,
                is_fleet,
            })
        })
        .collect();

    for c in to_stop {
        let session_start_gt = queue
            .charger_state
            .get(&c.entity)
            .map(|s| s.session_start_game_time)
            .unwrap_or(0.0);

        let end_ts = queue
            .game_time_to_utc(game_clock.total_game_time)
            .to_rfc3339();
        let start_ts = queue.game_time_to_utc(session_start_gt).to_rfc3339();
        let session_hours = (game_clock.total_game_time - session_start_gt) / 3600.0;

        let (auth_method, auth_ref) = if c.is_roaming {
            (
                AuthMethod::Command,
                make_authorization_reference(c.session_num),
            )
        } else {
            (AuthMethod::Whitelist, c.evcc.clone())
        };

        // Clear session state
        let state = queue.get_or_create(c.entity);
        state.session_id = None;
        state.active_driver = None;
        state.active_id_tag = None;
        state.session_kwh = 0.0;
        state.is_roaming = false;
        state.is_fleet = false;
        state.fleet_contract_id = None;

        let total_cost_val = (c.kwh * c.price) as f64;
        let geo = c.archetype.geo();

        let cdr = Cdr {
            country_code: CPO_COUNTRY_CODE.to_string(),
            party_id: CPO_PARTY_ID.to_string(),
            id: format!("CDR-{:05}", c.session_num),
            start_date_time: start_ts.clone(),
            end_date_time: end_ts.clone(),
            session_id: Some(format!("SES-{:05}", c.session_num)),
            cdr_token: make_cdr_token(&c.evcc, c.is_roaming, c.is_fleet),
            auth_method,
            authorization_reference: Some(auth_ref),
            cdr_location: CdrLocation {
                id: c.loc_id,
                name: Some(c.site_name),
                address: geo.address.to_string(),
                city: geo.city.to_string(),
                postal_code: Some(geo.postal_code.to_string()),
                country: geo.country.to_string(),
                coordinates: GeoLocation {
                    latitude: geo.latitude.to_string(),
                    longitude: geo.longitude.to_string(),
                },
                evse_uid: c.charger_id,
                evse_id: make_evse_id(geo.country_code, &state.charger_id),
                connector_id: "1".to_string(),
                connector_standard: connector_type_for(c.charger_type),
                connector_format: ConnectorFormat::Cable,
                connector_power_type: power_type_for(c.charger_type),
            },
            currency: CURRENCY.to_string(),
            charging_periods: vec![ChargingPeriod {
                start_date_time: start_ts,
                dimensions: vec![
                    CdrDimension {
                        dimension_type: CdrDimensionType::Energy,
                        volume: c.kwh as f64,
                    },
                    CdrDimension {
                        dimension_type: CdrDimensionType::Time,
                        volume: session_hours as f64,
                    },
                ],
                tariff_id: if c.tariff.is_empty() {
                    None
                } else {
                    Some(c.tariff)
                },
            }],
            total_cost: Price {
                before_taxes: total_cost_val,
                taxes: None,
            },
            total_energy: c.kwh as f64,
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

// ─────────────────────────────────────────────────────
//  6. Tariff PUT system
// ─────────────────────────────────────────────────────

pub fn ocpi_tariff_system(
    multi_site: Res<MultiSiteManager>,
    fleet_mgr: Res<crate::resources::FleetContractManager>,
    mut queue: ResMut<OcpiMessageQueue>,
    game_clock: Res<GameClock>,
) {
    if !queue.is_active() {
        return;
    }

    for (site_id, site) in multi_site.owned_sites.iter() {
        let strat = &site.service_strategy;
        let tariff_id = make_tariff_id(*site_id, strat.pricing.mode);

        use crate::resources::PricingMode;
        let mut elements = match strat.pricing.mode {
            PricingMode::Flat => vec![TariffElement {
                price_components: vec![PriceComponent {
                    component_type: TariffDimensionType::Energy,
                    price: strat.pricing.flat.price_kwh as f64,
                    step_size: 1,
                }],
                restrictions: None,
            }],
            PricingMode::TouLinked => vec![
                TariffElement {
                    price_components: vec![PriceComponent {
                        component_type: TariffDimensionType::Energy,
                        price: strat.pricing.tou.off_peak_price as f64,
                        step_size: 1,
                    }],
                    restrictions: Some(TariffRestrictions {
                        start_time: Some("21:00".to_string()),
                        end_time: Some("09:00".to_string()),
                    }),
                },
                TariffElement {
                    price_components: vec![PriceComponent {
                        component_type: TariffDimensionType::Energy,
                        price: strat.pricing.tou.on_peak_price as f64,
                        step_size: 1,
                    }],
                    restrictions: Some(TariffRestrictions {
                        start_time: Some("09:00".to_string()),
                        end_time: Some("21:00".to_string()),
                    }),
                },
            ],
            PricingMode::CostPlus => {
                let effective = strat.pricing.effective_price(
                    game_clock.game_time,
                    &site.site_energy_config,
                    site.charger_utilization,
                );
                vec![TariffElement {
                    price_components: vec![PriceComponent {
                        component_type: TariffDimensionType::Energy,
                        price: effective as f64,
                        step_size: 1,
                    }],
                    restrictions: None,
                }]
            }
            PricingMode::DemandResponsive => {
                let effective = strat.pricing.effective_price(
                    game_clock.game_time,
                    &site.site_energy_config,
                    site.charger_utilization,
                );
                vec![TariffElement {
                    price_components: vec![PriceComponent {
                        component_type: TariffDimensionType::Energy,
                        price: effective as f64,
                        step_size: 1,
                    }],
                    restrictions: None,
                }]
            }
        };

        // Append a MAX_POWER element when the site carries a non-default
        // demand/capacity charge (e.g. ScooterHub's punitive $25/kW rate).
        let demand_rate = site.site_energy_config.demand_rate_per_kw;
        if demand_rate > 15.0 + f32::EPSILON {
            let window_min = (site.site_energy_config.demand_window_seconds / 60.0) as i32;
            elements.push(TariffElement {
                price_components: vec![PriceComponent {
                    component_type: TariffDimensionType::MaxPower,
                    price: demand_rate as f64,
                    step_size: window_min,
                }],
                restrictions: None,
            });
        }

        let price_cents: Vec<i32> = elements
            .iter()
            .flat_map(|el| el.price_components.iter())
            .map(|pc| (pc.price * 100.0).round() as i32)
            .collect();

        let fingerprint = LastEmittedTariff {
            tariff_id: tariff_id.clone(),
            price_cents,
        };
        if queue.last_emitted_tariff.get(site_id) == Some(&fingerprint) {
            continue;
        }
        queue.last_emitted_tariff.insert(*site_id, fingerprint);

        let ts_iso = queue
            .game_time_to_utc(game_clock.total_game_time)
            .to_rfc3339();

        let mut description = match strat.pricing.mode {
            PricingMode::Flat => format!("Flat ${:.2}/kWh", strat.pricing.flat.price_kwh),
            PricingMode::TouLinked => format!(
                "Off-Peak ${:.2} / On-Peak ${:.2} per kWh",
                strat.pricing.tou.off_peak_price, strat.pricing.tou.on_peak_price
            ),
            PricingMode::CostPlus => format!(
                "Cost+{:.0}% (${:.2}-${:.2})",
                strat.pricing.cost_plus.markup_pct,
                strat.pricing.cost_plus.floor,
                strat.pricing.cost_plus.ceiling
            ),
            PricingMode::DemandResponsive => format!(
                "Base ${:.2}, {:.1}x surge @{:.0}%",
                strat.pricing.surge.base_price,
                strat.pricing.surge.multiplier,
                strat.pricing.surge.threshold * 100.0
            ),
        };

        if demand_rate > 15.0 + f32::EPSILON {
            let window_min = (site.site_energy_config.demand_window_seconds / 60.0) as u32;
            description.push_str(&format!(
                " | Capacity ${:.0}/kW ({window_min}m peak)",
                demand_rate
            ));
        }

        let tariff = Tariff {
            country_code: CPO_COUNTRY_CODE.to_string(),
            party_id: CPO_PARTY_ID.to_string(),
            id: tariff_id,
            currency: CURRENCY.to_string(),
            elements,
            tariff_alt_text: Some(vec![DisplayText {
                language: "en".to_string(),
                text: description,
            }]),
            last_updated: ts_iso.clone(),
        };

        queue.push_log(
            CPO_PARTY_ID.to_string(),
            ts_iso,
            "Tariff",
            "Tariff PUT",
            serialize_ocpi(&tariff),
        );
    }

    // Emit fleet-specific tariffs for active fleet contracts
    for contract in &fleet_mgr.active {
        if contract.terminated {
            continue;
        }

        let fleet_tariff_id = make_fleet_tariff_id(&contract.def.id);
        let price_cents =
            vec![(contract.def.contracted_price_per_kwh as f64 * 100.0).round() as i32];
        let fingerprint = LastEmittedTariff {
            tariff_id: fleet_tariff_id.clone(),
            price_cents,
        };

        let dummy_site_id = SiteId(9000 + contract.day_accepted);
        if queue.last_emitted_tariff.get(&dummy_site_id) == Some(&fingerprint) {
            continue;
        }
        queue.last_emitted_tariff.insert(dummy_site_id, fingerprint);

        let ts_iso = queue
            .game_time_to_utc(game_clock.total_game_time)
            .to_rfc3339();

        let fleet_tariff = Tariff {
            country_code: CPO_COUNTRY_CODE.to_string(),
            party_id: CPO_PARTY_ID.to_string(),
            id: fleet_tariff_id,
            currency: CURRENCY.to_string(),
            elements: vec![TariffElement {
                price_components: vec![PriceComponent {
                    component_type: TariffDimensionType::Energy,
                    price: contract.def.contracted_price_per_kwh as f64,
                    step_size: 1,
                }],
                restrictions: None,
            }],
            tariff_alt_text: Some(vec![DisplayText {
                language: "en".to_string(),
                text: format!(
                    "Fleet contract: {} @ ${:.2}/kWh",
                    contract.def.company_name, contract.def.contracted_price_per_kwh
                ),
            }]),
            last_updated: ts_iso.clone(),
        };

        queue.push_log(
            CPO_PARTY_ID.to_string(),
            ts_iso,
            "Tariff",
            "Tariff PUT (Fleet)",
            serialize_ocpi(&fleet_tariff),
        );
    }
}

// ─────────────────────────────────────────────────────
//  7. Hacker security event system
// ─────────────────────────────────────────────────────

/// Emits explicit OCPI incident log entries for hacker attacks and mitigations.
pub fn ocpi_hacker_event_system(
    mut queue: ResMut<OcpiMessageQueue>,
    game_clock: Res<GameClock>,
    mut attack_events: MessageReader<HackerAttackEvent>,
    mut detected_events: MessageReader<HackerDetectedEvent>,
) {
    let timestamp = queue.game_time_to_utc(game_clock.total_game_time);
    let ts_iso = timestamp.to_rfc3339();

    for event in attack_events.read() {
        let (action, attack_name) = match event.attack_type {
            HackerAttackType::TransformerOverload => ("CyberAttack", "TransformerOverload"),
            HackerAttackType::PriceSlash => ("CyberAttack", "PriceManipulation"),
        };

        let payload = serde_json::json!({
            "object_type": "Incident",
            "action": action,
            "attack_type": attack_name,
            "site_id": format!("{:?}", event.site_id),
            "timestamp": ts_iso,
        });

        queue.push_log(
            CPO_PARTY_ID.to_string(),
            ts_iso.clone(),
            "Incident",
            action,
            serde_json::to_string_pretty(&payload).unwrap_or_default(),
        );
    }

    for event in detected_events.read() {
        if !event.auto_blocked {
            continue;
        }

        let attack_name = match event.attack_type {
            HackerAttackType::TransformerOverload => "TransformerOverload",
            HackerAttackType::PriceSlash => "PriceManipulation",
        };

        let payload = serde_json::json!({
            "object_type": "Incident",
            "action": "CyberAttackMitigated",
            "attack_type": attack_name,
            "mitigation": "AgenticSOC",
            "site_id": format!("{:?}", event.site_id),
            "timestamp": ts_iso,
        });

        queue.push_log(
            CPO_PARTY_ID.to_string(),
            ts_iso.clone(),
            "Incident",
            "CyberAttackMitigated",
            serde_json::to_string_pretty(&payload).unwrap_or_default(),
        );
    }
}
