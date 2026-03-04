//! OCPP 1.6J message generation systems.
//!
//! These Bevy systems observe charger state, game events, and session
//! lifecycle to produce protocol-compliant OCPP messages that are pushed
//! into the [`OcppMessageQueue`].
//!
//! All message types and enums come from the canonical `rust-ocpp` crate
//! on crates.io. Wire-format serialization uses the thin `serialize_call`
//! helper in `types.rs`.

use bevy::prelude::*;
use rust_decimal::Decimal;

use crate::components::charger::{Charger, ChargerState, ChargerType, RemoteAction};
use crate::components::driver::Driver;
use crate::components::hacker::HackerAttackType;
use crate::components::site::BelongsToSite;
use crate::events::{HackerAttackEvent, HackerDetectedEvent, RemoteActionResultEvent};
use crate::resources::GameClock;

use super::ports_registry::{PortEntry, PortsRegistry};

use super::queue::{
    HEARTBEAT_INTERVAL_GAME_SECS, METER_VALUES_INTERVAL_GAME_SECS, OcppMessageQueue,
};
use super::types::*;

// ─────────────────────────────────────────────────────
//  1. BootNotification system
// ─────────────────────────────────────────────────────

/// Send `BootNotification` (+ synthetic CallResult) and an initial
/// `StatusNotification` for every charger that hasn't been booted yet.
/// Also registers the charger in the [`PortsRegistry`] for analytics.
pub fn ocpp_boot_system(
    chargers: Query<(Entity, &Charger, Option<&BelongsToSite>)>,
    mut queue: ResMut<OcppMessageQueue>,
    mut ports: ResMut<PortsRegistry>,
    game_clock: Res<GameClock>,
) {
    if !queue.is_active() {
        return;
    }

    for (entity, charger, site_tag) in chargers.iter() {
        let state = queue.get_or_create(entity);
        if state.boot_sent {
            continue;
        }

        // Cache the charger ID
        state.charger_id = charger.id.clone();
        state.boot_sent = true;

        let charger_id = charger.id.clone();
        let timestamp = queue.game_time_to_utc(game_clock.total_game_time);
        let ts_iso = timestamp.to_rfc3339();

        // Register in PortsRegistry for analytics consumers
        let location_id = site_tag
            .map(|s| format!("site-{}", s.site_id.0))
            .unwrap_or_else(|| "site-0".to_string());
        let connector_type = match charger.charger_type {
            crate::components::charger::ChargerType::DcFast => "CCS1",
            crate::components::charger::ChargerType::AcLevel2 => "J1772",
        };
        ports.entries.push(PortEntry {
            charge_point_id: charger_id.clone(),
            location_id,
            port_id: "1".to_string(),
            connector_id: "1".to_string(),
            connector_type: connector_type.to_string(),
            commissioned_ts: Some(ts_iso.clone()),
        });

        // Determine model from charger type
        let model = match charger.charger_type {
            crate::components::charger::ChargerType::DcFast => "DCFC-50kW",
            crate::components::charger::ChargerType::AcLevel2 => "AC-L2-7kW",
        };

        // BootNotification Call
        let uid = new_unique_id();
        let boot = BootNotificationRequest {
            charge_point_model: model.to_string(),
            charge_point_vendor: "KilowattTycoon".to_string(),
            charge_point_serial_number: Some(charger_id.clone()),
            firmware_version: Some("1.0.0".to_string()),
            ..Default::default()
        };
        queue.push_with_log(
            charger_id.clone(),
            ts_iso.clone(),
            "BootNotification",
            serialize_call(&uid, "BootNotification", &boot),
        );

        // BootNotification CallResult (synthetic)
        let boot_resp = BootNotificationResponse {
            status: RegistrationStatus::Accepted,
            current_time: timestamp,
            interval: 300,
        };
        queue.push_with_log(
            charger_id.clone(),
            ts_iso.clone(),
            "",
            serialize_callresult(&uid, &boot_resp),
        );

        // Initial StatusNotification
        let ocpp_status = charger_state_to_ocpp_status(charger.state());
        let error_code = charger
            .current_fault
            .map(fault_to_ocpp_error)
            .unwrap_or(ChargePointErrorCode::NoError);

        let status_notif = StatusNotificationRequest {
            connector_id: 1,
            error_code,
            status: ocpp_status.clone(),
            timestamp: Some(timestamp),
            info: charger.current_fault.map(|f| f.display_name().to_string()),
            vendor_id: charger.current_fault.map(|_| "KilowattTycoon".to_string()),
            vendor_error_code: charger.current_fault.map(|f| format!("{f:?}")),
        };
        queue.push_with_log(
            charger_id,
            ts_iso,
            "StatusNotification",
            serialize_call(&new_unique_id(), "StatusNotification", &status_notif),
        );

        // Update tracked status
        let state = queue.get_or_create(entity);
        state.last_status = Some(ocpp_status);
    }
}

// ─────────────────────────────────────────────────────
//  2. StatusNotification system (state change detection)
// ─────────────────────────────────────────────────────

/// Helper to push a `StatusNotification` and return the status that was sent.
fn push_status_notif(
    queue: &mut OcppMessageQueue,
    charger_id: &str,
    ts_iso: &str,
    timestamp: chrono::DateTime<chrono::Utc>,
    status: ChargePointStatus,
    error_code: ChargePointErrorCode,
    info: Option<String>,
    vendor_id: Option<String>,
    vendor_error_code: Option<String>,
) {
    let notif = StatusNotificationRequest {
        connector_id: 1,
        error_code,
        status,
        timestamp: Some(timestamp),
        info,
        vendor_id,
        vendor_error_code,
    };
    queue.push_with_log(
        charger_id.to_string(),
        ts_iso.to_string(),
        "StatusNotification",
        serialize_call(&new_unique_id(), "StatusNotification", &notif),
    );
}

/// Compare each charger's current state with the last sent status.
/// If it changed, emit a `StatusNotification`.
///
/// Session lifecycle transitions (Preparing/Finishing) are handled by the
/// transaction systems: `ocpp_start_transaction_system` owns
/// Preparing → StartTx → Charging and `ocpp_stop_transaction_system` owns
/// StopTx → Finishing → Available. This system only emits non-transaction
/// state changes (Faulted, Unavailable, reboots, etc.).
pub fn ocpp_status_system(
    chargers: Query<(Entity, &Charger)>,
    mut queue: ResMut<OcppMessageQueue>,
    game_clock: Res<GameClock>,
) {
    if !queue.is_active() {
        return;
    }

    for (entity, charger) in chargers.iter() {
        let ocpp_status = charger_state_to_ocpp_status(charger.state());

        let charger_id = {
            let state = queue.get_or_create(entity);
            if !state.boot_sent {
                continue;
            }

            if state.last_status.as_ref() == Some(&ocpp_status) {
                continue;
            }

            state.last_status = Some(ocpp_status.clone());
            state.charger_id.clone()
        };

        let error_code = charger
            .current_fault
            .map(fault_to_ocpp_error)
            .unwrap_or(ChargePointErrorCode::NoError);

        let info = charger.current_fault.map(|f| f.display_name().to_string());
        let vendor_id = charger.current_fault.map(|_| "KilowattTycoon".to_string());
        let vendor_error_code = charger.current_fault.map(|f| format!("{f:?}"));

        let timestamp = queue.game_time_to_utc(game_clock.total_game_time);
        let ts_iso = timestamp.to_rfc3339();

        push_status_notif(
            &mut queue,
            &charger_id,
            &ts_iso,
            timestamp,
            ocpp_status,
            error_code,
            info,
            vendor_id,
            vendor_error_code,
        );
    }
}

// ─────────────────────────────────────────────────────
//  3. StartTransaction system
// ─────────────────────────────────────────────────────

/// Detect when a charger transitions to `is_charging` and emit `StartTransaction`.
/// We detect this by checking chargers that are charging but don't have an active
/// OCPP transaction yet.
///
/// For roaming sessions (driver.is_roaming), a `RemoteStartTransaction` request/
/// response pair is emitted before the normal Preparing -> StartTransaction ->
/// Charging sequence, matching real CSMS-initiated session starts.
pub fn ocpp_start_transaction_system(
    chargers: Query<(Entity, &Charger)>,
    drivers: Query<(Entity, &Driver)>,
    mut queue: ResMut<OcppMessageQueue>,
    game_clock: Res<GameClock>,
) {
    if !queue.is_active() {
        return;
    }

    for (entity, charger) in chargers.iter() {
        if !charger.is_charging {
            continue;
        }

        // Check if we already have a transaction for this charger
        let needs_start = {
            let state = queue.get_or_create(entity);
            if !state.boot_sent {
                continue;
            }
            state.transaction_id.is_none()
        };

        if !needs_start {
            continue;
        }

        // Find the driver assigned to this charger
        let driver_info = drivers
            .iter()
            .find(|(_, d)| d.assigned_charger == Some(entity))
            .map(|(de, d)| (de, d.evcc_id.clone(), d.is_roaming));

        let (driver_entity, id_tag, is_roaming) = match driver_info {
            Some((de, evcc, roaming)) => (Some(de), format!("VID:{evcc}"), roaming),
            None => (None, "VID:000000000000".to_string(), false),
        };

        let meter_start_wh = (charger.total_energy_delivered_kwh * 1000.0) as i32;
        let transaction_id = queue.next_transaction_id();
        let timestamp = queue.game_time_to_utc(game_clock.total_game_time);
        let charger_id = charger.id.clone();

        // For roaming sessions, emit RemoteStartTransaction (CSMS → CP) first
        if is_roaming {
            let ts_remote = (timestamp - chrono::Duration::seconds(3)).to_rfc3339();
            let remote_uid = new_unique_id();

            let remote_start = RemoteStartTransactionRequest {
                connector_id: Some(1),
                id_tag: id_tag.clone(),
                charging_profile: None,
            };
            queue.push_with_log(
                charger_id.clone(),
                ts_remote.clone(),
                "RemoteStartTransaction",
                serialize_call(&remote_uid, "RemoteStartTransaction", &remote_start),
            );

            let remote_resp = RemoteStartTransactionResponse {
                status: RemoteStartStopStatus::Accepted,
            };
            queue.push_with_log(
                charger_id.clone(),
                ts_remote,
                "",
                serialize_callresult(&remote_uid, &remote_resp),
            );
        }

        // Emit the full OCPP 1.6 connector lifecycle with proper timestamp offsets.
        // kwwhat does not rely on this sequence and accounts for charger being offline
        // but still a right thing to do: Preparing → StartTransaction → Charging
        let ts_prep = (timestamp - chrono::Duration::seconds(2)).to_rfc3339();
        let ts_tx = (timestamp - chrono::Duration::seconds(1)).to_rfc3339();
        let ts_charging = timestamp.to_rfc3339();

        // 1. StatusNotification(Preparing) -- marks the start of a charge attempt
        let preparing = StatusNotificationRequest {
            connector_id: 1,
            error_code: ChargePointErrorCode::NoError,
            status: ChargePointStatus::Preparing,
            timestamp: Some(timestamp - chrono::Duration::seconds(2)),
            info: None,
            vendor_id: None,
            vendor_error_code: None,
        };
        queue.push_with_log(
            charger_id.clone(),
            ts_prep,
            "StatusNotification",
            serialize_call(&new_unique_id(), "StatusNotification", &preparing),
        );

        // 2. StartTransaction Call
        let uid = new_unique_id();
        let start_tx = StartTransactionRequest {
            connector_id: 1,
            id_tag: id_tag.clone(),
            meter_start: meter_start_wh,
            timestamp: timestamp - chrono::Duration::seconds(1),
            reservation_id: None,
        };
        queue.push_with_log(
            charger_id.clone(),
            ts_tx.clone(),
            "StartTransaction",
            serialize_call(&uid, "StartTransaction", &start_tx),
        );

        // 3. StartTransaction CallResult (synthetic -- provides transactionId)
        let start_resp = StartTransactionResponse {
            transaction_id,
            id_tag_info: IdTagInfo {
                status: AuthorizationStatus::Accepted,
                ..Default::default()
            },
        };
        queue.push_with_log(
            charger_id.clone(),
            ts_tx,
            "",
            serialize_callresult(&uid, &start_resp),
        );

        // 4. StatusNotification(Charging) -- suppresses duplicate from status_system
        let charging = StatusNotificationRequest {
            connector_id: 1,
            error_code: ChargePointErrorCode::NoError,
            status: ChargePointStatus::Charging,
            timestamp: Some(timestamp),
            info: None,
            vendor_id: None,
            vendor_error_code: None,
        };
        queue.push_with_log(
            charger_id,
            ts_charging,
            "StatusNotification",
            serialize_call(&new_unique_id(), "StatusNotification", &charging),
        );

        // Record transaction state + update last_status to Charging so
        // ocpp_status_system doesn't emit a duplicate Charging notification.
        let state = queue.get_or_create(entity);
        state.transaction_id = Some(transaction_id);
        state.meter_start_wh = meter_start_wh;
        state.active_driver = driver_entity;
        state.active_id_tag = Some(id_tag.clone());
        state.last_meter_game_time = game_clock.total_game_time;
        state.last_status = Some(ChargePointStatus::Charging);

        info!(
            "OCPP StartTransaction: charger={}, txn={}, driver={}{}",
            state.charger_id,
            transaction_id,
            id_tag,
            if is_roaming { " [roaming]" } else { "" }
        );
    }
}

// ─────────────────────────────────────────────────────
//  4. StopTransaction system
// ─────────────────────────────────────────────────────

/// Detect when a charger stops charging and emit `StopTransaction`.
/// Triggered when a charger has an active OCPP transaction but is no longer charging.
pub fn ocpp_stop_transaction_system(
    chargers: Query<(Entity, &Charger)>,
    mut queue: ResMut<OcppMessageQueue>,
    game_clock: Res<GameClock>,
) {
    if !queue.is_active() {
        return;
    }

    // Collect entities that need StopTransaction to avoid borrow conflicts.
    // Track whether the stop is fault-induced so we can emit the right status lifecycle.
    let to_stop: Vec<(
        Entity,
        String,
        i32,
        i32,
        Option<Reason>,
        Option<String>,
        bool,
    )> = chargers
        .iter()
        .filter_map(|(entity, charger)| {
            let state = queue.charger_state.get(&entity)?;
            let txn_id = state.transaction_id?;

            if charger.is_charging {
                return None;
            }

            let meter_stop_wh = (charger.total_energy_delivered_kwh * 1000.0) as i32;
            let is_fault = charger.current_fault.is_some();
            let reason = if is_fault {
                Some(Reason::PowerLoss)
            } else {
                Some(Reason::Local)
            };

            Some((
                entity,
                state.charger_id.clone(),
                txn_id,
                meter_stop_wh,
                reason,
                state.active_id_tag.clone(),
                is_fault,
            ))
        })
        .collect();

    for (entity, charger_id, txn_id, meter_stop_wh, reason, id_tag, is_fault) in to_stop {
        let timestamp = queue.game_time_to_utc(game_clock.total_game_time);
        let ts_stop = timestamp.to_rfc3339();
        let ts_finishing = (timestamp + chrono::Duration::seconds(1)).to_rfc3339();
        let ts_available = (timestamp + chrono::Duration::seconds(2)).to_rfc3339();

        // Final meter reading
        let final_meter = MeterValue {
            timestamp,
            sampled_value: vec![SampledValue {
                value: meter_stop_wh.to_string(),
                context: Some(ReadingContext::TransactionEnd),
                format: Some(ValueFormat::Raw),
                measurand: Some(Measurand::EnergyActiveImportRegister),
                phase: None,
                location: Some(Location::Outlet),
                unit: Some(UnitOfMeasure::Wh),
            }],
        };

        // 1. StopTransaction
        let stop_tx = StopTransactionRequest {
            id_tag,
            meter_stop: meter_stop_wh,
            timestamp,
            transaction_id: txn_id,
            reason,
            transaction_data: Some(vec![final_meter]),
        };
        queue.push_with_log(
            charger_id.clone(),
            ts_stop,
            "StopTransaction",
            serialize_call(&new_unique_id(), "StopTransaction", &stop_tx),
        );

        // 2. Finishing + Available (normal completion only).
        // Fault-induced stops skip this — ocpp_status_system will emit
        // the Faulted StatusNotification instead.
        let final_status = if is_fault {
            None
        } else {
            push_status_notif(
                &mut queue,
                &charger_id,
                &ts_finishing,
                timestamp + chrono::Duration::seconds(1),
                ChargePointStatus::Finishing,
                ChargePointErrorCode::NoError,
                None,
                None,
                None,
            );
            push_status_notif(
                &mut queue,
                &charger_id,
                &ts_available,
                timestamp + chrono::Duration::seconds(2),
                ChargePointStatus::Available,
                ChargePointErrorCode::NoError,
                None,
                None,
                None,
            );
            Some(ChargePointStatus::Available)
        };

        // Clear transaction state and update last_status
        if let Some(state) = queue.charger_state.get_mut(&entity) {
            info!(
                "OCPP StopTransaction: charger={}, txn={}",
                charger_id, txn_id
            );
            state.transaction_id = None;
            state.active_driver = None;
            state.active_id_tag = None;
            if let Some(status) = final_status {
                state.last_status = Some(status);
            }
        }
    }
}

// ─────────────────────────────────────────────────────
//  5. MeterValues system (periodic sampling)
// ─────────────────────────────────────────────────────

/// Compute outlet voltage (V) for a charger given the vehicle's SOC.
/// DCFC: models a 400V-class battery pack, 300 V at 0% rising to 400 V at 100%.
/// L2 AC: fixed 240 V nominal.
fn outlet_voltage(charger_type: ChargerType, soc_pct: Option<f32>) -> f32 {
    match charger_type {
        ChargerType::DcFast => {
            let soc_frac = soc_pct.unwrap_or(50.0) / 100.0;
            300.0 + soc_frac * 100.0
        }
        ChargerType::AcLevel2 => 240.0,
    }
}

/// Periodically emit `MeterValues` for all actively charging connectors.
/// Samples: energy (Wh), power (W), SoC (%), voltage (V), current (A).
pub fn ocpp_meter_values_system(
    chargers: Query<(Entity, &Charger)>,
    drivers: Query<&Driver>,
    mut queue: ResMut<OcppMessageQueue>,
    game_clock: Res<GameClock>,
) {
    if !queue.is_active() || game_clock.is_paused() {
        return;
    }

    let now = game_clock.total_game_time;

    // Collect what we need to avoid borrow conflicts
    let to_send: Vec<(Entity, String, i32, f32, f32, Option<f32>, ChargerType)> = chargers
        .iter()
        .filter_map(|(entity, charger)| {
            if !charger.is_charging {
                return None;
            }

            let state = queue.charger_state.get(&entity)?;
            let txn_id = state.transaction_id?;

            if now - state.last_meter_game_time < METER_VALUES_INTERVAL_GAME_SECS {
                return None;
            }

            let active_driver = state.active_driver.and_then(|de| drivers.get(de).ok());

            let soc = active_driver.map(|d| d.charge_progress() * 100.0);

            let session_energy_wh = active_driver
                .map(|d| d.charge_received_kwh * 1000.0)
                .unwrap_or(0.0);
            let energy_wh = state.meter_start_wh as f32 + session_energy_wh;
            let power_w = charger.current_power_kw * 1000.0;

            Some((
                entity,
                charger.id.clone(),
                txn_id,
                energy_wh,
                power_w,
                soc,
                charger.charger_type,
            ))
        })
        .collect();

    for (entity, charger_id, txn_id, energy_wh, power_w, soc, charger_type) in to_send {
        let timestamp = queue.game_time_to_utc(now);
        let ts_iso = timestamp.to_rfc3339();

        let voltage_v = outlet_voltage(charger_type, soc);
        let current_a = if voltage_v > 0.0 {
            power_w / voltage_v
        } else {
            0.0
        };

        let mut sampled = vec![
            SampledValue {
                value: format!("{:.0}", energy_wh),
                context: Some(ReadingContext::SamplePeriodic),
                format: Some(ValueFormat::Raw),
                measurand: Some(Measurand::EnergyActiveImportRegister),
                phase: None,
                location: Some(Location::Outlet),
                unit: Some(UnitOfMeasure::Wh),
            },
            SampledValue {
                value: format!("{:.0}", power_w),
                context: Some(ReadingContext::SamplePeriodic),
                format: Some(ValueFormat::Raw),
                measurand: Some(Measurand::PowerActiveImport),
                phase: None,
                location: Some(Location::Outlet),
                unit: Some(UnitOfMeasure::W),
            },
            SampledValue {
                value: format!("{:.1}", voltage_v),
                context: Some(ReadingContext::SamplePeriodic),
                format: Some(ValueFormat::Raw),
                measurand: Some(Measurand::Voltage),
                phase: None,
                location: Some(Location::Outlet),
                unit: Some(UnitOfMeasure::V),
            },
            SampledValue {
                value: format!("{:.1}", current_a),
                context: Some(ReadingContext::SamplePeriodic),
                format: Some(ValueFormat::Raw),
                measurand: Some(Measurand::CurrentImport),
                phase: None,
                location: Some(Location::Outlet),
                unit: Some(UnitOfMeasure::A),
            },
        ];

        if let Some(soc_val) = soc {
            sampled.push(SampledValue {
                value: format!("{:.0}", soc_val),
                context: Some(ReadingContext::SamplePeriodic),
                format: Some(ValueFormat::Raw),
                measurand: Some(Measurand::SoC),
                phase: None,
                location: None,
                unit: Some(UnitOfMeasure::Percent),
            });
        }

        let meter = MeterValuesRequest {
            connector_id: 1,
            transaction_id: Some(txn_id),
            meter_value: vec![MeterValue {
                timestamp,
                sampled_value: sampled,
            }],
        };
        queue.push_with_log(
            charger_id,
            ts_iso,
            "MeterValues",
            serialize_call(&new_unique_id(), "MeterValues", &meter),
        );

        if let Some(state) = queue.charger_state.get_mut(&entity) {
            state.last_meter_game_time = now;
        }
    }
}

// ─────────────────────────────────────────────────────
//  6. Heartbeat system
// ─────────────────────────────────────────────────────

/// Periodically emit `Heartbeat` messages (one per charger).
pub fn ocpp_heartbeat_system(
    chargers: Query<(Entity, &Charger)>,
    mut queue: ResMut<OcppMessageQueue>,
    game_clock: Res<GameClock>,
) {
    if !queue.is_active() || game_clock.is_paused() {
        return;
    }

    let now = game_clock.total_game_time;

    if now - queue.last_heartbeat_game_time < HEARTBEAT_INTERVAL_GAME_SECS {
        return;
    }

    queue.last_heartbeat_game_time = now;

    let timestamp = queue.game_time_to_utc(now);
    let ts_iso = timestamp.to_rfc3339();
    let heartbeat = HeartbeatRequest {};

    for (entity, charger) in chargers.iter() {
        let state = queue.get_or_create(entity);
        if !state.boot_sent {
            continue;
        }

        if matches!(
            charger.state(),
            ChargerState::Offline | ChargerState::Disabled
        ) {
            continue;
        }

        // Heartbeat Call
        let uid = new_unique_id();
        queue.push_with_log(
            charger.id.clone(),
            ts_iso.clone(),
            "Heartbeat",
            serialize_call(&uid, "Heartbeat", &heartbeat),
        );

        // Heartbeat CallResult (synthetic -- needed for offline detection)
        let hb_resp = HeartbeatResponse {
            current_time: timestamp,
        };
        queue.push_with_log(
            charger.id.clone(),
            ts_iso.clone(),
            "",
            serialize_callresult(&uid, &hb_resp),
        );
    }
}

// ─────────────────────────────────────────────────────
//  7. Reset system (reboot → re-boot sequence)
// ─────────────────────────────────────────────────────

/// When a successful `SoftReboot` or `HardReboot` action fires, emit a
/// `Reset.req` + `Reset.conf` and clear `boot_sent` so the boot system
/// replays the full BootNotification sequence on the next frame.
pub fn ocpp_reset_system(
    mut action_events: MessageReader<RemoteActionResultEvent>,
    mut queue: ResMut<OcppMessageQueue>,
    game_clock: Res<GameClock>,
) {
    if !queue.is_active() {
        return;
    }

    for event in action_events.read() {
        if !event.success {
            continue;
        }

        let reset_type = match event.action {
            RemoteAction::SoftReboot => ResetRequestStatus::Soft,
            RemoteAction::HardReboot => ResetRequestStatus::Hard,
            _ => continue,
        };

        let Some(state) = queue.charger_state.get(&event.charger_entity) else {
            continue;
        };
        let charger_id = state.charger_id.clone();
        if charger_id.is_empty() {
            continue;
        }

        let timestamp = queue.game_time_to_utc(game_clock.total_game_time);
        let ts_iso = timestamp.to_rfc3339();

        // Reset Call (CSMS → CP direction)
        let uid = new_unique_id();
        let reset_req = ResetRequest { kind: reset_type };
        queue.push_with_log(
            charger_id.clone(),
            ts_iso.clone(),
            "Reset",
            serialize_call(&uid, "Reset", &reset_req),
        );

        // Reset CallResult (CP → CSMS)
        let reset_resp = ResetResponse {
            status: ResetResponseStatus::Accepted,
        };
        queue.push_with_log(
            charger_id,
            ts_iso,
            "",
            serialize_callresult(&uid, &reset_resp),
        );

        // Clear boot state so ocpp_boot_system replays the full sequence
        let state = queue.get_or_create(event.charger_entity);
        state.boot_sent = false;
        state.last_status = None;
    }
}

// ─────────────────────────────────────────────────────
//  8. Day-end cleanup system
// ─────────────────────────────────────────────────────

/// Emit a final `StatusNotification(Available)` and `Heartbeat` for every
/// charger when entering the `DayEnd` state. Always emits Available regardless
/// of the charger's actual state so that open fault/offline periods in the
/// analytics are cleanly closed at the day boundary. Uses T+1s offset to
/// sort after any same-timestamp events from earlier systems.
pub fn ocpp_day_end_system(
    chargers: Query<(Entity, &Charger)>,
    mut queue: ResMut<OcppMessageQueue>,
    game_clock: Res<GameClock>,
) {
    if !queue.is_active() {
        return;
    }

    for (entity, _charger) in chargers.iter() {
        let state = queue.get_or_create(entity);
        if !state.boot_sent {
            continue;
        }

        let charger_id = state.charger_id.clone();
        let boundary =
            queue.game_time_to_utc(game_clock.total_game_time) + chrono::Duration::seconds(1);
        let ts_iso = boundary.to_rfc3339();

        push_status_notif(
            &mut queue,
            &charger_id,
            &ts_iso,
            boundary,
            ChargePointStatus::Available,
            ChargePointErrorCode::NoError,
            None,
            None,
            None,
        );

        let uid = new_unique_id();
        let heartbeat = HeartbeatRequest {};
        queue.push_with_log(
            charger_id.clone(),
            ts_iso.clone(),
            "Heartbeat",
            serialize_call(&uid, "Heartbeat", &heartbeat),
        );
        let hb_resp = HeartbeatResponse {
            current_time: boundary,
        };
        queue.push_with_log(
            charger_id.clone(),
            ts_iso,
            "",
            serialize_callresult(&uid, &hb_resp),
        );

        let state = queue.get_or_create(entity);
        state.last_status = Some(ChargePointStatus::Available);
    }
}

// ─────────────────────────────────────────────────────
//  9. SetChargingProfile system (peak shave threshold)
// ─────────────────────────────────────────────────────

/// Emit `SetChargingProfile` (CSMS → CP) with a `ChargePointMaxProfile`
/// whenever the player changes the peak shave threshold.
///
/// Uses a `Local` to track the last-emitted threshold so we only emit
/// on actual changes.
pub fn ocpp_charging_profile_system(
    chargers: Query<(Entity, &Charger)>,
    mut queue: ResMut<OcppMessageQueue>,
    game_clock: Res<GameClock>,
    multi_site: Res<crate::resources::MultiSiteManager>,
    mut last_threshold: Local<Option<f32>>,
) {
    if !queue.is_active() {
        return;
    }

    let Some(site_state) = multi_site.active_site() else {
        return;
    };

    let current_threshold = site_state.bess_state.peak_shave_threshold;

    let threshold_changed = match *last_threshold {
        Some(prev) => (prev - current_threshold).abs() > f32::EPSILON,
        None => true,
    };

    if !threshold_changed {
        return;
    }
    *last_threshold = Some(current_threshold);

    let site_capacity_kva = site_state.effective_capacity_kva();
    let limit_watts = site_capacity_kva * current_threshold * 1000.0;
    let timestamp = queue.game_time_to_utc(game_clock.total_game_time);
    let ts_iso = timestamp.to_rfc3339();

    for (entity, _charger) in chargers.iter() {
        let state = queue.get_or_create(entity);
        if !state.boot_sent {
            continue;
        }
        let charger_id = state.charger_id.clone();

        let uid = new_unique_id();
        let profile = SetChargingProfileRequest {
            connector_id: 0,
            cs_charging_profiles: ChargingProfile {
                charging_profile_id: 1,
                transaction_id: None,
                stack_level: 0,
                charging_profile_purpose: ChargingProfilePurposeType::ChargePointMaxProfile,
                charging_profile_kind: ChargingProfileKindType::Absolute,
                recurrency_kind: None,
                valid_from: Some(timestamp),
                valid_to: None,
                charging_schedule: ChargingSchedule {
                    duration: None,
                    start_schedule: Some(timestamp),
                    charging_rate_unit: ChargingRateUnitType::W,
                    charging_schedule_period: vec![ChargingSchedulePeriod {
                        start_period: 0,
                        limit: Decimal::from(limit_watts as i64),
                        number_phases: Some(3),
                    }],
                    min_charging_rate: None,
                },
            },
        };
        queue.push_with_log(
            charger_id.clone(),
            ts_iso.clone(),
            "SetChargingProfile",
            serialize_call(&uid, "SetChargingProfile", &profile),
        );

        let resp = SetChargingProfileResponse {
            status: ChargingProfileStatus::Accepted,
        };
        queue.push_with_log(
            charger_id,
            ts_iso.clone(),
            "",
            serialize_callresult(&uid, &resp),
        );
    }
}

// ─────────────────────────────────────────────────────
//  10. Hacker security event system
// ─────────────────────────────────────────────────────

/// Emits explicit OCPP StatusNotification messages for hacker attacks and mitigations.
pub fn ocpp_hacker_event_system(
    chargers: Query<(Entity, &Charger)>,
    mut queue: ResMut<OcppMessageQueue>,
    game_clock: Res<GameClock>,
    mut attack_events: MessageReader<HackerAttackEvent>,
    mut detected_events: MessageReader<HackerDetectedEvent>,
) {
    if !queue.is_active() {
        attack_events.read().for_each(|_| {});
        detected_events.read().for_each(|_| {});
        return;
    }

    let timestamp = queue.game_time_to_utc(game_clock.total_game_time);
    let ts_iso = timestamp.to_rfc3339();

    for event in attack_events.read() {
        let (info_text, vendor_code) = match event.attack_type {
            HackerAttackType::TransformerOverload => {
                ("CyberAttack: TransformerOverload", "CYBER_OVERLOAD")
            }
            HackerAttackType::PriceSlash => ("CyberAttack: PriceManipulation", "CYBER_PRICE_SLASH"),
        };

        for (entity, _charger) in chargers.iter() {
            let charger_id = {
                let state = queue.get_or_create(entity);
                if !state.boot_sent {
                    continue;
                }
                state.charger_id.clone()
            };

            push_status_notif(
                &mut queue,
                &charger_id,
                &ts_iso,
                timestamp,
                ChargePointStatus::Faulted,
                ChargePointErrorCode::OtherError,
                Some(info_text.to_string()),
                Some("KilowattTycoon".to_string()),
                Some(vendor_code.to_string()),
            );
        }
    }

    for event in detected_events.read() {
        if !event.auto_blocked {
            continue;
        }

        let info_text = match event.attack_type {
            HackerAttackType::TransformerOverload => {
                "CyberAttack: TransformerOverload mitigated by Agentic SOC"
            }
            HackerAttackType::PriceSlash => {
                "CyberAttack: PriceManipulation mitigated by Agentic SOC"
            }
        };

        for (entity, _charger) in chargers.iter() {
            let charger_id = {
                let state = queue.get_or_create(entity);
                if !state.boot_sent {
                    continue;
                }
                state.charger_id.clone()
            };

            push_status_notif(
                &mut queue,
                &charger_id,
                &ts_iso,
                timestamp,
                ChargePointStatus::Available,
                ChargePointErrorCode::NoError,
                Some(info_text.to_string()),
                Some("KilowattTycoon".to_string()),
                Some("CYBER_MITIGATED".to_string()),
            );
        }
    }
}
