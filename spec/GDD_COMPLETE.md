# Kilowatt Tycoon — Complete Game Design Document

> *A game that teaches why charging networks are hard — by letting players run one.*

---

## Table of Contents
1. [Overview](#1-overview)
2. [Core Fantasy & Design Values](#2-core-fantasy--design-values)
3. [Target Platforms & Engine](#3-target-platforms--engine)
4. [Core Gameplay Loop](#4-core-gameplay-loop)
5. [Key Systems Overview](#5-key-systems-overview)
6. [Operations & Support Mechanics](#6-operations--support-mechanics)
7. [Grid & Power Simulation](#7-grid--power-simulation)
8. [Economy & Progression](#8-economy--progression)
9. [Ecosystem & Site Types](#9-ecosystem--site-types)
10. [Sandbox & MVP Specification](#10-sandbox--mvp-specification)
11. [UI & Audio Design](#11-ui--audio-design)
12. [Data-Driven Architecture](#12-data-driven-architecture)
13. [Non-Goals](#13-non-goals)
14. [Open Questions & Decisions](#14-open-questions--decisions)
15. [Production Roadmap](#15-production-roadmap)

---

## 1. Overview

**Kilowatt Tycoon** is a 2D top-down simulation/tycoon game where the player operates an EV Charging Point Operator (CPO). The game focuses on infrastructure decisions, uptime, operational complexity, and business tradeoffs rather than city-building.

The player is not a driver or city mayor — they are a **charging network operator**.

**One-line pitch**: *Feel how messy and complicated it is to run an actual EV charging network.*

---

## 2. Core Fantasy & Design Values

### 2.1 Core Fantasy
> *Run a reliable, profitable EV charging network in a world where everything can break.*

Success is defined by:
- High uptime
- Profitable operations
- Happy customers & partners
- Scalable, resilient infrastructure

### 2.2 Design Pillars
1. **Systems over spectacle** — Depth emerges from interacting systems, not flashy events
2. **Tradeoffs over optimal paths** — Every decision has costs; there's no "best" strategy
3. **Failure is normal** — Things break constantly; mastery is managing chaos
4. **Uptime is earned** — Reliability requires investment, attention, and skill

### 2.3 Player Experience Goal
The player should feel like they're **spinning plates** — constantly triaging, prioritizing, and making imperfect decisions under time pressure. The game teaches why charging networks are hard through direct experience, not tutorials.

---

## 3. Target Platforms & Engine

### 3.1 Platforms
- macOS (primary development)
- Windows
- Linux

### 3.2 Engine
- **Bevy 0.17+ (Rust)** — ECS-based game engine
- Additional crates: `bevy_northstar` (pathfinding), JSON serialization

### 3.3 Input
- Mouse/keyboard (primary)
- Touch (future mobile)
- No multiplayer at launch

### 3.4 Visual Style
- **Camera**: Top-down 2D with pan/zoom
- **Assets**: SVG-based generation, PNG rasterized for runtime
- **Visual style**: Clean, semi-realistic, flat vector style
- **Tone**: Serious systems, playful presentation

---

## 4. Core Gameplay Loop

### 4.1 Primary Loop (Minute-to-Minute)
1. **Observe** — Monitor charger status, driver queues, power load, tickets
2. **Decide** — Prioritize which problems to address first
3. **Act** — Execute remote operations, dispatch technicians, respond to tickets
4. **Outcome** — Problems resolve or escalate; sessions complete or fail
5. **Reward** — Revenue earned, reputation changed, new opportunities unlocked
6. **Repeat** — New drivers arrive, new problems emerge

### 4.2 Failure Loop
When things go wrong:
1. Incident occurs (fault, complaint, overload)
2. Player is notified (priority-sorted)
3. Player chooses response (or ignores)
4. Consequences manifest (revenue loss, reputation damage, equipment damage)
5. Player learns and adapts strategy

### 4.3 Skill Expression
Mastery shows up in:
- **Planning**: Infrastructure layout, phase balancing, capacity headroom
- **Prioritization**: Triaging multiple simultaneous issues
- **Economy**: Balancing refunds vs reputation vs long-term costs
- **Timing**: Knowing when to invest in upgrades vs run lean

---

## 5. Key Systems Overview

| System | Purpose | Complexity |
|--------|---------|------------|
| **Infrastructure** | Place chargers, manage power | High |
| **Operations & Uptime** | Track health, handle failures | High |
| **Incidents & Tickets** | Respond to problems | Medium |
| **Technicians** | Dispatch for physical repairs | Medium |
| **Economy & Pricing** | Revenue, costs, demand | Medium |
| **Customers & Demand** | Driver behavior, reputation | Medium |
| **Grid & Power** | Voltage, phases, thermal limits | High |

---

## 6. Operations & Support Mechanics

### 6.1 Remote Operations (HUD Chores)
Actions the player can take from the main HUD without dispatching a technician.

| Action | Trigger | Success Rate | Cooldown |
|--------|---------|--------------|----------|
| **Soft Reboot** | Communication Error | 70% | 30 seconds |
| **Hard Reboot** | Soft Reboot failed | 90% | 2 minutes |
| **Release Connector** | Connector Locked | 80% | 10 seconds |
| **Refund Session** | Billing complaint | 100% | Instant |
| **Disable/Enable Charger** | Player choice | 95–100% | Instant/5s |

### 6.2 Physical Tasks (Technician Work)
Problems that require on-site intervention.

| Task | Base Duration | Requirements |
|------|---------------|--------------|
| Cable Untangle | 2 minutes | None |
| Connector Replacement | 15 minutes | Spare part |
| Screen Repair | 20 minutes | Replacement screen |
| Payment Terminal Reset | 5 minutes | None |
| Electrical Inspection | 30 minutes | Multimeter |
| Full Charger Replacement | 60 minutes | Replacement unit |

### 6.3 Technician Model
- **Location**: GPS-tracked, travel time matters
- **Skill Level**: 1–5, affects repair speed and success
- **Fatigue**: 0–100%, accumulates over shifts
- **Hourly Rate**: OpEx cost while active

### 6.4 Customer Support System
Drivers generate **support tickets** when problems occur.

| Ticket Type | SLA Timer | Consequence of Breach |
|-------------|-----------|------------------------|
| Billing Dispute | 5 minutes | Chargeback (2× cost) |
| Session Didn't Start | 3 minutes | 1-star review |
| Connector Stuck | 2 minutes | Reputation −5 |
| Slow Charging | 10 minutes | Partial refund request |
| App Error | 5 minutes | Reputation −2 |

**Resolution Actions**: Acknowledge, Apologize, Partial Refund, Full Refund, Dispatch Technician, Ignore

### 6.5 Priority System
All incidents and tickets have a priority score (0–100):
- Base priority from type
- +10 per minute elapsed
- +20 if driver physically waiting
- +30 if SLA <60 seconds from breach

---

## 7. Grid & Power Simulation

### 7.1 Electrical Model
Each site has:
- **Grid connection** with contracted capacity (kVA)
- **Transformer(s)** stepping down voltage
- **Three-phase distribution** to chargers
- Individual **charger power electronics**

### 7.2 Voltage Drop
When chargers draw power, voltage drops across the distribution system.

| Voltage Level | Effect |
|---------------|--------|
| >95% nominal | Normal operation |
| 90–95% nominal | −10% charging speed |
| 85–90% nominal | −25% speed; warning |
| 80–85% nominal | −50% speed; complaints |
| <80% nominal | Charger trips |

**Mitigation**: Upgrade cables, shorten runs, reduce load, adjust transformer tap

### 7.3 Phase Balancing
Three-phase systems require balanced loading.

| Imbalance | Effect |
|-----------|--------|
| <10% | Normal |
| 10–20% | Efficiency −2% |
| 20–30% | Efficiency −5%; warning |
| 30–50% | Efficiency −10%; trips likely |
| >50% | Phase offline |

**Mitigation**: Deliberate phase assignment, dynamic load balancing (upgrade), three-phase chargers

### 7.4 Inrush Events
Starting a charging session causes momentary current spike (3–5×).

| Scenario | Effect |
|----------|--------|
| 1 charger starts | Normal; brief voltage dip |
| 2 chargers within 5s | Visible sag; other sessions derate |
| 3+ chargers within 5s | 30–60% breaker trip probability |

**Mitigation**: Stagger starts, soft-start chargers (upgrade), capacitor banks

### 7.5 Transformer Thermal Model
Transformers heat up under load.

| Temperature | Effect |
|-------------|--------|
| <70°C | Normal |
| 70–85°C | Warning; accelerated aging |
| 85–100°C | Auto load shedding (−20%) |
| 100–110°C | Emergency; derate 50% |
| >110°C | Thermal trip; 30-minute cooldown |

**Aging**: Operating hot consumes transformer lifetime faster (2×–8× depending on temperature)

### 7.6 Demand Charges
Utilities charge based on **peak 15-minute average power**.

```
monthly_demand_charge = peak_demand_kw × demand_rate_per_kw
```

Exceeding contracted capacity: 2× overage rate + utility warnings

**Mitigation**: Battery storage, dynamic pricing, queue management, scheduled charging

### 7.7 Battery Storage
Optional upgrade for managing grid constraints.

| Mode | Behavior |
|------|----------|
| Peak Shaving | Discharge when approaching capacity |
| Backup | Discharge only during outage |
| Arbitrage | Charge low-price, discharge high-price |
| Manual | Player-controlled |

---

## 8. Economy & Progression

### 8.1 Revenue Model
```
Net Revenue = Session Revenue − Refunds − Penalties − OpEx
```

| Source | Rate |
|--------|------|
| DC Fast Charging | $0.40/kWh |
| AC Level 2 | $0.25/kWh |

### 8.2 Costs

| Cost Type | Rate |
|-----------|------|
| Technician | $30/hour |
| Electricity | $0.12/kWh |
| Demand charge | $15/kW peak |
| SLA breach | $25 + reputation |
| Chargeback | 2× session value |

### 8.3 Reputation System
Reputation (0–100) affects:
- Customer demand
- Contract opportunities
- Expansion options

| Reputation | Effect |
|------------|--------|
| 80–100 | Premium partners; +20% demand |
| 50–79 | Normal operations |
| 30–49 | −30% demand |
| <30 | Churn; contracts canceled |

### 8.4 Progression Structure

| Phase | Characteristics |
|-------|-----------------|
| **Early Game** | One site; manual responses; frequent failures |
| **Mid Game** | Multiple sites; automation unlocks; staffing strategy |
| **Late Game** | Network optimization; grid events; regulation |

---

## 9. Ecosystem & Site Types

### 9.1 Site Archetypes (Implemented)

| Type | Demand Pattern | Primary Challenge |
|------|----------------|-------------------|
| **ParkingLot** | Balanced | Learning the ropes (starter site) |
| **GasStation** | High traffic, compact | Throughput under power constraints |
| **FleetDepot** | Shift-change spikes | Scale and power management (endgame) |

See [EXPANSION_SITES_VEHICLES.md](EXPANSION_SITES_VEHICLES.md) for planned expansion archetypes (Pier, Airport, Bus Depot, Truck Stop, Apartment).

### 9.1.1 Charger Hardware

Five charger pad types: `L2` (22 kW), `DCFC50`, `DCFC100` (with ad screen), `DCFC150`, `DCFC350`.

Three charger tiers: `Value`, `Standard`, `Premium` — affecting MTBF, efficiency, and connector reliability.

### 9.1.2 Vehicle Types

10 generic vehicle types: `Compact`, `Sedan`, `Suv`, `Crossover`, `Pickup`, `Bus`, `Semi`, `Tractor`, `Scooter`, `Motorcycle`. Each has a display name (e.g., "Tesla Model 3") for flavor.

### 9.2 External Threats

| Threat | Impact | Status |
|--------|--------|--------|
| Cable Theft (Robber system) | Cable damage, replacement cost | Implemented |
| Weather (Sunny/Overcast/Rainy/Heatwave/Cold) | Failure rates, demand, patience | Implemented |
| Grid Events | Outages, voltage instability | Not yet implemented |

### 9.3 Energy Upgrades

| Upgrade | Benefit |
|---------|---------|
| Solar | Reduced energy costs; resilience |
| Battery | Peak shaving; backup power |
| Transformer upgrade | More headroom |

---

## 10. Sandbox & MVP Specification

### 10.1 MVP Success Criteria
Player should:
1. Start a session (10 seconds)
2. Understand the goal (30 seconds)
3. Experience a problem (90 seconds)
4. Make a meaningful decision (2 minutes)

### 10.2 Starting State

| Resource | Value |
|----------|-------|
| Cash | $1,000,000 |
| Reputation | 50/100 |
| Technicians | 1 (skill level 2) |

**Site**: "First Street Station" — ParkingLot archetype, 1500 kVA capacity

**Chargers**:
- CHG-01: 50kW DC, Phase A, 85% health
- CHG-02: 50kW DC, Phase A, 70% health (pre-seeded fault)
- CHG-03: 22kW AC, Phase B, 90% health
- CHG-04: 22kW AC, Phase C, 60% health (sticky connector)

### 10.3 Revenue Target
Earn **$100 net revenue** per scenario (revenue target configurable in scenario data).

### 10.4 Time Scale

The game uses a day-based model (86,400 game-seconds per day):

| Speed | Internal Multiplier | Real Seconds per Day | Display Label |
|-------|--------------------|--------------------|---------------|
| Paused | 0x | — | Paused |
| Normal | 1440x | 60s | "1x" |
| Fast (default) | 2880x | 30s | "10x" |

> Auto-pause triggers are planned but **not yet implemented**.

### 10.5 Driver Schedule
10 scripted drivers arrive over the scenario with varying patience levels (VeryLow to High). Vehicles use generic `VehicleType` categories with display names for flavor.

### 10.6 Guaranteed Events

| Time | Event |
|------|-------|
| 0:30 | CHG-02 Communication Error |
| 1:00 | CHG-04 connector jam |
| 2:00 | Transformer 75°C warning |
| 2:30 | CHG-02 fault recurs |
| 3:00 | Phase A overload |
| 3:30 | First billing dispute |
| 4:00 | Second charger fault |

### 10.7 Win/Lose Conditions

> **Status: NOT YET IMPLEMENTED** — The `win_lose_system` is a no-op stub. The game runs day-by-day without triggering game over. The `GameResult` enum exists with the intended variants but is never checked.

| Condition | Outcome | Status |
|-----------|---------|--------|
| Net revenue ≥ target | Won | Planned |
| Cash < $0 | LostBankruptcy | Planned |
| Reputation < threshold | LostReputation | Planned |
| Days elapsed without target | LostTimeout | Planned |

---

## 11. UI & Audio Design

### 11.1 HUD Layout

```
┌─────────────────────────────────────────────────────────────────┐
│ [Cash: $XXX] [Revenue: $XXX/$100] [Time: X:XX] [Reputation: XX] │
├─────────────────────────────────────────────────────────────────┤
│                                                                 │
│                       [ SITE MAP VIEW ]                         │
│                                                                 │
│   [CHG-01]    [CHG-02]    [CHG-03]    [CHG-04]                  │
│   ● Active    ⚠ Fault     ○ Idle     ● Active                  │
│                                                                 │
├─────────────────────────────────────────────────────────────────┤
│ [Power: 85/100 kVA] [Phase A: ██] [Phase B: █] [Phase C: █]     │
│ [Transformer: 72°C ⚠]                                           │
├─────────────────────────────────────────────────────────────────┤
│ [TICKETS]                              [SPEED: ▶▶] [⏸ Pause]    │
└─────────────────────────────────────────────────────────────────┘
```

### 11.2 Driver Visual States

| State | Indicator |
|-------|-----------|
| Waiting (happy) | Green outline |
| Waiting (impatient) | Yellow outline, tapping |
| Waiting (frustrated) | Orange, angry face |
| Waiting (about to leave) | Red, pulsing |
| Charging | Blue, phone animation |
| Complete (happy) | Green smiley, "+$X" |
| Leaving (angry) | Red, "−3 rep" |

### 11.3 Audio Events

| Event | Sound |
|-------|-------|
| Session start | Plug-in click |
| Session complete | Cash register |
| Fault | Warning buzzer |
| Ticket | Phone notification |
| SLA breach imminent | Urgent alarm |
| Driver leaves angry | Door slam + screech |
| Win | Victory fanfare |
| Lose | Power-down sound |

---

## 12. Data-Driven Architecture

### 12.1 Philosophy
All gameplay content is configurable via data files:
- Charger types and specs
- Site templates and layouts
- Driver schedules and behavior

### 12.2 Formats
- **JSON** for configuration and scenarios
- **Rust structs** with Serde for runtime deserialization

### 12.3 Key Data Files

| File | Purpose |
|------|---------|
| `data/sites/templates/*.json` | 12 site templates (parking_lot, gas_station, mall, etc.) |
| `data/chargers/*.json` | Charger specs |
| `data/scenarios/*.json` | Driver schedules, scripted events |

### 12.4 Asset Generation
Assets are generated via Python tools:
- `tools/generate_assets.py` — Entry point
- `tools/asset_generation/` — SVG generators for tiles, props, chargers
- Output: SVG source + PNG rasterized in `assets/`

---

## 13. Non-Goals

The following are explicitly **out of scope**:
- Real-time driving simulation
- City-scale simulation
- Multiplayer at launch
- Complex AI opponents
- Story-driven narrative

---

## 14. Open Questions & Decisions

### 14.1 Resolved (Assumptions for MVP)

| Question | Assumption |
|----------|------------|
| Power factor | Real power only; pf = 0.95 constant |
| Technician pathfinding | Abstract timer with ETA |
| Technician shifts | Available 24/7; fatigue accumulates |
| Multiple technicians per site | Yes |
| Driver visibility | Yes, 2D sprite with emotion indicator |
| Game speed default | Fast (10×) |
| Session start | Drivers self-serve |
| Charger interaction | Click to select, action buttons in panel |

### 14.2 Open (Decide Before Beta)

| Question | Options |
|----------|---------|
| Weather impact on transformer | Model temp-dependent ratings? |
| Harmonics | Model power quality issues? |
| Multi-site grid interaction | Sites affect each other? |
| Technician minigames | Include for depth? |
| On-site character control | Direct control or dispatch-only? |

---

## 15. Production Roadmap

### 15.1 Milestones

| Milestone | Definition | Target |
|-----------|------------|--------|
| **Prototype** | Core loop playable; ugly but functional | 4 weeks |
| **Vertical Slice** | MVP spec complete; one polished site | 8 weeks |
| **Alpha** | All systems functional; 3 sites; basic progression | 16 weeks |
| **Beta** | Full content; balancing pass; mobile build | 24 weeks |
| **Release** | Polished; tested; App Store ready | 32 weeks |

### 15.2 MVP Feature List (Vertical Slice)

| Feature | Priority |
|---------|----------|
| Site map view with chargers | P0 |
| Driver arrival and charging | P0 |
| Remote operations (reboot, release) | P0 |
| Technician dispatch | P0 |
| Ticket system | P0 |
| Power gauge and phase display | P0 |
| Transformer thermal | P0 |
| Revenue tracking and win condition | P0 |
| Pause and speed controls | P0 |
| Tutorial tooltips | P1 |
| Audio feedback | P1 |
| Save/load | P2 |

### 15.3 Risk Register

| Risk | Likelihood | Impact | Mitigation |
|------|------------|--------|------------|
| Electrical sim too complex | Medium | High | Simplify; expose depth gradually |
| UX overwhelming | High | High | Aggressive prioritization; good tutorials |
| Performance on mobile | Medium | Medium | Profile early; optimize hot paths |
| Balancing (too hard/easy) | High | Medium | Extensive playtesting; data-driven tuning |
| Scope creep | High | High | Strict MVP definition; say no |

---

## Appendix A: Related Documents

### Core Documentation

| Document | Purpose |
|----------|---------|
| [ARCHITECTURE.md](ARCHITECTURE.md) | Technical architecture and module structure |
| [ECOSYSTEM.md](ECOSYSTEM.md) | Game world and environmental systems |
| [SPEC_OPERATIONS.md](SPEC_OPERATIONS.md) | Operations & support mechanics |
| [SPEC_GRID_POWER.md](SPEC_GRID_POWER.md) | Electrical simulation and power economics |
| [SPEC_DEMAND_CHARGES.md](SPEC_DEMAND_CHARGES.md) | Demand charge UX and mechanics |
| [SPEC_SANDBOX_MVP.md](SPEC_SANDBOX_MVP.md) | MVP session design |
| [SPEC_SYSTEMS.md](SPEC_SYSTEMS.md) | Emotions, traffic, pathfinding |

### Asset & Style Documentation

| Document | Purpose |
|----------|---------|
| [STYLE_GUIDE.md](STYLE_GUIDE.md) | Art style guide |
| [MVP_ASSET_SCOPE.md](MVP_ASSET_SCOPE.md) | MVP asset requirements and use-case mapping |

### Future Plans

| Document | Purpose |
|----------|---------|
| [EXPANSION_SITES_VEHICLES.md](EXPANSION_SITES_VEHICLES.md) | Expansion content plans |

---

## Appendix B: Glossary

| Term | Definition |
|------|------------|
| **CPO** | Charge Point Operator — the player's role |
| **SLA** | Service Level Agreement — time limit to resolve issues |
| **kVA** | Kilovolt-ampere — unit of apparent power |
| **Derate** | Reduce power delivery below rated capacity |
| **Inrush** | Momentary current spike when equipment starts |
| **OCPP** | Open Charge Point Protocol — communication standard |
| **Phase** | One of three AC power lines in three-phase systems |

---

## Appendix C: Design Rationale — Demand Charges as Fun Factor

### The Power Economics Puzzle

Where classic tycoon games center strategic depth on recipe optimization or supply chains, Kilowatt Tycoon introduces **demand charge management** as its core skill expression vector. Players want to serve customers quickly (high power throughput = happy customers) but each power peak permanently increases operational costs. This tension between short-term customer satisfaction and long-term profit optimization forms the psychological hook.

### The Invisible Consequence Problem

Demand charge mechanics fail when they violate a fundamental principle: **feedback loops must be immediate and visible**. When costs appear only at end-of-day, players experience the mechanic as a "gotcha" punishment rather than a strategic challenge. The solution: transform demand charges from an invisible tax into a visible, manageable risk.

### The "Aha Moment" Architecture

The demand charge system achieves engagement through a carefully constructed progression:

- **Novice Phase (Recognition)**: The first "new peak" warning teaches that actions have permanent cost consequences.
- **Intermediate Phase (Automation Discovery)**: Purchasing a BESS reveals automated peak shaving. The UI displaying "PEAK SHAVING - Preventing +$750 charge" transforms the battery from passive component into heroic protector.
- **Advanced Phase (Strategic Optimization)**: Players learn to manipulate the 15-minute rolling average — charging BESS off-peak, running high power during rush hours, staggering session starts, manually reducing power when BESS SOC is low.

### Progressive Disclosure of Complexity

1. **Surface Layer (Novice)**: "Stay in green zone" — simple visual feedback
2. **Mechanical Layer (Intermediate)**: "BESS protects at 85% threshold" — understanding automation
3. **Strategic Layer (Advanced)**: "15-minute rolling window manipulation" — exploiting mechanics
4. **Mastery Layer (Expert)**: "BESS threshold tuning per site archetype" — meta-optimization

### Risk-Reward Tension

| Strategy | Revenue Impact | Risk Level | Skill Required |
|----------|---------------|------------|----------------|
| **Conservative** (Low power density) | -30% throughput | Zero risk | None |
| **Balanced** (100% with BESS) | Baseline | Low risk | Medium |
| **Aggressive** (120% power density) | +20% throughput | High risk | Expert |

### Failure States as Learning Moments

Educational failure states provide **causal clarity** immediately:

- Peak meter turns red during load spike
- Toast: "NEW PEAK SET - 520 kW -> +$300 charge"
- End-of-day: "Peak set at 2:45 PM (4 simultaneous charges)"
- Player learns: "I shouldn't charge everyone at once"

### BESS as Personality, Not Equipment

The emotional connection to the BESS is cultivated through anthropomorphization — states have personality (Standby as sleeping guardian, Peak Shaving as active protector), saves are celebrated via toasts, limitations create drama through low SOC warnings, and statistics create narrative ("Your battery saved $15,000 this week!").

### Design Principles

1. **Visible Consequences**: Real-time feedback transforms abstract economics into tangible gameplay
2. **Gradual Mastery**: Layered complexity provides a continuous skill ceiling
3. **Emotional Narrative**: BESS becomes a character, not just a mechanic
4. **Strategic Tradeoffs**: No dominant strategy; all approaches have merit
5. **Emergent Depth**: Simple rules create complex optimization space

---

*Document version: 1.2 — February 2026*
*Engine: Bevy 0.17+ (Rust)*
*Last updated: Merged demand charge design rationale; cleaned up cross-references*

