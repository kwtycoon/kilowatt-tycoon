//! OpenADR 3.0 message generation systems.
//!
//! These Bevy systems observe per-site DER state (solar, BESS, grid) and
//! produce protocol-compliant OpenADR 3.0 messages using the `openleadr-wire`
//! crate, pushed into the [`OpenAdrMessageQueue`].

use bevy::prelude::*;

use crate::resources::{
    GameClock, MultiSiteManager, PricingMode, SolarExportPolicy, site_energy::CarbonCreditMarket,
};

use super::queue::{OpenAdrMessageQueue, TELEMETRY_INTERVAL_GAME_SECS};
use super::types::*;

/// The fixed program ID for the simulated demand-response program.
const DR_PROGRAM_ID: &str = "kw-tycoon-dr";

// ─────────────────────────────────────────────────────
//  Targeting helpers
// ─────────────────────────────────────────────────────

fn ven_target(ven_name: &str) -> Option<TargetMap> {
    Some(TargetMap(vec![TargetEntry {
        label: TargetType::VENName,
        values: vec![ven_name.to_string()],
    }]))
}

fn resource_target(ven_name: &str, resource_name: &str) -> Option<TargetMap> {
    Some(TargetMap(vec![
        TargetEntry {
            label: TargetType::VENName,
            values: vec![ven_name.to_string()],
        },
        TargetEntry {
            label: TargetType::ResourceName,
            values: vec![resource_name.to_string()],
        },
    ]))
}

fn site_group_target(site_id: u32) -> Option<TargetMap> {
    Some(TargetMap(vec![TargetEntry {
        label: TargetType::Group,
        values: vec![format!("site-{site_id}")],
    }]))
}

// ─────────────────────────────────────────────────────
//  OperatingState mapping helpers
// ─────────────────────────────────────────────────────

fn bess_operating_state(
    current_power_kw: f32,
    soc_percent: f32,
    demand_event_active: bool,
) -> OperatingState {
    if current_power_kw > 0.0 && demand_event_active {
        OperatingState::RunningCurtailed
    } else if current_power_kw.abs() > f32::EPSILON {
        OperatingState::RunningNormal
    } else if soc_percent < 10.0 {
        OperatingState::IdleCurtailed
    } else {
        OperatingState::IdleNormal
    }
}

fn grid_operating_state(demand_event_active: bool, fire_active: bool) -> OperatingState {
    if fire_active {
        OperatingState::Private("EMERGENCY_SHUTDOWN".to_string())
    } else if demand_event_active {
        OperatingState::RunningHeightened
    } else {
        OperatingState::Normal
    }
}

/// Map a grid event name to the appropriate OpenADR 3.0 alert type.
fn grid_event_to_alert_type(event_name: &str) -> EventType {
    match event_name {
        "Record Demand" | "Generator Trip" | "Unexpected Plant Outage" => {
            EventType::AlertGridEmergency
        }
        "Transmission Constraint" | "Grid Congestion" => EventType::AlertPossibleOutage,
        "Heat Emergency" => EventType::AlertOther,
        "Renewable Shortfall" => EventType::AlertFlexAlert,
        _ => EventType::AlertOther,
    }
}

// ─────────────────────────────────────────────────────
//  0. Program registration system
// ─────────────────────────────────────────────────────

/// Emit a `ProgramContent` message once when the feed first activates.
pub fn openadr_program_system(mut queue: ResMut<OpenAdrMessageQueue>) {
    if !queue.is_active() || queue.program_registered {
        return;
    }
    queue.program_registered = true;

    let sim_start = queue.sim_start;
    let ts_iso = sim_start.to_rfc3339();

    let program = ProgramContent {
        program_name: DR_PROGRAM_ID.to_string(),
        program_long_name: Some("Kilowatt Tycoon DER Management Program".to_string()),
        retailer_name: Some("KW Tycoon Grid Operator".to_string()),
        retailer_long_name: None,
        program_type: Some("PRICING_TARIFF".to_string()),
        country: Some("US".to_string()),
        principal_subdivision: None,
        time_zone_offset: None,
        interval_period: Some(IntervalPeriod {
            start: sim_start,
            duration: None,
            randomize_start: None,
        }),
        program_descriptions: Some(vec![ProgramDescription {
            url: "https://kilowatt-tycoon.com/dr-program".to_string(),
        }]),
        binding_events: Some(true),
        local_price: Some(true),
        payload_descriptors: Some(vec![
            PayloadDescriptor::EventPayloadDescriptor(EventPayloadDescriptor {
                payload_type: EventType::Price,
                units: Some(Unit::Private("$/kWh".to_string())),
                currency: None,
            }),
            PayloadDescriptor::EventPayloadDescriptor(EventPayloadDescriptor {
                payload_type: EventType::GHG,
                units: Some(Unit::Private("gCO2/kWh".to_string())),
                currency: None,
            }),
            PayloadDescriptor::ReportPayloadDescriptor(ReportPayloadDescriptor {
                payload_type: ReportType::Usage,
                reading_type: ReadingType::DirectRead,
                units: Some(Unit::KW),
                accuracy: None,
                confidence: None,
            }),
        ]),
        targets: None,
    };

    let json = serialize_openadr(&program);
    queue.push_log(
        "vtn".to_string(),
        ts_iso,
        "Program",
        "Program Register",
        json,
    );
}

// ─────────────────────────────────────────────────────
//  1. VEN registration system (+ Resource registration)
// ─────────────────────────────────────────────────────

/// Register solar and BESS as VENs when capacity goes from 0 to >0.
/// Also emits `ResourceContent` for each physical DER asset.
pub fn openadr_register_system(
    multi_site: Res<MultiSiteManager>,
    mut queue: ResMut<OpenAdrMessageQueue>,
    game_clock: Res<GameClock>,
) {
    if !queue.is_active() {
        return;
    }

    let sim_start = queue.sim_start;

    for (site_id, site_state) in &multi_site.owned_sites {
        let ts_iso =
            (sim_start + chrono::Duration::seconds(game_clock.total_game_time as i64)).to_rfc3339();

        let der_state = queue.get_or_create(*site_id);

        // Solar VEN + Resource registration
        if !der_state.solar_registered && site_state.solar_state.installed_kw_peak > 0.0 {
            der_state.solar_registered = true;
            der_state.last_solar_kw_peak = site_state.solar_state.installed_kw_peak;

            let ven_name = format!("solar-site-{}", site_id.0);
            let ven_content = VenContent::new(
                ven_name.clone(),
                Some(vec![values_map_f32(
                    "CAPACITY_KW_PEAK",
                    site_state.solar_state.installed_kw_peak,
                )]),
                ven_target(&ven_name),
                None,
            );
            let json = serialize_openadr(&ven_content);
            queue.push_log(
                ven_name.clone(),
                ts_iso.clone(),
                "Ven",
                "Solar VEN Register",
                json,
            );

            let resource = ResourceContent {
                resource_name: format!("solar-array-site-{}", site_id.0),
                attributes: Some(vec![values_map_f32(
                    "CAPACITY_KW_PEAK",
                    site_state.solar_state.installed_kw_peak,
                )]),
                targets: resource_target(&ven_name, "solar-array"),
            };
            let resource_json = serialize_openadr(&resource);
            queue.push_log(
                ven_name,
                ts_iso.clone(),
                "Resource",
                "Resource Register",
                resource_json,
            );
        }

        // BESS VEN + Resource registration
        let der_state = queue.get_or_create(*site_id);
        if !der_state.bess_registered && site_state.bess_state.capacity_kwh > 0.0 {
            der_state.bess_registered = true;
            der_state.last_bess_kwh = site_state.bess_state.capacity_kwh;

            let ven_name = format!("bess-site-{}", site_id.0);
            let ven_content = VenContent::new(
                ven_name.clone(),
                Some(vec![
                    values_map_f32("CAPACITY_KWH", site_state.bess_state.capacity_kwh),
                    values_map_f32("MAX_CHARGE_KW", site_state.bess_state.max_charge_kw),
                    values_map_f32("MAX_DISCHARGE_KW", site_state.bess_state.max_discharge_kw),
                ]),
                ven_target(&ven_name),
                None,
            );
            let json = serialize_openadr(&ven_content);
            queue.push_log(
                ven_name.clone(),
                ts_iso.clone(),
                "Ven",
                "BESS VEN Register",
                json,
            );

            let resource = ResourceContent {
                resource_name: format!("bess-unit-site-{}", site_id.0),
                attributes: Some(vec![
                    values_map_f32("CAPACITY_KWH", site_state.bess_state.capacity_kwh),
                    values_map_f32("MAX_CHARGE_KW", site_state.bess_state.max_charge_kw),
                    values_map_f32("MAX_DISCHARGE_KW", site_state.bess_state.max_discharge_kw),
                ]),
                targets: resource_target(&ven_name, "bess-unit"),
            };
            let resource_json = serialize_openadr(&resource);
            queue.push_log(
                ven_name,
                ts_iso.clone(),
                "Resource",
                "Resource Register",
                resource_json,
            );
        }
    }
}

// ─────────────────────────────────────────────────────
//  2. Solar telemetry system
// ─────────────────────────────────────────────────────

/// Periodic solar generation reports using `ReportContent`.
pub fn openadr_solar_telemetry_system(
    multi_site: Res<MultiSiteManager>,
    mut queue: ResMut<OpenAdrMessageQueue>,
    game_clock: Res<GameClock>,
) {
    if !queue.is_active() {
        return;
    }

    let program_id = match make_program_id(DR_PROGRAM_ID) {
        Some(id) => id,
        None => return,
    };
    let sim_start = queue.sim_start;

    for (site_id, site_state) in &multi_site.owned_sites {
        if site_state.solar_state.installed_kw_peak <= 0.0 {
            continue;
        }

        let der_state = queue.get_or_create(*site_id);
        if !der_state.solar_registered {
            continue;
        }
        if game_clock.total_game_time - der_state.last_telemetry_game_time
            < TELEMETRY_INTERVAL_GAME_SECS
        {
            continue;
        }
        let interval_id = der_state.next_interval();

        let timestamp = sim_start + chrono::Duration::seconds(game_clock.total_game_time as i64);
        let ts_iso = timestamp.to_rfc3339();
        let ven_name = format!("solar-site-{}", site_id.0);

        let export_rate = if site_state.challenge_level >= 2 {
            site_state.spot_market.current_price_per_kwh
        } else {
            site_state
                .site_energy_config
                .current_export_rate(game_clock.game_time)
        };

        let report = ReportContent {
            program_id: program_id.clone(),
            event_id: "solar-telemetry"
                .parse()
                .unwrap_or_else(|_| unreachable!("static event ID should always parse")),
            client_name: ven_name.clone(),
            report_name: Some("Solar Generation".to_string()),
            payload_descriptors: Some(vec![ReportPayloadDescriptor {
                payload_type: ReportType::Usage,
                reading_type: ReadingType::DirectRead,
                units: Some(Unit::KW),
                accuracy: None,
                confidence: None,
            }]),
            resources: vec![ReportResource {
                resource_name: ResourceName::Private("solar-array".to_string()),
                interval_period: Some(IntervalPeriod {
                    start: timestamp,
                    duration: None,
                    randomize_start: None,
                }),
                intervals: vec![openleadr_wire::interval::Interval {
                    id: interval_id,
                    interval_period: None,
                    payloads: vec![
                        values_map_f32(
                            "GENERATION_KW",
                            site_state.solar_state.current_generation_kw,
                        ),
                        values_map_f32(
                            "TOTAL_GENERATED_KWH",
                            site_state.solar_state.total_generated_kwh,
                        ),
                        values_map_f32("EXPORT_KW", site_state.grid_import.export_kw),
                        values_map_f32("EXPORT_RATE", export_rate),
                    ],
                }],
            }],
        };

        let json = serialize_openadr(&report);
        queue.push_log(ven_name, ts_iso, "Report", "Solar Telemetry", json);
    }
}

// ─────────────────────────────────────────────────────
//  3. BESS telemetry system (+ OperatingState)
// ─────────────────────────────────────────────────────

/// Periodic BESS state-of-charge and power reports with `OperatingState`.
pub fn openadr_bess_telemetry_system(
    multi_site: Res<MultiSiteManager>,
    mut queue: ResMut<OpenAdrMessageQueue>,
    game_clock: Res<GameClock>,
) {
    if !queue.is_active() {
        return;
    }

    let program_id = match make_program_id(DR_PROGRAM_ID) {
        Some(id) => id,
        None => return,
    };
    let sim_start = queue.sim_start;

    for (site_id, site_state) in &multi_site.owned_sites {
        if site_state.bess_state.capacity_kwh <= 0.0 {
            continue;
        }

        let der_state = queue.get_or_create(*site_id);
        if !der_state.bess_registered {
            continue;
        }
        if game_clock.total_game_time - der_state.last_telemetry_game_time
            < TELEMETRY_INTERVAL_GAME_SECS
        {
            continue;
        }
        let interval_id = der_state.next_interval();
        let dr_active = der_state.demand_event_active;

        let timestamp = sim_start + chrono::Duration::seconds(game_clock.total_game_time as i64);
        let ts_iso = timestamp.to_rfc3339();
        let ven_name = format!("bess-site-{}", site_id.0);

        let op_state = bess_operating_state(
            site_state.bess_state.current_power_kw,
            site_state.bess_state.soc_percent(),
            dr_active,
        );

        let report = ReportContent {
            program_id: program_id.clone(),
            event_id: "bess-telemetry"
                .parse()
                .unwrap_or_else(|_| unreachable!("static event ID should always parse")),
            client_name: ven_name.clone(),
            report_name: Some("BESS Status".to_string()),
            payload_descriptors: Some(vec![
                ReportPayloadDescriptor {
                    payload_type: ReportType::StorageChargeLevel,
                    reading_type: ReadingType::DirectRead,
                    units: Some(Unit::Percent),
                    accuracy: None,
                    confidence: None,
                },
                ReportPayloadDescriptor {
                    payload_type: ReportType::Demand,
                    reading_type: ReadingType::DirectRead,
                    units: Some(Unit::KW),
                    accuracy: None,
                    confidence: None,
                },
            ]),
            resources: vec![ReportResource {
                resource_name: ResourceName::Private("bess-unit".to_string()),
                interval_period: Some(IntervalPeriod {
                    start: timestamp,
                    duration: None,
                    randomize_start: None,
                }),
                intervals: vec![openleadr_wire::interval::Interval {
                    id: interval_id,
                    interval_period: None,
                    payloads: vec![
                        values_map_f32("SOC_PERCENT", site_state.bess_state.soc_percent()),
                        values_map_f32("POWER_KW", site_state.bess_state.current_power_kw),
                        values_map_str("MODE", site_state.bess_state.mode.display_name()),
                        values_map_str("OPERATING_STATE", &format!("{op_state:?}")),
                    ],
                }],
            }],
        };

        let json = serialize_openadr(&report);
        queue.push_log(ven_name, ts_iso, "Report", "BESS Telemetry", json);
    }
}

// ─────────────────────────────────────────────────────
//  4. Grid telemetry system (+ OperatingState)
// ─────────────────────────────────────────────────────

/// Periodic grid import and demand reports with `OperatingState`.
pub fn openadr_grid_telemetry_system(
    multi_site: Res<MultiSiteManager>,
    transformers: Query<&crate::components::power::Transformer>,
    mut queue: ResMut<OpenAdrMessageQueue>,
    game_clock: Res<GameClock>,
) {
    if !queue.is_active() {
        return;
    }

    let program_id = match make_program_id(DR_PROGRAM_ID) {
        Some(id) => id,
        None => return,
    };
    let sim_start = queue.sim_start;

    for (site_id, site_state) in &multi_site.owned_sites {
        let der_state = queue.get_or_create(*site_id);

        if game_clock.total_game_time - der_state.last_telemetry_game_time
            < TELEMETRY_INTERVAL_GAME_SECS
        {
            continue;
        }

        der_state.last_telemetry_game_time = game_clock.total_game_time;
        let interval_id = der_state.next_interval();
        let dr_active = der_state.demand_event_active;

        let fire_active = transformers
            .iter()
            .any(|t| t.site_id == *site_id && (t.on_fire || t.destroyed));

        let timestamp = sim_start + chrono::Duration::seconds(game_clock.total_game_time as i64);
        let ts_iso = timestamp.to_rfc3339();
        let ven_name = format!("grid-site-{}", site_id.0);

        let op_state = grid_operating_state(dr_active, fire_active);

        let export_rate = if site_state.challenge_level >= 2 {
            site_state.spot_market.current_price_per_kwh
        } else {
            site_state
                .site_energy_config
                .current_export_rate(game_clock.game_time)
        };

        let report = ReportContent {
            program_id: program_id.clone(),
            event_id: "grid-telemetry"
                .parse()
                .unwrap_or_else(|_| unreachable!("static event ID should always parse")),
            client_name: ven_name.clone(),
            report_name: Some("Grid Import".to_string()),
            payload_descriptors: Some(vec![ReportPayloadDescriptor {
                payload_type: ReportType::Demand,
                reading_type: ReadingType::DirectRead,
                units: Some(Unit::KW),
                accuracy: None,
                confidence: None,
            }]),
            resources: vec![ReportResource {
                resource_name: ResourceName::Private("grid-meter".to_string()),
                interval_period: Some(IntervalPeriod {
                    start: timestamp,
                    duration: None,
                    randomize_start: None,
                }),
                intervals: vec![openleadr_wire::interval::Interval {
                    id: interval_id,
                    interval_period: None,
                    payloads: vec![
                        values_map_f32("NET_IMPORT_KW", site_state.grid_import.current_kw),
                        values_map_f32("NET_IMPORT_KVA", site_state.grid_import.current_kva),
                        values_map_f32("EXPORT_KW", site_state.grid_import.export_kw),
                        values_map_f32("PEAK_DEMAND_KW", site_state.utility_meter.peak_demand_kw),
                        values_map_str("OPERATING_STATE", &format!("{op_state:?}")),
                        values_map_f32("EXPORT_RATE", export_rate),
                    ],
                }],
            }],
        };

        let json = serialize_openadr(&report);
        queue.push_log(ven_name, ts_iso, "Report", "Grid Telemetry", json);
    }
}

// ─────────────────────────────────────────────────────
//  5. DR event system (VTN events) + TargetMap addressing
// ─────────────────────────────────────────────────────

/// Emit OpenADR Events when TOU period transitions or demand approaches
/// the site capacity threshold. Events now include `TargetMap` addressing.
pub fn openadr_event_system(
    multi_site: Res<MultiSiteManager>,
    mut queue: ResMut<OpenAdrMessageQueue>,
    game_clock: Res<GameClock>,
) {
    if !queue.is_active() {
        return;
    }

    let program_id = match make_program_id(DR_PROGRAM_ID) {
        Some(id) => id,
        None => return,
    };
    let sim_start = queue.sim_start;

    for (site_id, site_state) in &multi_site.owned_sites {
        let current_tou = site_state
            .site_energy_config
            .current_tou_period(game_clock.game_time);

        let timestamp = sim_start + chrono::Duration::seconds(game_clock.total_game_time as i64);
        let ts_iso = timestamp.to_rfc3339();
        let vtn_name = format!("grid-site-{}", site_id.0);

        // TOU period transition -> Price event
        {
            let der_state = queue.get_or_create(*site_id);
            let should_send = der_state.last_tou_period != Some(current_tou);
            if should_send {
                der_state.last_tou_period = Some(current_tou);
            }
            if should_send {
                let rate = site_state
                    .site_energy_config
                    .current_rate(game_clock.game_time);

                let event_content = EventContent {
                    program_id: program_id.clone(),
                    event_name: Some(format!("TOU-{}", current_tou.display_name())),
                    priority: Priority::new(5),
                    targets: site_group_target(site_id.0),
                    report_descriptors: None,
                    payload_descriptors: Some(vec![EventPayloadDescriptor {
                        payload_type: EventType::Price,
                        units: Some(Unit::Private("$/kWh".to_string())),
                        currency: None,
                    }]),
                    interval_period: Some(IntervalPeriod {
                        start: timestamp,
                        duration: None,
                        randomize_start: None,
                    }),
                    intervals: vec![EventInterval {
                        id: 0,
                        interval_period: None,
                        payloads: vec![EventValuesMap {
                            value_type: EventType::Price,
                            values: vec![ovalue_f32(rate)],
                        }],
                    }],
                };

                let json = serialize_openadr(&event_content);
                queue.push_log(
                    vtn_name.clone(),
                    ts_iso.clone(),
                    "Event",
                    "Price Signal",
                    json,
                );

                let export_rate = if site_state.challenge_level >= 2 {
                    site_state.spot_market.current_price_per_kwh
                } else {
                    site_state
                        .site_energy_config
                        .current_export_rate(game_clock.game_time)
                };

                let export_label = if site_state.challenge_level >= 2 {
                    format!("Spot-Export-{}", current_tou.display_name())
                } else {
                    format!("TOU-Export-{}", current_tou.display_name())
                };

                let solar_ven = format!("solar-site-{}", site_id.0);
                let export_event = EventContent {
                    program_id: program_id.clone(),
                    event_name: Some(export_label),
                    priority: Priority::new(5),
                    targets: ven_target(&solar_ven),
                    report_descriptors: None,
                    payload_descriptors: Some(vec![EventPayloadDescriptor {
                        payload_type: EventType::ExportPrice,
                        units: Some(Unit::Private("$/kWh".to_string())),
                        currency: None,
                    }]),
                    interval_period: Some(IntervalPeriod {
                        start: timestamp,
                        duration: None,
                        randomize_start: None,
                    }),
                    intervals: vec![EventInterval {
                        id: 0,
                        interval_period: None,
                        payloads: vec![EventValuesMap {
                            value_type: EventType::ExportPrice,
                            values: vec![ovalue_f32(export_rate)],
                        }],
                    }],
                };

                let export_json = serialize_openadr(&export_event);
                queue.push_log(
                    vtn_name.clone(),
                    ts_iso.clone(),
                    "Event",
                    "Solar Export Price",
                    export_json,
                );
            }
        }

        // Spot market grid event start/end signals (level 2+ only)
        if site_state.challenge_level >= 2 {
            let has_event = site_state.spot_market.grid_event.is_some();
            let der_state = queue.get_or_create(*site_id);
            let was_active = der_state.spot_grid_event_active;

            if has_event && !was_active {
                der_state.spot_grid_event_active = true;

                if let Some(ref grid_event) = site_state.spot_market.grid_event {
                    let event_content = EventContent {
                        program_id: program_id.clone(),
                        event_name: Some(format!("Grid-Event: {}", grid_event.name)),
                        priority: Priority::new(1),
                        targets: site_group_target(site_id.0),
                        report_descriptors: None,
                        payload_descriptors: Some(vec![EventPayloadDescriptor {
                            payload_type: EventType::ExportPrice,
                            units: Some(Unit::Private("$/kWh".to_string())),
                            currency: None,
                        }]),
                        interval_period: Some(IntervalPeriod {
                            start: timestamp,
                            duration: None,
                            randomize_start: None,
                        }),
                        intervals: vec![EventInterval {
                            id: 0,
                            interval_period: None,
                            payloads: vec![EventValuesMap {
                                value_type: EventType::ExportPrice,
                                values: vec![ovalue_f32(
                                    site_state.spot_market.current_price_per_kwh,
                                )],
                            }],
                        }],
                    };

                    let json = serialize_openadr(&event_content);
                    queue.push_log(
                        vtn_name.clone(),
                        ts_iso.clone(),
                        "Event",
                        "Grid Event Start",
                        json,
                    );
                }
            } else if !has_event && was_active {
                let der_state = queue.get_or_create(*site_id);
                der_state.spot_grid_event_active = false;
                der_state.grid_alert_emitted = false;
            }
        }

        // Demand approaching capacity -> ImportCapacityLimit event
        let capacity_kva = site_state.effective_capacity_kva();
        let current_kva = site_state.grid_import.current_kva;
        let load_ratio = if capacity_kva > 0.0 {
            current_kva / capacity_kva
        } else {
            0.0
        };

        let der_state = queue.get_or_create(*site_id);
        if load_ratio > 0.85 && !der_state.demand_event_active {
            der_state.demand_event_active = true;
            der_state.last_demand_event_game_time = game_clock.total_game_time;

            let event_content = EventContent {
                program_id: program_id.clone(),
                event_name: Some("Demand Limit Warning".to_string()),
                priority: Priority::new(2),
                targets: site_group_target(site_id.0),
                report_descriptors: None,
                payload_descriptors: Some(vec![EventPayloadDescriptor {
                    payload_type: EventType::ImportCapacityLimit,
                    units: Some(Unit::KVA),
                    currency: None,
                }]),
                interval_period: Some(IntervalPeriod {
                    start: timestamp,
                    duration: None,
                    randomize_start: None,
                }),
                intervals: vec![EventInterval {
                    id: 0,
                    interval_period: None,
                    payloads: vec![EventValuesMap {
                        value_type: EventType::ImportCapacityLimit,
                        values: vec![ovalue_f32(capacity_kva)],
                    }],
                }],
            };

            let json = serialize_openadr(&event_content);
            queue.push_log(vtn_name, ts_iso, "Event", "Demand Limit", json);
        } else if load_ratio <= 0.75 && der_state.demand_event_active {
            der_state.demand_event_active = false;
        }
    }
}

// ─────────────────────────────────────────────────────
//  6. BESS event response + dispatch setpoint system
// ─────────────────────────────────────────────────────

/// Emit `DispatchSetpoint` and `ChargeStateSetpoint` events from the VTN side,
/// plus the existing BESS DR Response report as VEN-side acknowledgment.
///
/// Fires whenever the BESS is actively discharging (not only during demand
/// limit events) so that TOU Arbitrage and PeakShaving discharge cycles
/// produce the expected OpenADR message stream.
pub fn openadr_event_response_system(
    multi_site: Res<MultiSiteManager>,
    mut queue: ResMut<OpenAdrMessageQueue>,
    game_clock: Res<GameClock>,
) {
    if !queue.is_active() {
        return;
    }

    let program_id = match make_program_id(DR_PROGRAM_ID) {
        Some(id) => id,
        None => return,
    };
    let sim_start = queue.sim_start;

    for (site_id, site_state) in &multi_site.owned_sites {
        if site_state.bess_state.capacity_kwh <= 0.0 {
            continue;
        }

        let bess_power = site_state.bess_state.current_power_kw;
        let is_discharging = bess_power > 0.0;

        let der_state = queue.get_or_create(*site_id);

        // Emit a one-shot event on discharge start/stop transitions
        if is_discharging && !der_state.bess_discharging {
            der_state.bess_discharging = true;

            let timestamp =
                sim_start + chrono::Duration::seconds(game_clock.total_game_time as i64);
            let ts_iso = timestamp.to_rfc3339();
            let ven_name = format!("bess-site-{}", site_id.0);

            let event_content = EventContent {
                program_id: program_id.clone(),
                event_name: Some("BESS Discharge Start".to_string()),
                priority: Priority::new(3),
                targets: resource_target(&ven_name, "bess-unit"),
                report_descriptors: None,
                payload_descriptors: Some(vec![EventPayloadDescriptor {
                    payload_type: EventType::DispatchSetpoint,
                    units: Some(Unit::KW),
                    currency: None,
                }]),
                interval_period: Some(IntervalPeriod {
                    start: timestamp,
                    duration: None,
                    randomize_start: None,
                }),
                intervals: vec![EventInterval {
                    id: 0,
                    interval_period: None,
                    payloads: vec![
                        EventValuesMap {
                            value_type: EventType::DispatchSetpoint,
                            values: vec![ovalue_f32(bess_power)],
                        },
                        EventValuesMap {
                            value_type: EventType::ChargeStateSetpoint,
                            values: vec![ovalue_f32(site_state.bess_state.soc_percent())],
                        },
                    ],
                }],
            };

            let json = serialize_openadr(&event_content);
            queue.push_log(ven_name, ts_iso, "Event", "BESS Discharge Start", json);
        } else if !is_discharging && der_state.bess_discharging {
            der_state.bess_discharging = false;
        }

        if !is_discharging {
            continue;
        }

        let der_state = queue.get_or_create(*site_id);
        if game_clock.total_game_time - der_state.last_demand_event_game_time
            < TELEMETRY_INTERVAL_GAME_SECS
        {
            continue;
        }
        der_state.last_demand_event_game_time = game_clock.total_game_time;
        let interval_id = der_state.next_interval();

        let timestamp = sim_start + chrono::Duration::seconds(game_clock.total_game_time as i64);
        let ts_iso = timestamp.to_rfc3339();
        let ven_name = format!("bess-site-{}", site_id.0);

        let action_label = if der_state.demand_event_active {
            "Dispatch Setpoint"
        } else {
            "BESS Dispatch"
        };

        // VTN-side: DispatchSetpoint event targeting the BESS
        let dispatch_event = EventContent {
            program_id: program_id.clone(),
            event_name: Some(action_label.to_string()),
            priority: Priority::new(2),
            targets: resource_target(&ven_name, "bess-unit"),
            report_descriptors: None,
            payload_descriptors: Some(vec![EventPayloadDescriptor {
                payload_type: EventType::DispatchSetpoint,
                units: Some(Unit::KW),
                currency: None,
            }]),
            interval_period: Some(IntervalPeriod {
                start: timestamp,
                duration: None,
                randomize_start: None,
            }),
            intervals: vec![EventInterval {
                id: 0,
                interval_period: None,
                payloads: vec![
                    EventValuesMap {
                        value_type: EventType::DispatchSetpoint,
                        values: vec![ovalue_f32(bess_power)],
                    },
                    EventValuesMap {
                        value_type: EventType::ChargeStateSetpoint,
                        values: vec![ovalue_f32(site_state.bess_state.soc_percent())],
                    },
                ],
            }],
        };

        let dispatch_json = serialize_openadr(&dispatch_event);
        queue.push_log(
            ven_name.clone(),
            ts_iso.clone(),
            "Event",
            action_label,
            dispatch_json,
        );

        // VEN-side: DR response report
        let report = ReportContent {
            program_id: program_id.clone(),
            event_id: "demand-response"
                .parse()
                .unwrap_or_else(|_| unreachable!("static event ID should always parse")),
            client_name: ven_name.clone(),
            report_name: Some("BESS DR Response".to_string()),
            payload_descriptors: Some(vec![ReportPayloadDescriptor {
                payload_type: ReportType::Setpoint,
                reading_type: ReadingType::DirectRead,
                units: Some(Unit::KW),
                accuracy: None,
                confidence: None,
            }]),
            resources: vec![ReportResource {
                resource_name: ResourceName::Private("bess-unit".to_string()),
                interval_period: Some(IntervalPeriod {
                    start: timestamp,
                    duration: None,
                    randomize_start: None,
                }),
                intervals: vec![openleadr_wire::interval::Interval {
                    id: interval_id,
                    interval_period: None,
                    payloads: vec![values_map_f32("DISCHARGE_KW", bess_power)],
                }],
            }],
        };

        let json = serialize_openadr(&report);
        queue.push_log(ven_name, ts_iso, "Report", "BESS DR Response", json);
    }
}

// ─────────────────────────────────────────────────────
//  7. Solar export event system (+ TargetMap)
// ─────────────────────────────────────────────────────

/// Emit an `ExportCapacityAvailable` event when the site transitions
/// from not-exporting to exporting (and clear when it stops).
///
/// Fires for both solar-driven and BESS-driven export so that BESS
/// discharge that spills to the grid is properly signaled.
pub fn openadr_export_event_system(
    multi_site: Res<MultiSiteManager>,
    mut queue: ResMut<OpenAdrMessageQueue>,
    game_clock: Res<GameClock>,
) {
    if !queue.is_active() {
        return;
    }

    let program_id = match make_program_id(DR_PROGRAM_ID) {
        Some(id) => id,
        None => return,
    };
    let sim_start = queue.sim_start;

    for (site_id, site_state) in &multi_site.owned_sites {
        let has_solar_export = site_state.solar_state.installed_kw_peak > 0.0
            && site_state.service_strategy.solar_export_policy != SolarExportPolicy::Never;
        let bess_exporting =
            site_state.bess_state.current_power_kw > 0.0 && site_state.grid_import.export_kw > 0.1;

        if !has_solar_export && !bess_exporting {
            // Clear state if neither source can export
            let der_state = queue.get_or_create(*site_id);
            if der_state.export_active {
                der_state.export_active = false;
            }
            continue;
        }

        let export_kw = site_state.grid_import.export_kw;
        let is_exporting = export_kw > 0.1;

        let der_state = queue.get_or_create(*site_id);

        if is_exporting && !der_state.export_active {
            der_state.export_active = true;

            let timestamp =
                sim_start + chrono::Duration::seconds(game_clock.total_game_time as i64);
            let ts_iso = timestamp.to_rfc3339();

            let (ven_name, resource_name, event_label, action_label) = if has_solar_export {
                (
                    format!("solar-site-{}", site_id.0),
                    "solar-array",
                    "Solar Export Active",
                    "Solar Export",
                )
            } else {
                (
                    format!("bess-site-{}", site_id.0),
                    "bess-unit",
                    "BESS Export Active",
                    "BESS Export",
                )
            };

            let event_content = EventContent {
                program_id: program_id.clone(),
                event_name: Some(event_label.to_string()),
                priority: Priority::new(5),
                targets: resource_target(&ven_name, resource_name),
                report_descriptors: None,
                payload_descriptors: Some(vec![EventPayloadDescriptor {
                    payload_type: EventType::ExportCapacityAvailable,
                    units: Some(Unit::KW),
                    currency: None,
                }]),
                interval_period: Some(IntervalPeriod {
                    start: timestamp,
                    duration: None,
                    randomize_start: None,
                }),
                intervals: vec![EventInterval {
                    id: 0,
                    interval_period: None,
                    payloads: vec![EventValuesMap {
                        value_type: EventType::ExportCapacityAvailable,
                        values: vec![ovalue_f32(export_kw)],
                    }],
                }],
            };

            let json = serialize_openadr(&event_content);
            queue.push_log(ven_name, ts_iso, "Event", action_label, json);
        } else if !is_exporting && der_state.export_active {
            der_state.export_active = false;
        }
    }
}

// ─────────────────────────────────────────────────────
//  8. Customer Price Signal system (+ TargetMap)
// ─────────────────────────────────────────────────────

/// Emit an OpenADR Event whenever the customer-facing effective price changes.
pub fn openadr_customer_price_system(
    multi_site: Res<MultiSiteManager>,
    mut queue: ResMut<OpenAdrMessageQueue>,
    game_clock: Res<GameClock>,
) {
    if !queue.is_active() {
        return;
    }

    let program_id = match make_program_id(DR_PROGRAM_ID) {
        Some(id) => id,
        None => return,
    };
    let sim_start = queue.sim_start;

    for (site_id, site) in &multi_site.owned_sites {
        let effective = site.service_strategy.pricing.effective_price(
            game_clock.game_time,
            &site.site_energy_config,
            site.charger_utilization,
        );

        let quantised = (effective * 100.0).round() / 100.0;

        let der_state = queue.get_or_create(*site_id);
        if der_state.last_customer_price == Some(quantised) {
            continue;
        }
        der_state.last_customer_price = Some(quantised);

        let timestamp = sim_start + chrono::Duration::seconds(game_clock.total_game_time as i64);
        let ts_iso = timestamp.to_rfc3339();
        let vtn_name = format!("grid-site-{}", site_id.0);

        let mode_label = match site.service_strategy.pricing.mode {
            PricingMode::Flat => "Flat",
            PricingMode::TouLinked => "TOU-Linked",
            PricingMode::CostPlus => "Cost-Plus",
            PricingMode::DemandResponsive => "Surge",
        };

        let event_content = EventContent {
            program_id: program_id.clone(),
            event_name: Some(format!("CustomerPrice-{mode_label}-{}", site_id.0)),
            priority: Priority::new(5),
            targets: site_group_target(site_id.0),
            report_descriptors: None,
            payload_descriptors: Some(vec![EventPayloadDescriptor {
                payload_type: EventType::Price,
                units: Some(Unit::Private("$/kWh".to_string())),
                currency: None,
            }]),
            interval_period: Some(IntervalPeriod {
                start: timestamp,
                duration: None,
                randomize_start: None,
            }),
            intervals: vec![EventInterval {
                id: 0,
                interval_period: None,
                payloads: vec![EventValuesMap {
                    value_type: EventType::Price,
                    values: vec![ovalue_f32(quantised)],
                }],
            }],
        };

        let json = serialize_openadr(&event_content);
        queue.push_log(vtn_name, ts_iso, "Event", "Customer Price Signal", json);
    }
}

// ─────────────────────────────────────────────────────
//  9. GHG signal system
// ─────────────────────────────────────────────────────

/// Emit `EventType::GHG` events when the carbon credit market rate changes,
/// providing a grid carbon-intensity signal.
pub fn openadr_ghg_signal_system(
    multi_site: Res<MultiSiteManager>,
    carbon_market: Res<CarbonCreditMarket>,
    mut queue: ResMut<OpenAdrMessageQueue>,
    game_clock: Res<GameClock>,
) {
    if !queue.is_active() {
        return;
    }

    let program_id = match make_program_id(DR_PROGRAM_ID) {
        Some(id) => id,
        None => return,
    };
    let sim_start = queue.sim_start;

    // Derive a synthetic carbon intensity from the credit rate.
    // Higher credit rate => dirtier grid => higher gCO2/kWh.
    // rate_per_kwh ranges ~$0.10-$0.20; map to ~200-800 gCO2/kWh.
    let rate = carbon_market.rate_per_kwh();
    let carbon_intensity = rate * 4000.0;
    let quantised = (carbon_intensity * 10.0).round() / 10.0;

    for site_id in multi_site.owned_sites.keys() {
        let der_state = queue.get_or_create(*site_id);
        if der_state.last_carbon_rate == Some(quantised) {
            continue;
        }
        der_state.last_carbon_rate = Some(quantised);

        let timestamp = sim_start + chrono::Duration::seconds(game_clock.total_game_time as i64);
        let ts_iso = timestamp.to_rfc3339();
        let vtn_name = format!("grid-site-{}", site_id.0);

        let event_content = EventContent {
            program_id: program_id.clone(),
            event_name: Some("GHG-Signal".to_string()),
            priority: Priority::new(4),
            targets: site_group_target(site_id.0),
            report_descriptors: None,
            payload_descriptors: Some(vec![EventPayloadDescriptor {
                payload_type: EventType::GHG,
                units: Some(Unit::Private("gCO2/kWh".to_string())),
                currency: None,
            }]),
            interval_period: Some(IntervalPeriod {
                start: timestamp,
                duration: None,
                randomize_start: None,
            }),
            intervals: vec![EventInterval {
                id: 0,
                interval_period: None,
                payloads: vec![EventValuesMap {
                    value_type: EventType::GHG,
                    values: vec![ovalue_f32(quantised)],
                }],
            }],
        };

        let json = serialize_openadr(&event_content);
        queue.push_log(vtn_name, ts_iso, "Event", "GHG Signal", json);
    }
}

// ─────────────────────────────────────────────────────
//  10. Grid alert system
// ─────────────────────────────────────────────────────

/// Emit proper OpenADR 3.0 alert events (`AlertGridEmergency`,
/// `AlertPossibleOutage`, `AlertFlexAlert`) for spot-market grid events.
pub fn openadr_grid_alert_system(
    multi_site: Res<MultiSiteManager>,
    mut queue: ResMut<OpenAdrMessageQueue>,
    game_clock: Res<GameClock>,
) {
    if !queue.is_active() {
        return;
    }

    let program_id = match make_program_id(DR_PROGRAM_ID) {
        Some(id) => id,
        None => return,
    };
    let sim_start = queue.sim_start;

    for (site_id, site_state) in &multi_site.owned_sites {
        if site_state.challenge_level < 2 {
            continue;
        }

        let grid_event = match &site_state.spot_market.grid_event {
            Some(ge) => ge,
            None => continue,
        };

        let der_state = queue.get_or_create(*site_id);
        if der_state.grid_alert_emitted {
            continue;
        }
        der_state.grid_alert_emitted = true;

        let timestamp = sim_start + chrono::Duration::seconds(game_clock.total_game_time as i64);
        let ts_iso = timestamp.to_rfc3339();
        let vtn_name = format!("grid-site-{}", site_id.0);

        let alert_type = grid_event_to_alert_type(grid_event.name);

        let event_content = EventContent {
            program_id: program_id.clone(),
            event_name: Some(format!("Grid-Alert: {}", grid_event.name)),
            priority: Priority::new(1),
            targets: site_group_target(site_id.0),
            report_descriptors: None,
            payload_descriptors: Some(vec![EventPayloadDescriptor {
                payload_type: alert_type.clone(),
                units: None,
                currency: None,
            }]),
            interval_period: Some(IntervalPeriod {
                start: timestamp,
                duration: None,
                randomize_start: None,
            }),
            intervals: vec![EventInterval {
                id: 0,
                interval_period: None,
                payloads: vec![EventValuesMap {
                    value_type: alert_type,
                    values: vec![ovalue_f32(grid_event.price_multiplier)],
                }],
            }],
        };

        let json = serialize_openadr(&event_content);
        queue.push_log(vtn_name, ts_iso, "Event", "Grid Alert", json);
    }
}

// ─────────────────────────────────────────────────────
//  11. Transformer fire event system
// ─────────────────────────────────────────────────────

/// Emit OpenADR events for transformer fire lifecycle:
///   - Fire starts → `AlertGridEmergency` + `ImportCapacityLimit = 0`
///   - Capacity restored (transformer rebuilt) → `ImportCapacityLimit = rating`
///
/// The grid telemetry system picks up the `OperatingState` change separately
/// via `grid_operating_state()`.
pub fn openadr_transformer_fire_system(
    transformers: Query<&crate::components::power::Transformer>,
    multi_site: Res<MultiSiteManager>,
    mut queue: ResMut<OpenAdrMessageQueue>,
    game_clock: Res<GameClock>,
) {
    if !queue.is_active() {
        return;
    }

    let program_id = match make_program_id(DR_PROGRAM_ID) {
        Some(id) => id,
        None => return,
    };
    let sim_start = queue.sim_start;

    for site_id in multi_site.owned_sites.keys() {
        let any_on_fire = transformers
            .iter()
            .any(|t| t.site_id == *site_id && t.on_fire);
        let any_destroyed = transformers
            .iter()
            .any(|t| t.site_id == *site_id && t.destroyed);
        let site_incident = any_on_fire || any_destroyed;

        let timestamp = sim_start + chrono::Duration::seconds(game_clock.total_game_time as i64);
        let ts_iso = timestamp.to_rfc3339();
        let vtn_name = format!("grid-site-{}", site_id.0);

        let der_state = queue.get_or_create(*site_id);

        // Fire starts: emit alert + capacity = 0
        if any_on_fire && !der_state.fire_alert_active {
            der_state.fire_alert_active = true;
            der_state.fire_capacity_zeroed = true;

            let alert = EventContent {
                program_id: program_id.clone(),
                event_name: Some("Transformer Fire".to_string()),
                priority: Priority::new(0),
                targets: site_group_target(site_id.0),
                report_descriptors: None,
                payload_descriptors: Some(vec![EventPayloadDescriptor {
                    payload_type: EventType::AlertGridEmergency,
                    units: None,
                    currency: None,
                }]),
                interval_period: Some(IntervalPeriod {
                    start: timestamp,
                    duration: None,
                    randomize_start: None,
                }),
                intervals: vec![EventInterval {
                    id: 0,
                    interval_period: None,
                    payloads: vec![EventValuesMap {
                        value_type: EventType::AlertGridEmergency,
                        values: vec![ovalue_f32(1.0)],
                    }],
                }],
            };

            let alert_json = serialize_openadr(&alert);
            queue.push_log(
                vtn_name.clone(),
                ts_iso.clone(),
                "Event",
                "Transformer Fire",
                alert_json,
            );

            let capacity_event = EventContent {
                program_id: program_id.clone(),
                event_name: Some("Transformer Fire — Capacity Loss".to_string()),
                priority: Priority::new(0),
                targets: site_group_target(site_id.0),
                report_descriptors: None,
                payload_descriptors: Some(vec![EventPayloadDescriptor {
                    payload_type: EventType::ImportCapacityLimit,
                    units: Some(Unit::KVA),
                    currency: None,
                }]),
                interval_period: Some(IntervalPeriod {
                    start: timestamp,
                    duration: None,
                    randomize_start: None,
                }),
                intervals: vec![EventInterval {
                    id: 0,
                    interval_period: None,
                    payloads: vec![EventValuesMap {
                        value_type: EventType::ImportCapacityLimit,
                        values: vec![ovalue_f32(0.0)],
                    }],
                }],
            };

            let capacity_json = serialize_openadr(&capacity_event);
            queue.push_log(
                vtn_name,
                ts_iso,
                "Event",
                "Transformer Fire Capacity Loss",
                capacity_json,
            );

            continue;
        }

        // Incident resolved: all transformers at site are healthy again
        if !site_incident && der_state.fire_alert_active {
            der_state.fire_alert_active = false;
            der_state.fire_capacity_zeroed = false;
            der_state.last_fire_capacity_kva = None;

            let total_rating_kva: f32 = transformers
                .iter()
                .filter(|t| t.site_id == *site_id)
                .map(|t| t.rating_kva)
                .sum();

            let restore = EventContent {
                program_id: program_id.clone(),
                event_name: Some("Transformer Capacity Restored".to_string()),
                priority: Priority::new(3),
                targets: site_group_target(site_id.0),
                report_descriptors: None,
                payload_descriptors: Some(vec![EventPayloadDescriptor {
                    payload_type: EventType::ImportCapacityLimit,
                    units: Some(Unit::KVA),
                    currency: None,
                }]),
                interval_period: Some(IntervalPeriod {
                    start: timestamp,
                    duration: None,
                    randomize_start: None,
                }),
                intervals: vec![EventInterval {
                    id: 0,
                    interval_period: None,
                    payloads: vec![EventValuesMap {
                        value_type: EventType::ImportCapacityLimit,
                        values: vec![ovalue_f32(total_rating_kva)],
                    }],
                }],
            };

            let restore_json = serialize_openadr(&restore);
            queue.push_log(
                vtn_name,
                ts_iso,
                "Event",
                "Transformer Capacity Restored",
                restore_json,
            );
        }
    }
}
