//! Real OCPP 1.6J client: inbound frame parsing and CSMS command handling.
//!
//! When connected to a real CSMS (`OCPP_ENDPOINT` set), the game behaves as a
//! genuine OCPP charge point: it correlates real `CallResult`s to the Calls it
//! sent, answers CSMS-initiated Calls (`SetChargingProfile`, `Reset`, ...) with
//! proper `CallResult`s, and returns a `CallError` for unsupported actions.
//!
//! Inbound handling is native-only; on WASM the client remains send-only.

use serde_json::Value;

/// A parsed inbound OCPP-J frame.
#[derive(Debug, Clone, PartialEq)]
pub enum InboundFrame {
    /// `[2, uniqueId, action, payload]` — a Call from the CSMS.
    Call {
        unique_id: String,
        action: String,
        payload: Value,
    },
    /// `[3, uniqueId, payload]` — a CallResult answering one of our Calls.
    CallResult { unique_id: String, payload: Value },
    /// `[4, uniqueId, errorCode, errorDescription, errorDetails]`.
    CallError {
        unique_id: String,
        error_code: String,
        description: String,
    },
}

/// Parse a single OCPP-J text frame.
pub fn parse_frame(text: &str) -> Result<InboundFrame, String> {
    let value: Value =
        serde_json::from_str(text).map_err(|e| format!("invalid JSON frame: {e}"))?;
    let arr = value
        .as_array()
        .ok_or_else(|| "frame is not a JSON array".to_string())?;

    let message_type = arr
        .first()
        .and_then(|v| v.as_u64())
        .ok_or_else(|| "missing message type id".to_string())?;

    let unique_id = arr
        .get(1)
        .and_then(|v| v.as_str())
        .ok_or_else(|| "missing unique id".to_string())?
        .to_string();

    match message_type {
        2 => {
            let action = arr
                .get(2)
                .and_then(|v| v.as_str())
                .ok_or_else(|| "Call missing action".to_string())?
                .to_string();
            let payload = arr.get(3).cloned().unwrap_or(Value::Null);
            Ok(InboundFrame::Call {
                unique_id,
                action,
                payload,
            })
        }
        3 => {
            let payload = arr.get(2).cloned().unwrap_or(Value::Null);
            Ok(InboundFrame::CallResult { unique_id, payload })
        }
        4 => {
            let error_code = arr
                .get(2)
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string();
            let description = arr
                .get(3)
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string();
            Ok(InboundFrame::CallError {
                unique_id,
                error_code,
                description,
            })
        }
        other => Err(format!("unknown message type id: {other}")),
    }
}

#[cfg(not(target_arch = "wasm32"))]
pub use native::ocpp_receive_system;

#[cfg(not(target_arch = "wasm32"))]
mod native {
    use std::collections::HashMap;

    use bevy::prelude::*;

    use crate::components::charger::Charger;
    use crate::resources::GameClock;

    use super::super::charging_profiles::{
        ChargingProfileStore, DEFAULT_PHASES, SUPPLY_VOLTAGE, SetProfileResult,
    };
    use super::super::connection::OcppConnectionManager;
    use super::super::queue::OcppMessageQueue;
    use super::super::types::*;
    use super::{InboundFrame, parse_frame};

    /// Drain inbound frames from every charger connection and handle them:
    /// answer CSMS Calls with CallResults, correlate CallResults to pending
    /// Calls, and return CallErrors for unsupported actions.
    pub fn ocpp_receive_system(
        conn_mgr: Res<OcppConnectionManager>,
        mut queue: ResMut<OcppMessageQueue>,
        mut store: ResMut<ChargingProfileStore>,
        mut chargers: Query<(Entity, &mut Charger)>,
        game_clock: Res<GameClock>,
    ) {
        if !queue.real_csms() {
            return;
        }

        // Collect inbound frames (charger_id, raw text) first to release the
        // connection-manager borrow before mutating the queue/store.
        let mut inbound: Vec<(String, String)> = Vec::new();
        for (charger_id, conn) in conn_mgr.connections.iter() {
            for raw in conn.handle.drain_inbound() {
                inbound.push((charger_id.clone(), raw));
            }
        }
        if inbound.is_empty() {
            return;
        }

        // Map charger id -> entity for state lookups and session control.
        let id_to_entity: HashMap<String, Entity> = chargers
            .iter()
            .map(|(entity, charger)| (charger.id.clone(), entity))
            .collect();

        for (charger_id, raw) in inbound {
            let frame = match parse_frame(&raw) {
                Ok(frame) => frame,
                Err(err) => {
                    warn!("OCPP: dropping unparseable inbound frame: {err}");
                    continue;
                }
            };

            match frame {
                InboundFrame::Call {
                    unique_id,
                    action,
                    payload,
                } => {
                    handle_call(
                        &mut queue,
                        &mut store,
                        &mut chargers,
                        &id_to_entity,
                        &game_clock,
                        &charger_id,
                        &unique_id,
                        &action,
                        &raw,
                        payload,
                    );
                }
                InboundFrame::CallResult { unique_id, payload } => {
                    handle_call_result(
                        &mut queue,
                        &game_clock,
                        &charger_id,
                        &unique_id,
                        &payload,
                        &raw,
                    );
                }
                InboundFrame::CallError {
                    unique_id,
                    error_code,
                    description,
                } => {
                    warn!(
                        "OCPP: CallError from CSMS for charger {charger_id}: {error_code} {description}"
                    );
                    log_inbound(&mut queue, &game_clock, &charger_id, "", &raw);
                    queue.pending_calls.remove(&unique_id);
                }
            }
        }
    }

    /// Append an observed inbound frame to the event log / disk buffer without
    /// enqueuing it on the outbound wire.
    fn log_inbound(
        queue: &mut OcppMessageQueue,
        game_clock: &GameClock,
        charger_id: &str,
        action: &str,
        raw: &str,
    ) {
        let ts_iso = queue
            .game_time_to_utc(game_clock.total_game_time)
            .to_rfc3339();
        queue.log_only(charger_id.to_string(), ts_iso, action, raw.to_string());
    }

    #[allow(clippy::too_many_arguments)]
    fn handle_call(
        queue: &mut OcppMessageQueue,
        store: &mut ChargingProfileStore,
        chargers: &mut Query<(Entity, &mut Charger)>,
        id_to_entity: &HashMap<String, Entity>,
        game_clock: &GameClock,
        charger_id: &str,
        unique_id: &str,
        action: &str,
        raw: &str,
        payload: serde_json::Value,
    ) {
        // Log the inbound Call under its action name.
        log_inbound(queue, game_clock, charger_id, action, raw);

        let ts_iso = queue
            .game_time_to_utc(game_clock.total_game_time)
            .to_rfc3339();
        let entity = id_to_entity.get(charger_id).copied();

        // Build a CallResult reply (or a CallError) for the action.
        let reply = match action {
            "SetChargingProfile" => {
                handle_set_charging_profile(queue, store, entity, charger_id, unique_id, payload)
            }
            "ClearChargingProfile" => {
                handle_clear_charging_profile(store, charger_id, unique_id, payload)
            }
            "GetConfiguration" => handle_get_configuration(queue, unique_id, payload),
            "ChangeConfiguration" => handle_change_configuration(queue, unique_id, payload),
            "Reset" => handle_reset(queue, entity, unique_id),
            "RemoteStopTransaction" => {
                handle_remote_stop(queue, chargers, entity, unique_id, payload)
            }
            "RemoteStartTransaction" => {
                // Cannot conjure a vehicle; decline remote-started sessions.
                Some(serialize_callresult(
                    unique_id,
                    &RemoteStartTransactionResponse {
                        status: RemoteStartStopStatus::Rejected,
                    },
                ))
            }
            "GetCompositeSchedule" => handle_get_composite_schedule(
                queue,
                store,
                entity,
                charger_id,
                unique_id,
                game_clock.total_game_time,
                payload,
            ),
            "TriggerMessage" => handle_trigger_message(queue, entity, unique_id, payload),
            other => {
                warn!("OCPP: unsupported CSMS action '{other}' for charger {charger_id}");
                Some(serialize_callerror(
                    unique_id,
                    "NotImplemented",
                    "Action not supported",
                ))
            }
        };

        if let Some(reply) = reply {
            // CallResult/CallError replies are logged with an empty action, matching
            // the existing convention for non-Call frames.
            queue.push_with_log(charger_id.to_string(), ts_iso, "", reply);
        }
    }

    /// rust-ocpp 2.0.1 incorrectly requires `ChargingSchedule::minChargingRate`
    /// on deserialize (it uses a custom serde adapter without `#[serde(default)]`).
    /// Real CSMS messages routinely omit it, so inject an explicit null into any
    /// charging-schedule object that lacks it before deserializing.
    fn normalize_charging_schedule(value: &mut serde_json::Value) {
        match value {
            serde_json::Value::Object(map) => {
                if map.contains_key("chargingSchedulePeriod")
                    && !map.contains_key("minChargingRate")
                {
                    map.insert("minChargingRate".to_string(), serde_json::Value::Null);
                }
                for v in map.values_mut() {
                    normalize_charging_schedule(v);
                }
            }
            serde_json::Value::Array(arr) => {
                for v in arr.iter_mut() {
                    normalize_charging_schedule(v);
                }
            }
            _ => {}
        }
    }

    fn handle_set_charging_profile(
        queue: &OcppMessageQueue,
        store: &mut ChargingProfileStore,
        entity: Option<Entity>,
        charger_id: &str,
        unique_id: &str,
        mut payload: serde_json::Value,
    ) -> Option<String> {
        normalize_charging_schedule(&mut payload);
        let req: SetChargingProfileRequest = match serde_json::from_value(payload) {
            Ok(req) => req,
            Err(err) => {
                return Some(serialize_callerror(
                    unique_id,
                    "FormationViolation",
                    &format!("invalid SetChargingProfile: {err}"),
                ));
            }
        };

        let active_tx_id = entity
            .and_then(|e| queue.charger_state.get(&e))
            .and_then(|s| s.transaction_id);

        let result = store.set_profile(
            charger_id,
            req.connector_id,
            req.cs_charging_profiles,
            active_tx_id,
        );
        let status = match result {
            SetProfileResult::Accepted => ChargingProfileStatus::Accepted,
            SetProfileResult::Rejected => ChargingProfileStatus::Rejected,
        };
        info!("OCPP: SetChargingProfile for {charger_id}: {status:?}");
        Some(serialize_callresult(
            unique_id,
            &SetChargingProfileResponse { status },
        ))
    }

    fn handle_clear_charging_profile(
        store: &mut ChargingProfileStore,
        charger_id: &str,
        unique_id: &str,
        payload: serde_json::Value,
    ) -> Option<String> {
        let req: ClearChargingProfileRequest = serde_json::from_value(payload).unwrap_or_default();
        let removed = store.clear_profiles(
            charger_id,
            req.id,
            req.connector_id,
            req.charging_profile_purpose,
            req.stack_level,
        );
        let status = if removed {
            ClearChargingProfileStatus::Accepted
        } else {
            ClearChargingProfileStatus::Unknown
        };
        Some(serialize_callresult(
            unique_id,
            &ClearChargingProfileResponse { status },
        ))
    }

    fn handle_get_configuration(
        queue: &OcppMessageQueue,
        unique_id: &str,
        payload: serde_json::Value,
    ) -> Option<String> {
        let req: GetConfigurationRequest = serde_json::from_value(payload).unwrap_or_default();
        let known = configuration_keys(queue);

        let (configuration_key, unknown_key) = match req.key {
            Some(requested) => {
                let mut found = Vec::new();
                let mut unknown = Vec::new();
                for key in requested {
                    match known.iter().find(|kv| kv.key.eq_ignore_ascii_case(&key)) {
                        Some(kv) => found.push(kv.clone()),
                        None => unknown.push(key),
                    }
                }
                (
                    (!found.is_empty()).then_some(found),
                    (!unknown.is_empty()).then_some(unknown),
                )
            }
            None => (Some(known), None),
        };

        Some(serialize_callresult(
            unique_id,
            &GetConfigurationResponse {
                configuration_key,
                unknown_key,
            },
        ))
    }

    fn handle_change_configuration(
        queue: &mut OcppMessageQueue,
        unique_id: &str,
        payload: serde_json::Value,
    ) -> Option<String> {
        let req: ChangeConfigurationRequest = match serde_json::from_value(payload) {
            Ok(req) => req,
            Err(err) => {
                return Some(serialize_callerror(
                    unique_id,
                    "FormationViolation",
                    &format!("invalid ChangeConfiguration: {err}"),
                ));
            }
        };

        let status = if req.key.eq_ignore_ascii_case("HeartbeatInterval") {
            match req.value.parse::<f32>() {
                Ok(secs) if secs > 0.0 => {
                    queue.heartbeat_interval_game_secs = secs;
                    ConfigurationStatus::Accepted
                }
                _ => ConfigurationStatus::Rejected,
            }
        } else if known_config_key(&req.key) {
            // Recognized but read-only / not applied in the simulation.
            ConfigurationStatus::Rejected
        } else {
            ConfigurationStatus::NotSupported
        };

        Some(serialize_callresult(
            unique_id,
            &ChangeConfigurationResponse { status },
        ))
    }

    fn handle_reset(
        queue: &mut OcppMessageQueue,
        entity: Option<Entity>,
        unique_id: &str,
    ) -> Option<String> {
        // Clear boot state so ocpp_boot_system replays the boot sequence, modelling
        // the charge point rebooting and re-announcing to the CSMS.
        if let Some(entity) = entity {
            let state = queue.get_or_create(entity);
            state.boot_sent = false;
            state.last_status = None;
        }
        Some(serialize_callresult(
            unique_id,
            &ResetResponse {
                status: ResetResponseStatus::Accepted,
            },
        ))
    }

    fn handle_remote_stop(
        queue: &OcppMessageQueue,
        chargers: &mut Query<(Entity, &mut Charger)>,
        entity: Option<Entity>,
        unique_id: &str,
        payload: serde_json::Value,
    ) -> Option<String> {
        let req: RemoteStopTransactionRequest = match serde_json::from_value(payload) {
            Ok(req) => req,
            Err(err) => {
                return Some(serialize_callerror(
                    unique_id,
                    "FormationViolation",
                    &format!("invalid RemoteStopTransaction: {err}"),
                ));
            }
        };

        let matches_active = entity
            .and_then(|e| queue.charger_state.get(&e))
            .and_then(|s| s.transaction_id)
            == Some(req.transaction_id);

        let status = if matches_active {
            if let Some(entity) = entity
                && let Ok((_, mut charger)) = chargers.get_mut(entity)
            {
                // Ending the session lets ocpp_stop_transaction_system emit the
                // StopTransaction and the driver systems tear the session down.
                charger.is_charging = false;
                charger.current_power_kw = 0.0;
            }
            RemoteStartStopStatus::Accepted
        } else {
            RemoteStartStopStatus::Rejected
        };

        Some(serialize_callresult(
            unique_id,
            &RemoteStopTransactionResponse { status },
        ))
    }

    fn handle_get_composite_schedule(
        queue: &OcppMessageQueue,
        store: &ChargingProfileStore,
        entity: Option<Entity>,
        charger_id: &str,
        unique_id: &str,
        now_total_game_time: f32,
        payload: serde_json::Value,
    ) -> Option<String> {
        let req: GetCompositeScheduleRequest = match serde_json::from_value(payload) {
            Ok(req) => req,
            Err(err) => {
                return Some(serialize_callerror(
                    unique_id,
                    "FormationViolation",
                    &format!("invalid GetCompositeSchedule: {err}"),
                ));
            }
        };

        let unit = req.charging_rate_unit.unwrap_or(ChargingRateUnitType::W);
        let now = queue.game_time_to_utc(now_total_game_time);
        let tx_start = entity
            .and_then(|e| queue.charger_state.get(&e))
            .and_then(|s| s.tx_start_total_game_time)
            .map(|t| queue.game_time_to_utc(t));

        let periods =
            composite_schedule_periods(store, charger_id, now, tx_start, req.duration, &unit);

        Some(serialize_callresult(
            unique_id,
            &GetCompositeScheduleResponse {
                status: GetCompositeScheduleStatus::Accepted,
                connector_id: Some(req.connector_id),
                schedule_start: Some(now),
                charging_schedule: Some(ChargingSchedule {
                    duration: Some(req.duration),
                    start_schedule: Some(now),
                    charging_rate_unit: unit,
                    charging_schedule_period: periods,
                    min_charging_rate: None,
                }),
            },
        ))
    }

    fn handle_trigger_message(
        queue: &mut OcppMessageQueue,
        entity: Option<Entity>,
        unique_id: &str,
        payload: serde_json::Value,
    ) -> Option<String> {
        let req: TriggerMessageRequest = match serde_json::from_value(payload) {
            Ok(req) => req,
            Err(err) => {
                return Some(serialize_callerror(
                    unique_id,
                    "FormationViolation",
                    &format!("invalid TriggerMessage: {err}"),
                ));
            }
        };

        let status = match req.requested_message {
            MessageTrigger::Heartbeat => {
                // Force the heartbeat system to emit on its next run.
                queue.last_heartbeat_game_time = f32::NEG_INFINITY;
                TriggerMessageStatus::Accepted
            }
            MessageTrigger::StatusNotification => {
                if let Some(entity) = entity {
                    queue.get_or_create(entity).last_status = None;
                }
                TriggerMessageStatus::Accepted
            }
            MessageTrigger::MeterValues => {
                if let Some(entity) = entity {
                    queue.get_or_create(entity).last_meter_game_time = f32::NEG_INFINITY;
                }
                TriggerMessageStatus::Accepted
            }
            _ => TriggerMessageStatus::NotImplemented,
        };

        Some(serialize_callresult(
            unique_id,
            &TriggerMessageResponse { status },
        ))
    }

    fn handle_call_result(
        queue: &mut OcppMessageQueue,
        game_clock: &GameClock,
        charger_id: &str,
        unique_id: &str,
        payload: &serde_json::Value,
        raw: &str,
    ) {
        log_inbound(queue, game_clock, charger_id, "", raw);

        let Some(pending) = queue.pending_calls.remove(unique_id) else {
            // Unsolicited or already-expired CallResult; nothing to correlate.
            return;
        };

        match pending.action.as_str() {
            "BootNotification" => {
                if let Ok(resp) =
                    serde_json::from_value::<BootNotificationResponse>(payload.clone())
                {
                    if resp.interval > 0 {
                        queue.heartbeat_interval_game_secs = resp.interval as f32;
                    }
                    info!(
                        "OCPP: BootNotification.conf for {charger_id}: {:?}, interval={}s",
                        resp.status, resp.interval
                    );
                }
            }
            "StartTransaction" => {
                if let Ok(resp) =
                    serde_json::from_value::<StartTransactionResponse>(payload.clone())
                {
                    let state = queue.get_or_create(pending.entity);
                    state.transaction_id = Some(resp.transaction_id);
                    state.awaiting_tx_conf = false;
                    info!(
                        "OCPP: StartTransaction.conf for {charger_id}: txn={}",
                        resp.transaction_id
                    );
                }
            }
            // Other confirmations (Heartbeat, MeterValues, StatusNotification,
            // StopTransaction) are acknowledgements only.
            _ => {}
        }
    }

    /// Known configuration keys reported by GetConfiguration.
    fn configuration_keys(queue: &OcppMessageQueue) -> Vec<KeyValue> {
        vec![
            KeyValue {
                key: "HeartbeatInterval".to_string(),
                readonly: false,
                value: Some((queue.heartbeat_interval_game_secs as i64).to_string()),
            },
            KeyValue {
                key: "MeterValueSampleInterval".to_string(),
                readonly: false,
                value: Some(
                    (super::super::queue::METER_VALUES_INTERVAL_GAME_SECS as i64).to_string(),
                ),
            },
            KeyValue {
                key: "NumberOfConnectors".to_string(),
                readonly: true,
                value: Some("1".to_string()),
            },
            KeyValue {
                key: "ChargeProfileMaxStackLevel".to_string(),
                readonly: true,
                value: Some("8".to_string()),
            },
            KeyValue {
                key: "ChargingScheduleAllowedChargingRateUnit".to_string(),
                readonly: true,
                value: Some("Current,Power".to_string()),
            },
            KeyValue {
                key: "ChargingScheduleMaxPeriods".to_string(),
                readonly: true,
                value: Some("32".to_string()),
            },
        ]
    }

    fn known_config_key(key: &str) -> bool {
        [
            "HeartbeatInterval",
            "MeterValueSampleInterval",
            "NumberOfConnectors",
            "ChargeProfileMaxStackLevel",
            "ChargingScheduleAllowedChargingRateUnit",
            "ChargingScheduleMaxPeriods",
        ]
        .iter()
        .any(|k| k.eq_ignore_ascii_case(key))
    }

    /// Build a composite schedule (list of periods) over `duration` seconds by
    /// sampling the composite limit and emitting a new period whenever it changes.
    fn composite_schedule_periods(
        store: &ChargingProfileStore,
        charger_id: &str,
        now: chrono::DateTime<chrono::Utc>,
        tx_start: Option<chrono::DateTime<chrono::Utc>>,
        duration: i32,
        unit: &ChargingRateUnitType,
    ) -> Vec<ChargingSchedulePeriod> {
        use rust_decimal::Decimal;

        // Bound the sampling horizon to a week to keep the request cheap.
        let horizon = duration.clamp(0, 604_800);

        let mut periods = Vec::new();
        let mut last_limit: Option<i64> = None;
        for t in 0..=horizon {
            let sample = now + chrono::Duration::seconds(t as i64);
            let limit_kw = store.composite_limit_kw(charger_id, sample, tx_start);
            // Represent "no constraint" as a very high value so the schedule is
            // well-formed (first period always starts at 0).
            let limit_in_unit = match limit_kw {
                Some(kw) => match unit {
                    ChargingRateUnitType::W => (kw * 1000.0) as i64,
                    ChargingRateUnitType::A => {
                        (kw * 1000.0 / (SUPPLY_VOLTAGE * DEFAULT_PHASES as f32)) as i64
                    }
                },
                None => i64::from(i32::MAX),
            };
            if last_limit != Some(limit_in_unit) {
                periods.push(ChargingSchedulePeriod {
                    start_period: t,
                    limit: Decimal::from(limit_in_unit),
                    number_phases: None,
                });
                last_limit = Some(limit_in_unit);
            }
        }
        periods
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_csms_call() {
        let frame = parse_frame(r#"[2,"abc","Reset",{"type":"Soft"}]"#).expect("should parse");
        match frame {
            InboundFrame::Call {
                unique_id,
                action,
                payload,
            } => {
                assert_eq!(unique_id, "abc");
                assert_eq!(action, "Reset");
                assert_eq!(payload["type"], "Soft");
            }
            other => panic!("expected Call, got {other:?}"),
        }
    }

    #[test]
    fn parses_call_result() {
        let frame = parse_frame(r#"[3,"xyz",{"status":"Accepted"}]"#).expect("should parse");
        match frame {
            InboundFrame::CallResult { unique_id, payload } => {
                assert_eq!(unique_id, "xyz");
                assert_eq!(payload["status"], "Accepted");
            }
            other => panic!("expected CallResult, got {other:?}"),
        }
    }

    #[test]
    fn parses_call_error() {
        let frame = parse_frame(r#"[4,"id1","NotImplemented","nope",{}]"#).expect("should parse");
        match frame {
            InboundFrame::CallError {
                unique_id,
                error_code,
                description,
            } => {
                assert_eq!(unique_id, "id1");
                assert_eq!(error_code, "NotImplemented");
                assert_eq!(description, "nope");
            }
            other => panic!("expected CallError, got {other:?}"),
        }
    }

    #[test]
    fn rejects_non_array_frame() {
        assert!(parse_frame(r#"{"not":"an array"}"#).is_err());
    }

    #[test]
    fn rejects_unknown_message_type() {
        assert!(parse_frame(r#"[9,"id",{}]"#).is_err());
    }
}
