//! OpenADR 3.0 message generation systems.
//!
//! These Bevy systems observe per-site DER state (solar, BESS, grid) and
//! produce protocol-compliant OpenADR 3.0 messages using the `openleadr-wire`
//! crate, pushed into the [`OpenAdrMessageQueue`].

use bevy::prelude::*;

use crate::resources::{GameClock, MultiSiteManager, PricingMode, SolarExportPolicy};

use super::queue::{OpenAdrMessageQueue, TELEMETRY_INTERVAL_GAME_SECS};
use super::types::*;

/// The fixed program ID for the simulated demand-response program.
const DR_PROGRAM_ID: &str = "kw-tycoon-dr";

// ─────────────────────────────────────────────────────
//  1. VEN registration system
// ─────────────────────────────────────────────────────

/// Register solar and BESS as VENs when capacity goes from 0 to >0.
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

        // Solar VEN registration
        if !der_state.solar_registered && site_state.solar_state.installed_kw_peak > 0.0 {
            der_state.solar_registered = true;
            der_state.last_solar_kw_peak = site_state.solar_state.installed_kw_peak;

            let ven_name = format!("solar-site-{}", site_id.0);
            let ven_content = VenContent::new(ven_name.clone(), None, None, None);
            let json = serialize_openadr(&ven_content);
            queue.push_log(ven_name, ts_iso.clone(), "Ven", "Solar VEN Register", json);
        }

        // BESS VEN registration
        let der_state = queue.get_or_create(*site_id);
        if !der_state.bess_registered && site_state.bess_state.capacity_kwh > 0.0 {
            der_state.bess_registered = true;
            der_state.last_bess_kwh = site_state.bess_state.capacity_kwh;

            let ven_name = format!("bess-site-{}", site_id.0);
            let ven_content = VenContent::new(ven_name.clone(), None, None, None);
            let json = serialize_openadr(&ven_content);
            queue.push_log(ven_name, ts_iso.clone(), "Ven", "BESS VEN Register", json);
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

        // Check timing and registration; extract what we need then drop the borrow
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
                    ],
                }],
            }],
        };

        let json = serialize_openadr(&report);
        queue.push_log(ven_name, ts_iso, "Report", "Solar Telemetry", json);
    }
}

// ─────────────────────────────────────────────────────
//  3. BESS telemetry system
// ─────────────────────────────────────────────────────

/// Periodic BESS state-of-charge and power reports.
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

        let timestamp = sim_start + chrono::Duration::seconds(game_clock.total_game_time as i64);
        let ts_iso = timestamp.to_rfc3339();
        let ven_name = format!("bess-site-{}", site_id.0);

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
                    ],
                }],
            }],
        };

        let json = serialize_openadr(&report);
        queue.push_log(ven_name, ts_iso, "Report", "BESS Telemetry", json);
    }
}

// ─────────────────────────────────────────────────────
//  4. Grid telemetry system
// ─────────────────────────────────────────────────────

/// Periodic grid import and demand reports from the VTN perspective.
pub fn openadr_grid_telemetry_system(
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
        let der_state = queue.get_or_create(*site_id);

        if game_clock.total_game_time - der_state.last_telemetry_game_time
            < TELEMETRY_INTERVAL_GAME_SECS
        {
            continue;
        }

        // Mark telemetry time here so all three telemetry systems share the same tick
        der_state.last_telemetry_game_time = game_clock.total_game_time;
        let interval_id = der_state.next_interval();

        let timestamp = sim_start + chrono::Duration::seconds(game_clock.total_game_time as i64);
        let ts_iso = timestamp.to_rfc3339();
        let ven_name = format!("grid-site-{}", site_id.0);

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
                    ],
                }],
            }],
        };

        let json = serialize_openadr(&report);
        queue.push_log(ven_name, ts_iso, "Report", "Grid Telemetry", json);
    }
}

// ─────────────────────────────────────────────────────
//  5. DR event system (VTN events)
// ─────────────────────────────────────────────────────

/// Emit OpenADR Events when TOU period transitions or demand approaches
/// the site capacity threshold.
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
            // Drop der_state borrow here
            if should_send {
                let rate = site_state
                    .site_energy_config
                    .current_rate(game_clock.game_time);

                let event_content = EventContent {
                    program_id: program_id.clone(),
                    event_name: Some(format!("TOU-{}", current_tou.display_name())),
                    priority: Priority::new(5),
                    targets: None,
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

                // Emit an ExportPrice event with the buyback rate.
                // Level 2+ sites use the spot market price; level 1 uses fixed TOU.
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

                let export_event = EventContent {
                    program_id: program_id.clone(),
                    event_name: Some(export_label),
                    priority: Priority::new(5),
                    targets: None,
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
                        targets: None,
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
                targets: None,
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
//  6. BESS event response system
// ─────────────────────────────────────────────────────

/// Emit a dispatch setpoint report when BESS starts or stops
/// discharging for peak shaving in response to a demand event.
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

        let der_state = queue.get_or_create(*site_id);
        if !der_state.demand_event_active {
            continue;
        }

        // Only report when BESS is actively discharging (positive power = discharge)
        let bess_power = site_state.bess_state.current_power_kw;
        if bess_power <= 0.0 {
            continue;
        }

        // Throttle to once per telemetry interval
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
//  7. Solar export event system
// ─────────────────────────────────────────────────────

/// Emit an `ExportCapacityAvailable` event when excess solar transitions
/// from not-exporting to exporting (and clear when it stops).
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
        if site_state.service_strategy.solar_export_policy == SolarExportPolicy::Never {
            continue;
        }
        if site_state.solar_state.installed_kw_peak <= 0.0 {
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
            let vtn_name = format!("grid-site-{}", site_id.0);

            let event_content = EventContent {
                program_id: program_id.clone(),
                event_name: Some("Solar Export Active".to_string()),
                priority: Priority::new(5),
                targets: None,
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
            queue.push_log(vtn_name, ts_iso, "Event", "Solar Export", json);
        } else if !is_exporting && der_state.export_active {
            der_state.export_active = false;
        }
    }
}

// ─────────────────────────────────────────────────────
//  8. Customer Price Signal system
// ─────────────────────────────────────────────────────

/// Emit an OpenADR Event whenever the customer-facing effective price changes
/// (TOU transition, surge ramp, mode switch, etc.).
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

        // Quantise to 2 decimal places to avoid noise from floating-point jitter
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
            targets: None,
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
