# Kilowatt Tycoon — OCPP 1.6J Client

## 1. Purpose

Kilowatt Tycoon's chargers speak **OCPP 1.6J** (JSON over WebSocket). The game can
run in two modes:

- **Synthetic mode** (default, no endpoint configured): the game fabricates the
  CSMS side of the conversation locally so logs and the in-browser feed show a
  complete, well-formed OCPP transcript. Nothing leaves the machine except the
  optional disk log.
- **Real-CSMS mode** (`OCPP_ENDPOINT` set): the game behaves as a genuine OCPP
  charge point. It opens one WebSocket per charger, sends real Calls, consumes
  the CSMS's real CallResults, answers CSMS-initiated Calls, and **applies
  received `SetChargingProfile` limits to actual charging power**.

This document describes how to exercise and verify both modes.

---

## 2. Enabling the client

| Platform | How to enable | Endpoint per charger |
|----------|---------------|----------------------|
| Native | `OCPP_ENDPOINT=ws://host:port/path cargo run` | `{OCPP_ENDPOINT}/{charger_id}` |
| WASM | append `?ocpp_endpoint=ws://host:port/path` to the page URL | `{ocpp_endpoint}/{charger_id}` |

Charger ids are `chg_01`, `chg_02`, … so with `OCPP_ENDPOINT=ws://localhost:9000/ocpp`
the first charger connects to `ws://localhost:9000/ocpp/chg_01`.

Real-mode inbound handling (answering CSMS Calls, applying charging profiles) is
**native only**. WASM remains send-only.

### Requirements a CSMS must meet

- **Echo the `ocpp1.6` subprotocol** in the WebSocket handshake response. The
  client requests it and (correctly) refuses connections where the server does
  not confirm it. Real brokers/CSMS do this.
- `SetChargingProfile.req` may omit `minChargingRate` (the client tolerates it).

---

## 3. Automated tests

```bash
# Unit tests: composite-limit evaluation + frame parsing
cargo test --lib ocpp

# Integration tests: mock CSMS socket round-trip, SetChargingProfile applied,
# unknown action -> CallError
cargo test --test ocpp_client_test

# Full workflow
cargo fmt --check
cargo clippy --all --benches --tests --examples --all-features
cargo test
```

Key coverage:

- [tests/ocpp_client_test.rs](../tests/ocpp_client_test.rs) — stands up a tiny
  `tokio-tungstenite` mock CSMS, verifies bidirectional traffic, and asserts a
  `SetChargingProfile` is stored, answered `Accepted`, and applied as a
  `Charger::ocpp_limit_kw` cap.
- [src/ocpp/charging_profiles.rs](../src/ocpp/charging_profiles.rs) tests —
  purposes, stack levels, Absolute/Relative/Recurring kinds, A→kW conversion,
  validity windows, and ClearChargingProfile matching.
- [src/ocpp/client.rs](../src/ocpp/client.rs) tests — frame parsing for
  Call / CallResult / CallError and malformed input.

---

## 4. Offline manual test (synthetic mode)

Native runs always mirror OCPP traffic to disk, so you can inspect a full
transcript without any server:

```bash
cargo run
# play for a bit, then look at:
ls ocpp_datastream/
tail -f ocpp_datastream/chg_01.jsonl
```

Each line is a raw OCPP-J frame (`[2,…]` Calls, `[3,…]` CallResults). In
synthetic mode both directions are present because the game fabricates the CSMS
responses.

---

## 5. End-to-end test with a mock CSMS (real mode)

This is the fastest way to see charging profiles throttle power.
[tools/mock_csms.py](../tools/mock_csms.py) is a self-contained CSMS that
negotiates `ocpp1.6`, answers the core Calls, and (once a charger boots) pushes a
`SetChargingProfile` capping the charge point to **20 kW**. It prints every
`MeterValues` so you can watch the reported power drop to the cap.

### 5.1 Run it

```bash
# terminal 1
pip install websockets
python tools/mock_csms.py

# terminal 2
OCPP_ENDPOINT=ws://localhost:9000/ocpp cargo run
```

Open the station, place/enable a DC fast charger, and start a session.

### 5.2 What to verify

1. **Connection + subprotocol** — the CSMS prints `charger connected …
   (subprotocol=ocpp1.6)`. If it connects but the game logs a
   `SubProtocol error`, the server is not echoing `ocpp1.6`.
2. **Boot handshake** — CSMS receives `BootNotification`; the game adopts the
   returned `interval` as its heartbeat cadence.
3. **Transaction id from the CSMS** — on a session start the CSMS receives
   `StartTransaction` and replies `transactionId: 1001`. Subsequent
   `MeterValues`/`StopTransaction` for that charger carry `1001` (not a locally
   generated id). Game log: `OCPP: StartTransaction.conf … txn=1001`.
4. **Profile applied / power capped** — after `SetChargingProfile` the game
   replies `[3,"srv-set-1",{"status":"Accepted"}]` and the charger's power is
   capped to 20 kW. Watch the `MeterValues` the CSMS prints: the
   `Power.Active.Import` sample should not exceed ~20000 W even on a 150 kW
   charger. In game, the charger's throttle indicator appears and delivered kW
   drops.
5. **CSMS-initiated commands** — you can extend the script to send other Calls
   and confirm replies:
   - `Reset` → `{"status":"Accepted"}` and the charger re-sends `BootNotification`.
   - `GetConfiguration` → returns `HeartbeatInterval`, `NumberOfConnectors`, etc.
   - `ClearChargingProfile` → removes the cap; power returns to normal.
   - `GetCompositeSchedule` → returns the current composite schedule.
   - `TriggerMessage` (`Heartbeat`/`StatusNotification`/`MeterValues`) → the game
     emits the requested message promptly.
   - An unsupported action → the game replies with a `[4,…,"NotImplemented",…]`
     CallError.

### Quick profile-cap check with `websocat`

To confirm the cap changes power without the full script, connect the game to
the mock CSMS above, then edit `SET_PROFILE["csChargingProfiles"]
["chargingSchedule"]["chargingSchedulePeriod"][0]["limit"]` to a small value
(e.g. `5000`) and restart the CSMS: reported power should track the new 5 kW cap.

---

## 7. How the profile becomes power (reference)

The composite limit is computed from all stored profiles
([src/ocpp/charging_profiles.rs](../src/ocpp/charging_profiles.rs)) — highest
`stackLevel` wins within a purpose, `TxProfile` overrides `TxDefaultProfile`, and
`ChargePointMaxProfile` caps everything — then written to `Charger::ocpp_limit_kw`.
`power_dispatch_system` ([src/systems/power_dispatch.rs](../src/systems/power_dispatch.rs))
clamps each session's requested power to that cap **before** first-come-first-served
allocation, so a throttled charger frees grid headroom for others.

The cap is a **maximum, not a floor**: grid capacity, transformer thermal
throttle, and cold-weather derating can still deliver less than the profile
allows. A hacker overload attack deliberately bypasses the cap.

Semantics are ported from the EVerest libocpp v1.6 reference implementation;
expected values in the unit tests were cross-checked against its test vectors.
