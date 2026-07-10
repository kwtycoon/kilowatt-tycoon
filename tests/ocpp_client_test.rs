//! Integration tests for the real OCPP 1.6J client path.
//!
//! Two layers are exercised:
//! 1. The bidirectional native WebSocket connection against a tiny
//!    tokio-tungstenite mock CSMS (outbound Call is received by the server;
//!    an inbound CSMS Call reaches the client's inbound channel).
//! 2. The ECS inbound pipeline: `ocpp_receive_system` handling a
//!    `SetChargingProfile` Call (store + Accepted reply) and
//!    `apply_charging_profiles_system` capping the charger's power.
//!
//! Native-only; the client's inbound handling is not compiled for WASM.

#![cfg(not(target_arch = "wasm32"))]

use std::sync::mpsc;
use std::time::{Duration, Instant};

use bevy::prelude::*;

use kilowatt_tycoon::components::charger::Charger;
use kilowatt_tycoon::ocpp::charging_profiles::{
    ChargingProfileStore, apply_charging_profiles_system,
};
use kilowatt_tycoon::ocpp::client::{InboundFrame, ocpp_receive_system, parse_frame};
use kilowatt_tycoon::ocpp::connection::{
    ChargerConnection, ConnectionHandle, OcppConnectionManager, WsStatus,
};
use kilowatt_tycoon::ocpp::queue::OcppMessageQueue;
use kilowatt_tycoon::ocpp::types::{
    ChargingProfile, ChargingProfileKindType, ChargingProfilePurposeType, ChargingRateUnitType,
    ChargingSchedule, ChargingSchedulePeriod, SetChargingProfileRequest, serialize_call,
};
use kilowatt_tycoon::resources::GameClock;
use rust_decimal::Decimal;

/// A `SetChargingProfile` Call for connector 0 (ChargePointMaxProfile) limiting
/// the charge point to 10_000 W (10 kW). Built from the typed request and
/// serialized with the same helper `message_gen` uses, so it round-trips.
fn set_charging_profile_frame(unique_id: &str) -> String {
    let request = SetChargingProfileRequest {
        connector_id: 0,
        cs_charging_profiles: ChargingProfile {
            charging_profile_id: 100,
            transaction_id: None,
            stack_level: 0,
            charging_profile_purpose: ChargingProfilePurposeType::ChargePointMaxProfile,
            charging_profile_kind: ChargingProfileKindType::Absolute,
            recurrency_kind: None,
            valid_from: None,
            valid_to: None,
            charging_schedule: ChargingSchedule {
                duration: None,
                start_schedule: None,
                charging_rate_unit: ChargingRateUnitType::W,
                charging_schedule_period: vec![ChargingSchedulePeriod {
                    start_period: 0,
                    limit: Decimal::from(10_000),
                    number_phases: Some(3),
                }],
                min_charging_rate: None,
            },
        },
    };
    serialize_call(unique_id, "SetChargingProfile", &request)
}

// ─────────────────────────────────────────────────────
//  1. Real-socket bidirectional connection
// ─────────────────────────────────────────────────────

#[test]
fn mock_csms_round_trip_delivers_inbound_and_outbound() {
    // Signal the bound address back to the main thread.
    let (addr_tx, addr_rx) = mpsc::channel::<String>();
    // Report the outbound frame the server received from the client.
    let (recv_tx, recv_rx) = mpsc::channel::<String>();

    let server = std::thread::spawn(move || {
        let rt = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .expect("build runtime");

        rt.block_on(async move {
            use futures_util::{SinkExt, StreamExt};
            use tokio_tungstenite::tungstenite::Message;
            use tokio_tungstenite::tungstenite::handshake::server::{
                ErrorResponse, Request, Response,
            };
            use tokio_tungstenite::tungstenite::http::HeaderValue;

            // Echo the OCPP subprotocol, as a real CSMS/broker does. The client
            // requires the server to confirm the requested subprotocol.
            #[allow(clippy::result_large_err)]
            fn accept_callback(
                _req: &Request,
                mut response: Response,
            ) -> Result<Response, ErrorResponse> {
                response.headers_mut().insert(
                    "Sec-WebSocket-Protocol",
                    HeaderValue::from_static("ocpp1.6"),
                );
                Ok(response)
            }

            let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
                .await
                .expect("bind listener");
            let addr = listener.local_addr().expect("local addr");
            addr_tx.send(addr.to_string()).expect("send addr");

            let (stream, _) = listener.accept().await.expect("accept connection");
            let mut ws = tokio_tungstenite::accept_hdr_async(stream, accept_callback)
                .await
                .expect("ws handshake");

            // Push an inbound CSMS Call to the client as soon as it connects.
            ws.send(Message::Text(set_charging_profile_frame("srv-1").into()))
                .await
                .expect("send profile");

            // Read the client's outbound frame (e.g. a BootNotification) and the
            // CallResult it sends back for our SetChargingProfile.
            while let Some(Ok(msg)) = ws.next().await {
                if let Message::Text(text) = msg
                    && recv_tx.send(text.to_string()).is_err()
                {
                    break;
                }
            }
        });
    });

    let addr = addr_rx
        .recv_timeout(Duration::from_secs(5))
        .expect("server should report address");
    let url = format!("ws://{addr}/chg_01");

    let handle = ConnectionHandle::connect(url);

    // Send an outbound frame from the client (the background thread delivers it
    // once the socket is connected).
    let sender = handle.sender.as_ref().expect("sender present");
    sender
        .send(
            r#"[2,"cli-1","BootNotification",{"chargePointModel":"m","chargePointVendor":"v"}]"#
                .to_string(),
        )
        .expect("queue outbound frame");

    // Poll for the inbound SetChargingProfile Call from the mock CSMS.
    let deadline = Instant::now() + Duration::from_secs(10);
    let mut inbound = Vec::new();
    while Instant::now() < deadline {
        inbound = handle.drain_inbound();
        if !inbound.is_empty() {
            break;
        }
        std::thread::sleep(Duration::from_millis(50));
    }

    assert!(
        !inbound.is_empty(),
        "client should receive the inbound CSMS Call"
    );
    match parse_frame(&inbound[0]).expect("parse inbound") {
        InboundFrame::Call { action, .. } => assert_eq!(action, "SetChargingProfile"),
        other => panic!("expected a Call, got {other:?}"),
    }

    // The server should have received the client's outbound BootNotification.
    let received = recv_rx
        .recv_timeout(Duration::from_secs(10))
        .expect("server should receive client's outbound frame");
    assert!(received.contains("BootNotification"));

    drop(handle);
    let _ = server.join();
}

// ─────────────────────────────────────────────────────
//  2. ECS inbound handling: SetChargingProfile → cap
// ─────────────────────────────────────────────────────

/// Build a connection whose inbound channel already contains `frames`, without
/// opening a real socket. Fields on `ConnectionHandle` are public for testing.
fn connection_with_inbound(url: &str, frames: Vec<String>) -> ChargerConnection {
    let (tx, rx) = mpsc::channel::<String>();
    for frame in frames {
        tx.send(frame).expect("seed inbound frame");
    }
    // Keep `tx` alive so the receiver doesn't report disconnect; leak it into a
    // Box so the channel stays open for the duration of the test.
    Box::leak(Box::new(tx));

    let handle = ConnectionHandle {
        sender: None,
        status_rx: None,
        inbound_rx: Some(std::sync::Mutex::new(rx)),
    };

    ChargerConnection {
        status: WsStatus::Connected,
        pending: Vec::new(),
        handle,
        url: url.to_string(),
        reconnect_timer: 0.0,
    }
}

#[test]
fn set_charging_profile_is_stored_answered_and_applied() {
    let mut app = App::new();
    app.add_plugins(MinimalPlugins);

    // Real-CSMS mode so inbound handling is active.
    let mut queue = OcppMessageQueue::default();
    queue.enabled = true;
    queue.endpoint_url = "ws://localhost:9999".to_string();
    app.insert_resource(queue);

    let mut conn_mgr = OcppConnectionManager::default();
    conn_mgr.connections.insert(
        "chg_01".to_string(),
        connection_with_inbound(
            "ws://localhost:9999/chg_01",
            vec![set_charging_profile_frame("srv-1")],
        ),
    );
    app.insert_resource(conn_mgr);

    app.insert_resource(ChargingProfileStore::default());
    app.insert_resource(GameClock::default());

    let charger_entity = app
        .world_mut()
        .spawn(Charger {
            id: "chg_01".to_string(),
            rated_power_kw: 150.0,
            ..default()
        })
        .id();

    app.add_systems(
        Update,
        (ocpp_receive_system, apply_charging_profiles_system).chain(),
    );
    app.update();

    // The profile was stored.
    let store = app.world().resource::<ChargingProfileStore>();
    assert!(
        store.chargers.contains_key("chg_01"),
        "profile should be stored for the charger"
    );

    // An Accepted CallResult was queued back to the CSMS.
    let queue = app.world().resource::<OcppMessageQueue>();
    let replied = queue
        .messages
        .iter()
        .any(|(cid, json)| cid == "chg_01" && json.contains("Accepted"));
    assert!(replied, "an Accepted CallResult should be enqueued");

    // The composite limit (10 kW) was applied to the charger.
    let charger = app.world().get::<Charger>(charger_entity).expect("charger");
    let limit = charger.ocpp_limit_kw.expect("limit should be set");
    assert!(
        (limit - 10.0).abs() < 1e-3,
        "expected 10 kW cap, got {limit}"
    );
}

#[test]
fn unknown_action_yields_call_error() {
    let mut app = App::new();
    app.add_plugins(MinimalPlugins);

    let mut queue = OcppMessageQueue::default();
    queue.enabled = true;
    queue.endpoint_url = "ws://localhost:9999".to_string();
    app.insert_resource(queue);

    let mut conn_mgr = OcppConnectionManager::default();
    conn_mgr.connections.insert(
        "chg_01".to_string(),
        connection_with_inbound(
            "ws://localhost:9999/chg_01",
            vec![r#"[2,"u-1","FooBarUnsupported",{}]"#.to_string()],
        ),
    );
    app.insert_resource(conn_mgr);
    app.insert_resource(ChargingProfileStore::default());
    app.insert_resource(GameClock::default());

    app.world_mut().spawn(Charger {
        id: "chg_01".to_string(),
        ..default()
    });

    app.add_systems(Update, ocpp_receive_system);
    app.update();

    let queue = app.world().resource::<OcppMessageQueue>();
    // A CallError (message type id 4) referencing our unique id should be queued.
    let errored = queue
        .messages
        .iter()
        .any(|(_, json)| json.contains("NotImplemented") && json.starts_with("[4"));
    assert!(errored, "unknown action should produce a CallError");
}
