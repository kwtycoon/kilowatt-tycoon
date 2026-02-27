# Kilowatt Tycoon — Sandbox & MVP Specification

## 1. Purpose
This document defines the **initial sandbox parameters and MVP session design**. The goal: get the player into chaos quickly, teach through failure, and demonstrate the core loop.

> **Note**: The game uses a **day-by-day model** where each day is one game-day (86,400 game-seconds). The original 5-minute session concept has been replaced with a persistent multi-day experience.

---

## 2. MVP Success Criteria
A player who has never seen the game should be able to:
1. **Start a session** within 10 seconds of loading
2. **Understand the goal** (earn money) within 30 seconds
3. **Experience a problem** (failure, complaint, or capacity issue) within 90 seconds
4. **Make a meaningful decision** (fix, refund, or ignore) within 2 minutes
5. **See the revenue** and track progress throughout

---

## 3. Sandbox Start State

### 3.1 Initial Resources

| Resource | Starting Value | Notes |
|----------|----------------|-------|
| **Cash** | $1,000,000 | Enough for a major charging hub |
| **Reputation** | 50/100 | Neutral; room to rise or fall |
| **Technicians** | 1 | Skill level 2, stationed on-site |

### 3.2 Initial Site: "First Street Station"
A small retail charging site with constrained infrastructure.

| Parameter | Value |
|-----------|-------|
| **Site Type** | ParkingLot |
| **Contracted Capacity** | 100 kVA |
| **Transformer** | 75 kVA (already slightly undersized) |
| **Grid Voltage** | 400V (3-phase) |
| **Phases** | 3, unbalanced initial assignment |

### 3.3 Initial Equipment

| Charger ID | Type | Rated Power | Phase | Condition | Notes |
|------------|------|-------------|-------|-----------|-------|
| **CHG-01** | DC Fast | 50 kW | A | 85% health | Reliable |
| **CHG-02** | DC Fast | 50 kW | A | 70% health | Prone to faults |
| **CHG-03** | AC Level 2 | 22 kW | B | 90% health | Reliable |
| **CHG-04** | AC Level 2 | 22 kW | C | 60% health | Frequent connector issues |

**Note**: Two 50kW chargers on the same phase (A) creates immediate imbalance risk when both are active.

### 3.4 Pre-Seeded Issues
To ensure the player sees action quickly, the following conditions exist at game start:

| Issue | State | Time to Manifest |
|-------|-------|------------------|
| CHG-02 has a pending software fault | Hidden | Triggers on first session (30s in) |
| CHG-04 connector is slightly sticky | Hidden | 40% chance to jam on session end |
| Driver already queued at CHG-01 | Active | Session starts immediately |
| Second driver arriving | En route | Arrives at 0:20 |
| Third driver arriving | En route | Arrives at 0:45 |

---

## 4. Revenue Target

### 4.1 Goal
Earn **$100 in net revenue** (revenue target configurable per scenario).

### 4.2 Revenue Calculation
```
Net Revenue = Session Revenue − Refunds − Penalties − OpEx
```

### 4.3 Revenue Sources

| Source | Rate | Expected Sessions (5 min) |
|--------|------|---------------------------|
| DC Fast Charging (50 kW) | $0.40/kWh | 3–4 sessions |
| AC Level 2 Charging (22 kW) | $0.25/kWh | 2–3 sessions |

### 4.4 Expected Session Economics

| Session Type | Duration | Energy | Revenue |
|--------------|----------|--------|---------|
| DC Fast (80% SoC target) | 20–30 min | 25–40 kWh | $10–16 |
| DC Fast (quick top-up) | 5–10 min | 8–15 kWh | $3–6 |
| AC Level 2 (1-hour retail stop) | 30–60 min | 11–22 kWh | $3–5 |

**Note**: At Normal speed (1440x), one game-day takes 60 real seconds. At Fast speed (2880x), one game-day takes 30 real seconds.

### 4.5 Target Breakdown
To reach $100:
- ~6–8 successful charging sessions
- Minimal refunds (<$20 total)
- No major incidents causing prolonged downtime

---

## 5. Time Scale

### 5.1 Day-Based Model
The game operates on a day-by-day cycle. Each game day is 86,400 game-seconds.

### 5.2 Speed Controls

| Speed | Internal Multiplier | Real Seconds per Day | Display Label |
|-------|--------------------|--------------------|---------------|
| Paused | 0x | — | Paused |
| Normal | 1440x | 60s | "1x" |
| Fast (default) | 2880x | 30s | "10x" |

> **Note**: Display labels ("1x", "10x") are shorthand and don't reflect the actual multiplier over real-time. "Normal" compresses 24 hours into 60 real seconds.

### 5.3 Auto-Pause Triggers

> **Status: NOT YET IMPLEMENTED** — Auto-pause triggers are planned but not active. The `GameState` tracks `first_fault_seen`, `first_ticket_seen`, `first_session_completed` booleans for future use.

---

## 6. Driver Behavior & Demand

### 6.1 Driver Arrival Pattern
Drivers arrive in a **scripted wave** to ensure consistent experience:

| Time (game sec) | Driver ID | Vehicle Type | Vehicle Name | Target Charger | Patience | Charge (kWh) |
|-----------------|-----------|-------------|--------------|----------------|----------|--------------|
| 0 | DRV-01 | Sedan | Tesla Model 3 | CHG-01 (DC) | High | 45 |
| 20 | DRV-02 | Pickup | Rivian R1T | CHG-02 (DC) | Medium | 60 |
| 45 | DRV-03 | Compact | Chevy Bolt | CHG-03 (AC) | High | 30 |
| 90 | DRV-04 | Pickup | Ford F-150 Lightning | CHG-01 (DC) | Low | 80 |
| 120 | DRV-05 | Suv | Hyundai Ioniq 5 | CHG-02 (DC) | Medium | 50 |
| 150 | DRV-06 | Crossover | VW ID.4 | CHG-04 (AC) | High | 35 |
| 180 | DRV-07 | Sedan | Porsche Taycan | CHG-01 (DC) | VeryLow | 55 |
| 210 | DRV-08 | Suv | Kia EV6 | Any Available | Medium | 40 |
| 240 | DRV-09 | Sedan | Mercedes EQS | Any Available | Low | 70 |
| 270 | DRV-10 | Suv | BMW iX | Any Available | Medium | 45 |

> **Note**: `vehicle` uses generic `VehicleType` enum values (`Sedan`, `Pickup`, `Compact`, `Suv`, `Crossover`). `vehicle_name` is a display string.

### 6.2 Driver Patience System
Each driver has a **patience meter** (0–100) that depletes while waiting.

| Patience Level | Initial Value | Depletion Rate | Effective Wait |
|----------------|--------------|----------------|----------------|
| VeryLow | 25 | 20/min | ~1.25 min |
| Low | 50 | 15/min | ~3.3 min |
| Medium | 75 | 10/min | ~7.5 min |
| High | 100 | 5/min | ~20 min |

### 6.3 Driver States

```
[Arriving] → [WaitingForCharger] → [Queued] → [Charging] → [Complete] → [DepartingHappy]
                    ↓                                                        ↓
              [Frustrated] → [LeftAngry]                              [Leaving]
                    ↓                                                        ↓
              [DepartingAngry]                                         (exit)
```

### 6.4 Driver Wait Thresholds

| Patience % | Behavior |
|------------|----------|
| 100–75% | Neutral; no indicator |
| 75–50% | Visible impatience (pacing animation, "..." speech bubble) |
| 50–25% | Frustrated (angry face, "!!" speech bubble) |
| 25–0% | About to leave (red highlight, countdown timer visible) |
| 0% | Leaves; reputation −3; potential 1-star review |

### 6.5 Queue Behavior
If target charger is occupied:
1. Driver joins virtual queue
2. Patience timer begins
3. If another charger becomes free (same type), driver may switch
4. If patience depletes, driver leaves

### 6.6 Driver Sprites & Visual Communication

| State | Visual Indicator |
|-------|------------------|
| Waiting (happy) | Green outline, neutral face |
| Waiting (impatient) | Yellow outline, tapping foot |
| Waiting (frustrated) | Orange outline, angry face |
| Waiting (about to leave) | Red outline, pulsing |
| Charging | Blue outline, phone-in-hand animation |
| Complete (happy) | Green smiley, "+$X" floating text |
| Complete (unhappy) | Yellow face, no floating text |
| Leaving (angry) | Red angry face, "−3 rep" floating text |

---

## 7. Failure Injection (Ensuring Chaos)

### 7.1 Guaranteed Events in MVP Session
These events are scripted in `mvp_drivers.scenario.json` to ensure the player experiences core systems:

| Time (game sec) | Event | Expected Player Action |
|-----------------|-------|------------------------|
| 30 | CHG-02 shows "Communication Error" | Soft Reboot (remote) |
| 120 | Transformer reaches 75°C warning | Observe; consider limiting new sessions |
| 150 | CHG-02 "Communication Error" recurs | Hard Reboot or dispatch technician |
| 180 | Phase A overload check | Observe derated charging; consider queue management |
| 210 | First billing dispute ticket | Acknowledge, apologize, or refund |

> **Note**: Two originally planned events are not yet in the data file: CHG-04 connector jam at t=60, and a random second fault at t=240.

### 7.2 Random Event Pool (Post-MVP)
After scripted events, random events can occur:

| Event | Probability/minute | Severity |
|-------|---------------------|----------|
| Connector jam | 5% per session end | Low |
| Payment error | 3% per session start | Low |
| Communication fault | 2% per charger | Medium |
| Ground fault | 0.5% per charger | High |
| Vandalism attempt | 0.2% | High |

---

## 8. Economy Constraints (Forcing Tradeoffs)

### 8.1 Starting Cash: $1,000,000
This is intentionally tight:
- Full refund costs $10–16
- Technician callout costs $50/hour (pro-rated)
- If player refunds everything, they run out of money
- If player ignores problems, reputation tanks

### 8.2 OpEx During Session

| Cost | Rate |
|------|------|
| Technician (on-site) | $30/hour (prorated per game-minute) |
| Electricity (grid) | $0.12/kWh |
| Demand charge | Calculated at session end |

### 8.3 Penalties

| Penalty | Cost |
|---------|------|
| SLA breach (ticket) | −$25 + reputation −5 |
| Chargeback | −2× session value |
| Reputation <30 | Demand drops 30% |

---

## 9. Win/Lose Conditions

> **Status: NOT YET IMPLEMENTED** — The `win_lose_system` is a no-op stub. The game operates day-by-day without explicit win/lose conditions. Players can go into debt or lose reputation without triggering game over.

### 9.1 Planned Win Condition
- Net revenue reaches target (defined per scenario)
- Display: Celebration screen with stats

### 9.2 Planned Lose Conditions
1. **Bankruptcy** (`LostBankruptcy`): Cash drops below $0
2. **Reputation Collapse** (`LostReputation`): Reputation drops below threshold
3. **Timeout** (`LostTimeout`): Days elapsed without meeting revenue target

The `GameResult` enum with these variants exists but is never checked or set.

### 9.3 Current Behavior
The game transitions `Playing -> DayEnd -> Playing` each day cycle. Players see an end-of-day summary with revenue, costs, and demand charges.

---

## 10. Tutorial Overlay (Just-In-Time)

### 10.1 Philosophy
No upfront tutorial. Tooltips appear **when relevant events occur**.

### 10.2 Tooltip Triggers

| Trigger | Tooltip Content |
|---------|-----------------|
| First driver arrives | "A driver is waiting! Click the charger to start their session." |
| First fault appears | "Something's wrong! Click the charger to see available fixes." |
| First ticket appears | "A driver has a complaint. Click to respond before they escalate." |
| Transformer warning | "Your transformer is getting hot. Too much load?" |
| First session completes | "Ka-ching! $X earned. Keep the chargers running to hit your target." |
| Cash drops below $100 | "Running low on cash. Be careful with refunds!" |
| Reputation drops below 40 | "Drivers are getting frustrated. Faster fixes = happier customers." |

---

## 11. HUD Layout for MVP

```
┌─────────────────────────────────────────────────────────────────┐
│ [Cash: $XXX] [Revenue Target: $XXX/$100] [Time: X:XX] [Rep: XX] │
├─────────────────────────────────────────────────────────────────┤
│                                                                 │
│                     [ SITE MAP VIEW ]                           │
│                                                                 │
│   [CHG-01]    [CHG-02]    [CHG-03]    [CHG-04]                  │
│   ● Active    ⚠ Fault     ○ Idle     ● Active                  │
│                                                                 │
│   [Driver]    [Driver]                [Driver]                  │
│                                                                 │
├─────────────────────────────────────────────────────────────────┤
│ [Power: 85/100 kVA] [Phase A: ██] [Phase B: █] [Phase C: █]     │
│ [Transformer: 72°C ⚠]                                           │
├─────────────────────────────────────────────────────────────────┤
│ [TICKETS]                              [SPEED: ▶▶] [⏸ Pause]    │
│ • Billing dispute (SLA: 2:30)                                   │
└─────────────────────────────────────────────────────────────────┘
```

---

## 12. Audio Design (MVP)

### 12.1 Ambient
- Electrical hum (volume scales with load)
- Occasional car sounds (arrival, departure)

### 12.2 Event Sounds

| Event | Sound |
|-------|-------|
| Session start | Satisfying "plug-in" click |
| Session complete | Cash register "cha-ching" |
| Fault occurs | Warning buzzer (short) |
| Ticket arrives | Phone notification sound |
| SLA about to breach | Urgent alarm (escalating) |
| Driver leaves angry | Car door slam + screech |
| Revenue milestone | Uplifting chime |
| Win | Victory fanfare |
| Lose | Sad trombone / power-down sound |

---

## 13. Data Files for MVP

### 13.1 Site Definition
Site definitions are stored as **Tiled TMX maps** (not JSON):
- `assets/maps/01_first_street.tmx`
- `assets/maps/02_quick_charge_express.tmx`
- `assets/maps/03_central_fleet_plaza.tmx`

TMX map properties contain site configuration (capacity, popularity, rent cost, etc.). Example from original design:
```json
{
  "id": "first_street_station",
  "name": "First Street Station",
  "type": "retail",
  "contracted_capacity_kva": 100,
  "transformer": {
    "type": "small_75kva",
    "rating_kva": 75,
    "thermal_limit_c": 110
  },
  "grid_voltage": 400,
  "chargers": ["chg_01", "chg_02", "chg_03", "chg_04"]
}
```

### 13.2 Charger Definitions
`assets/data/chargers/mvp_chargers.chargers.json`
```json
[
  {
    "id": "chg_01",
    "type": "dc_fast",
    "rated_power_kw": 50,
    "phase": "A",
    "health": 0.85,
    "position": {"x": 200, "y": 300}
  },
  {
    "id": "chg_02",
    "type": "dc_fast",
    "rated_power_kw": 50,
    "phase": "A",
    "health": 0.70,
    "position": {"x": 350, "y": 300},
    "scripted_fault": {"type": "communication_error", "trigger_time": 30}
  },
  {
    "id": "chg_03",
    "type": "ac_level2",
    "rated_power_kw": 22,
    "phase": "B",
    "health": 0.90,
    "position": {"x": 500, "y": 300}
  },
  {
    "id": "chg_04",
    "type": "ac_level2",
    "rated_power_kw": 22,
    "phase": "C",
    "health": 0.60,
    "position": {"x": 650, "y": 300},
    "connector_jam_chance": 0.4
  }
]
```

### 13.3 Driver Schedule
`assets/data/scenarios/mvp_drivers.scenario.json`

The actual data includes additional fields not shown in this abbreviated schema: `vehicle_name` (display string), `charge_needed_kwh`, and `notes`.
```json
{
  "drivers": [
    {"id": "drv_01", "vehicle": "Tesla Model 3", "arrival_time": 0, "target_charger": "chg_01", "patience": "high"},
    {"id": "drv_02", "vehicle": "Rivian R1T", "arrival_time": 20, "target_charger": "chg_02", "patience": "medium"},
    {"id": "drv_03", "vehicle": "Chevy Bolt", "arrival_time": 45, "target_charger": "chg_03", "patience": "high"},
    {"id": "drv_04", "vehicle": "Ford F-150 Lightning", "arrival_time": 90, "target_charger": "chg_01", "patience": "low"},
    {"id": "drv_05", "vehicle": "Hyundai Ioniq 5", "arrival_time": 120, "target_charger": "chg_02", "patience": "medium"},
    {"id": "drv_06", "vehicle": "VW ID.4", "arrival_time": 150, "target_charger": "chg_04", "patience": "high"},
    {"id": "drv_07", "vehicle": "Porsche Taycan", "arrival_time": 180, "target_charger": "chg_01", "patience": "very_low"},
    {"id": "drv_08", "vehicle": "Kia EV6", "arrival_time": 210, "target_charger": null, "patience": "medium"},
    {"id": "drv_09", "vehicle": "Mercedes EQS", "arrival_time": 240, "target_charger": null, "patience": "low"},
    {"id": "drv_10", "vehicle": "BMW iX", "arrival_time": 270, "target_charger": null, "patience": "medium"}
  ]
}
```

---

## 14. Open Questions

1. **Game speed default**: Should "Fast" (10×) be the default, or should players opt in?
   - *Assumption for MVP*: Fast (10×) is default; players can slow down.

2. **Technician always on-site**: Or should they start nearby and travel in?
   - *Assumption for MVP*: On-site, to reduce first-session friction.

3. **Charger interaction**: Click to open panel, or hover-to-preview + click-to-act?
   - *Assumption for MVP*: Click to select charger, action buttons appear in panel.

4. **Session auto-start**: Do players click to start sessions, or do drivers self-serve?
   - *Assumption for MVP*: Drivers self-serve (realistic); player intervenes only on problems.

---

## 15. Summary
This spec defines a **tight, curated experience**:
- Scripted driver arrivals ensure consistent action
- Pre-seeded faults guarantee the player sees core systems
- Constrained resources force meaningful tradeoffs
- Day-by-day progression with end-of-day summaries

The sandbox is designed to be **replayable** with different strategies and **expandable** as new sites and mechanics are added.

> **Note on 5-minute session**: The original design was a 5-minute real-time session at 10x speed. The current implementation uses a persistent day model where each day takes 30-60 real seconds. Win/lose conditions are planned but not yet active.

