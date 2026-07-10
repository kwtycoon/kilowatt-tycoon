#!/usr/bin/env python3
"""Minimal OCPP 1.6J mock CSMS for testing the Kilowatt Tycoon charge point.

Negotiates the `ocpp1.6` subprotocol, answers the core Calls a charge point
sends, and (once a charger boots) pushes a SetChargingProfile that caps the
whole charge point to 20 kW. Every MeterValues is printed so you can watch the
reported power track the cap.

Usage:
    pip install websockets
    python tools/mock_csms.py
    # then, in another terminal:
    OCPP_ENDPOINT=ws://localhost:9000/ocpp cargo run

See spec/OCPP.md for the full testing guide.
"""

import asyncio
import datetime
import json

import websockets


def now_iso():
    return datetime.datetime.now(datetime.timezone.utc).isoformat()


# ChargePointMaxProfile capping the whole charge point to 20 kW.
SET_PROFILE = {
    "connectorId": 0,
    "csChargingProfiles": {
        "chargingProfileId": 1,
        "stackLevel": 0,
        "chargingProfilePurpose": "ChargePointMaxProfile",
        "chargingProfileKind": "Absolute",
        "chargingSchedule": {
            "chargingRateUnit": "W",
            "chargingSchedulePeriod": [
                {"startPeriod": 0, "limit": 20000, "numberPhases": 3}
            ],
        },
    },
}


def respond(action):
    """Return the CallResult payload for a charge-point-initiated Call."""
    if action == "BootNotification":
        return {"currentTime": now_iso(), "interval": 300, "status": "Accepted"}
    if action == "StartTransaction":
        return {"transactionId": 1001, "idTagInfo": {"status": "Accepted"}}
    if action == "Heartbeat":
        return {"currentTime": now_iso()}
    if action == "Authorize":
        return {"idTagInfo": {"status": "Accepted"}}
    if action == "StopTransaction":
        return {"idTagInfo": {"status": "Accepted"}}
    # MeterValues, StatusNotification, DataTransfer, etc. take an empty conf.
    return {}


async def handler(ws):
    print(f"[+] charger connected (subprotocol={ws.subprotocol})")
    profile_sent = False
    async for raw in ws:
        msg = json.loads(raw)
        if msg[0] == 2:  # Call from the charge point
            uid, action, payload = msg[1], msg[2], msg[3]
            print(f"  <- {action} {json.dumps(payload)[:120]}")
            await ws.send(json.dumps([3, uid, respond(action)]))
            # After the first BootNotification, push a charging profile.
            if action == "BootNotification" and not profile_sent:
                profile_sent = True
                await ws.send(
                    json.dumps([2, "srv-set-1", "SetChargingProfile", SET_PROFILE])
                )
                print("  -> SetChargingProfile (cap 20 kW)")
        elif msg[0] == 3:  # CallResult answering one of our Calls
            print(f"  <- CallResult {json.dumps(msg[2])[:120]}")
        elif msg[0] == 4:  # CallError
            print(f"  <- CallError {msg[2:]}")


async def main():
    async with websockets.serve(
        handler, "0.0.0.0", 9000, subprotocols=["ocpp1.6"]
    ):
        print("mock CSMS listening on ws://0.0.0.0:9000/{path}")
        await asyncio.Future()  # run forever


if __name__ == "__main__":
    asyncio.run(main())
