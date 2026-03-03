# Kilowatt Tycoon — Hacker Threat System

This document specifies the cyber-attack threat system, its countermeasures, protocol event signatures, and anomaly detection patterns for researchers.

---

## 1. Overview

The hacker system introduces **cyber threats** as a second axis of risk alongside physical cable theft (robbers). Hackers are visible entities that walk onto the site, execute a cyber-attack against the charging infrastructure, and flee. Two attack types target different parts of the system: **Transformer Overload** disrupts the power system, while **Price Slash** manipulates billing.

An **Infosec upgrade path** (Cyber Firewall → Agentic SOC) lets the player defend against these attacks — mirroring how security cameras and anti-theft cables counter robbers.

All attacks produce observable anomalies across OCPP, OCPI, and OpenADR protocol feeds, enabling researchers to build and evaluate anomaly detection algorithms against labeled datasets.

---

## 2. Attack Types

### 2.1 Transformer Overload ("Burn it down")

| Property | Value |
|----------|-------|
| Duration of hack | 15–45 game-minutes (random) |
| Effect duration | 30 game-minutes after hack completes |
| Mechanism | Forces all chargers to request max rated power; disables Smart Load Shedding |
| Consequence | Transformer temperature spikes rapidly; likely fire if player doesn't intervene |
| Player cost | Transformer fire fine ($5,000), rebuild cost, site downtime, reputation loss (−10) |

**Mechanics:**

1. On success, sets a `HackerOverloadActive` resource with a 30-game-minute timer.
2. While active, `power.rs` skips the Smart Load Shedding throttle curve.
3. While active, `power_dispatch.rs` allocates each charger its full `rated_power_kw` regardless of transformer headroom.
4. The transformer's `update_temperature()` drives toward the thermal limit under the artificial 100%+ load ratio.
5. If temperature exceeds the critical threshold for 30 seconds, a transformer fire ignites (existing fire system).

### 2.2 Price Slash ("Power to the people!")

| Property | Value |
|----------|-------|
| Duration of hack | 15–45 game-minutes (random) |
| Effect duration | 30 game-minutes after hack completes |
| Mechanism | Overrides `effective_price()` to return $0.01/kWh |
| Consequence | Revenue drops to near-zero; demand spikes from cheap power |
| Player cost | Lost revenue for the override period |

**Mechanics:**

1. On success, sets a `HackerPriceOverride` resource: `override_price = 0.01`, `remaining_secs = 1800.0`.
2. `DynamicPricingConfig::effective_price()` checks for an active override before computing the normal price. When active, it returns the override value ($0.01).
3. This propagates through **two critical paths**:
   - **Game revenue** (`driver.rs`): `revenue = charge_received_kwh * effective_price()` — the player's actual income tanks.
   - **OCPI CDR billing** (`ocpi/message_gen.rs`): `total_cost = kwh * effective_price()` — the billing record sent to roaming partners shows the slashed price.

**Worked example:**

| Metric | Normal | Hacked |
|--------|--------|--------|
| Energy delivered | 50 kWh | 50 kWh |
| Price per kWh | $0.45 | $0.01 |
| Session revenue | $22.50 | $0.50 |
| CDR `total_cost` | $22.50 | $0.50 |
| CDR cost/energy ratio | 0.45 | 0.01 |

---

## 3. Hacker Entity

### 3.1 Phases

| Phase | Description |
|-------|-------------|
| `Infiltrating` | Walking from random map edge toward the transformer (or nearest charger) |
| `Hacking` | Stationary at target, countdown timer ticking, VFX active |
| `Fleeing` | Walking to a random exit edge after attack completes or fails |
| `Gone` | Off-screen, ready for entity cleanup |

### 3.2 Movement

Hackers walk in a straight line (no pathfinding), identical to robbers. Speed: 80 pixels/second with the same walk animation (bob + sway + flip).

### 3.3 Visual Variants

| Variant | Description |
|---------|-------------|
| `Green` | Green hoodie (reuses robber sprite with green tint initially) |
| `Purple` | Purple hoodie (reuses robber sprite with purple tint initially) |

### 3.4 Name Pool

Names are randomly assigned on spawn:

- "Zero Cool"
- "Crash Override"
- "The Phantom"
- "Script Kiddie"
- "Root Access"
- "Pwn3d"

### 3.5 Speech Bubbles

| Attack type | On success | On failure |
|-------------|------------|------------|
| Transformer Overload | "BURN IT DOWN" (orange) | "FIREWALLED" (red) |
| Price Slash | "POWER TO THE PEOPLE" (green) | "ACCESS DENIED" (red) |

---

## 4. Spawn Probability Model

The spawn model mirrors the robber system with different base rates.

### 4.1 Formula

```
effective_chance = BASE_HACK_CHANCE_PER_HOUR
                   × night_multiplier
                   × challenge_multiplier
                   × delta_hours
```

### 4.2 Parameters

| Parameter | Value |
|-----------|-------|
| `BASE_HACK_CHANCE_PER_HOUR` | 0.005 per game-hour per site |
| Night multiplier (10 PM – 5 AM) | 3.0× |
| Challenge level 0–1 | 0.25× |
| Challenge level 2 | 0.6× |
| Challenge level 3 | 1.0× |
| Challenge level 4 | 1.25× |
| Challenge level 5+ | 1.5× |

### 4.3 Constraints

- Only one active hacker globally at a time.
- No spawns while game is paused or tutorial is active.
- Attack type is 50/50 random between Overload and PriceSlash.
- Hack duration (time at target before effect triggers): random 15–45 game-minutes.

### 4.4 Hack Time Multiplier

Hacker timers (hacking countdown + effect duration + SOC auto-terminate) use a dedicated `hack_multiplier()` rather than the full game-speed `multiplier()`. This ensures hack effects last long enough to be noticeable in real time.

| Game Speed | `multiplier()` | `hack_multiplier()` | 30 game-min effect in real time |
|------------|----------------|---------------------|---------------------------------|
| Normal | 1440× | 45× | ~40 seconds |
| Fast | 2880× | 90× | ~20 seconds |

---

## 5. Infosec Countermeasures

Two tiered upgrades added to the existing `SiteUpgrades` system.

### 5.1 Cyber Firewall

| Property | Value |
|----------|-------|
| Cost | $12,000 |
| Prerequisite | None |
| Attack success chance | 50% (down from 100%) |
| Detection | Instant toast alert when attack begins |

### 5.2 Agentic SOC

| Property | Value |
|----------|-------|
| Cost | $35,000 |
| Prerequisite | Cyber Firewall |
| Attack success chance | 2% (down from 50%) |
| Auto-terminate | Active attacks cancelled after 5 game-minutes |
| Detection | Instant toast alert with "AUTO-BLOCKED" label |

### 5.3 Attack Success Resolution

When the hack timer expires:

```
base_success = 1.0
if has_cyber_firewall:  base_success = 0.50
if has_agentic_soc:     base_success = 0.02

roll = random(0.0 .. 1.0)
if roll < base_success:
    execute_attack()    # Overload or PriceSlash effect applied
else:
    attack_failed()     # "FIREWALLED" / "ACCESS DENIED" bubble
```

### 5.4 UI Control Locking

While a hack effect is active, the corresponding strategy panel controls are **locked** with a coloured overlay that blocks interaction:

| Attack type | Locked panel | Overlay colour | Label |
|-------------|-------------|---------------|-------|
| Price Slash | Pricing (all pricing sliders/buttons) | Semi-transparent green | "HACKED — PRICE OVERRIDE ACTIVE" |
| Transformer Overload | Power Management (density, BESS, peak shave, grid sellback) | Semi-transparent orange | "HACKED — OVERLOAD IN PROGRESS" |

- The player's stored settings are **not modified** — only overridden for the effect duration.
- When the hack expires (timer, Agentic SOC auto-terminate, or day-end cleanup), the overlay disappears and controls resume with the player's original values.
- Button handlers also reject inputs on hacked controls as a safety guard.

---

## 6. Transformer Overload: Power System Impact

### 6.1 Normal Operation

Under normal operation with Smart Load Shedding:

1. Transformer temperature is monitored continuously.
2. At 75°C (warning), charger power is gradually throttled.
3. At 90°C (critical), aggressive throttling prevents fire.
4. Load shedding limits charger `allocated_power_kw` based on remaining thermal headroom.

### 6.2 During Overload Attack

1. The `HackerOverloadActive` resource has a `remaining_secs` timer (default 1800.0 = 30 game-minutes).
2. `power.rs`: The smart load shedding check is bypassed — `site_upgrades.has_smart_load_shedding()` effectively returns `false` while the override is active.
3. `power_dispatch.rs`: Each charger is allocated its full `rated_power_kw`, ignoring transformer capacity limits.
4. The resulting load ratio exceeds 1.0, causing `update_temperature()` to drive `current_temp_c` toward `thermal_limit_c`.
5. If temperature stays above 90°C for 30 consecutive seconds, the existing fire system ignites the transformer.

### 6.3 Player Response Options

- **Manual throttle**: Disable chargers or reduce power density before transformer fires.
- **Agentic SOC auto-terminate**: Cancels the override after 5 game-minutes (before fire in most cases).
- **Rebuild**: If fire occurs, pay the $5,000 fine and wait for firetruck + rebuild.

---

## 7. Protocol Event Signatures

All protocol events fall into two categories:

- **Implicit signals**: State changes that flow naturally through existing protocol plumbing (realistic anomaly patterns).
- **Explicit security events**: Labeled security messages for ground-truth dataset creation.

### 7.1 OCPP (OCPP 1.6-J)

| Signal Type | Message | Key Fields | Trigger |
|-------------|---------|------------|---------|
| Implicit | MeterValues | `Power.Active.Import` spikes to rated max across all chargers simultaneously | Overload active |
| Implicit | StatusNotification | `status: Faulted`, `error_code: OverTemperature` | Overload → transformer fire |
| Implicit | SetChargingProfile | `chargingRateUnit: W`, `limit` jumps to rated max | Overload forces max power |
| Explicit | StatusNotification | `status: Faulted`, `error_code: OtherError`, `vendor_id: "KilowattTycoon"`, `vendor_error_code: "CYBER_OVERLOAD"`, `info: "CyberAttack: TransformerOverload"` | Overload attack starts |
| Explicit | StatusNotification | `vendor_error_code: "CYBER_PRICE_SLASH"`, `info: "CyberAttack: PriceManipulation"` | Price slash attack starts |
| Explicit | StatusNotification | `info: "CyberAttack: Mitigated by Agentic SOC"`, `status: Available` | Agentic SOC auto-terminates attack |

### 7.2 OCPI (2.3.0)

| Signal Type | Object | Key Fields | Trigger |
|-------------|--------|------------|---------|
| Implicit | Tariff PUT | `price` drops from ~$0.45 to $0.01 per kWh | Price slash activates |
| Implicit | Tariff PUT | `price` restores to normal | Price slash expires |
| Implicit | CDR POST | `total_cost / total_energy` ratio drops from ~0.45 to ~0.01 | Session completes during price slash |
| Implicit | EVSE Status | `status: OutOfOrder` | Overload → transformer fire |
| Explicit | Incident log | `object_type: "Incident"`, `action: "CyberAttack"`, attack type, affected charger IDs | Any attack starts |

### 7.3 OpenADR (3.0)

| Signal Type | Event Type | Key Fields | Trigger |
|-------------|-----------|------------|---------|
| Implicit | Price (Customer) | Price signal value drops from ~0.45 to 0.01 | Price slash active |
| Implicit | Report (Grid Telemetry) | `NET_IMPORT_KW` spikes as all chargers draw max power | Overload active |
| Explicit | AlertGridEmergency | `values: { "type": "CyberAttack", "attack": "TransformerOverload" }` | Overload attack starts |
| Explicit | AlertOther | `values: { "type": "CyberAttack", "attack": "PriceManipulation", "overridden_price": 0.01 }` | Price slash attack starts |
| Explicit | AlertOther | `values: { "type": "CyberAttackMitigated", "mitigation": "AgenticSOC" }` | Agentic SOC auto-terminates |

---

## 8. Day-End Cleanup

At the end of each game day (`OnEnter(AppState::DayEnd)`):

1. All `Hacker` entities are despawned (any phase).
2. `HackerOverloadActive` timer is reset to zero (override cancelled).
3. `HackerPriceOverride` timer is reset to zero (price restored).
4. `DailyHackerTracker` resets `hack_triggered_today` for the new day.
5. Associated VFX entities (`HackingGlitchVfx`, `HackerLootBubble`) are despawned.

---

## 9. Implementation Files

### New Files

| File | Contents |
|------|----------|
| `src/components/hacker.rs` | `Hacker`, `HackerPhase`, `HackerAttackType`, `HackerVariant`, `HACKER_NAMES` |
| `src/systems/hacker.rs` | Spawn, movement, arrival, attack, flee, cleanup systems |

### Modified Files

| File | Changes |
|------|---------|
| `src/components/mod.rs` | Add `pub mod hacker` and `pub use hacker::*` |
| `src/systems/mod.rs` | Register hacker systems in appropriate schedules |
| `src/events/mod.rs` | Add `HackerAttackEvent`, `HackerDetectedEvent` messages |
| `src/resources/site_upgrades.rs` | Add `has_cyber_firewall`, `has_agentic_soc`, `UpgradeId::CyberFirewall`, `UpgradeId::AgenticSoc` |
| `src/resources/strategy.rs` | Add hacker price override check to `effective_price()` |
| `src/components/power.rs` | Add `HackerOverloadActive` resource |
| `src/systems/power.rs` | Bypass load shedding when `HackerOverloadActive` |
| `src/systems/power_dispatch.rs` | Force max power allocation when `HackerOverloadActive` |
| `src/systems/sprite.rs` | Add `HackingGlitchVfx`, `HackerLootBubble` VFX components |
| `src/resources/mod.rs` | Init `DailyHackerTracker`, `HackerOverloadActive`, `HackerPriceOverride` |
| `src/states/mod.rs` | Add hacker entities to day-end cleanup |
| `src/ocpp/message_gen.rs` | Add `ocpp_hacker_event_system` |
| `src/ocpp/mod.rs` | Register OCPP hacker system |
| `src/ocpi/message_gen.rs` | Add `ocpi_hacker_event_system` |
| `src/ocpi/mod.rs` | Register OCPI hacker system |
| `src/openadr/message_gen.rs` | Add `openadr_hacker_event_system` |
| `src/openadr/mod.rs` | Register OpenADR hacker system |
