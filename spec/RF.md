# Kilowatt Tycoon — RF Environment & Communication Reliability

## 1. Purpose

This document specifies the **radio frequency (RF) environment simulation** that governs communication-related charger faults and connector reliability. The goal is to replace pure random fault injection with a physically-grounded model where faults emerge from observable site conditions the player can influence.

---

## 2. Design Philosophy

- **Faults should have causes.** Communication errors happen because the RF environment is degraded, not because a dice roll said so.
- **The player should be able to read the situation.** Visible SNR and noise floor stats let players anticipate problems before they happen.
- **Every build decision has RF consequences.** More chargers, more amenities, more vehicles — all raise the noise floor. The player must balance throughput against reliability.
- **Mitigation is a strategic choice.** RF boosters, charger tier selection, and load management all improve SNR, but each has a cost.

---

## 3. Real-World Grounding

EV charging sites are noisy RF environments. Real-world sources of interference include:

- **Power electronics**: DC fast charger inverters produce broadband EMI from high-frequency switching. Multiple chargers operating simultaneously compound this.
- **Vehicle radios**: Every vehicle on-site has Bluetooth, cellular, and often WiFi radios active. A busy lot has dozens of transmitters.
- **Powerline communication (PLC)**: CCS and CHAdeMO use PLC on the charging cable for session negotiation. Conducted EMI from adjacent chargers can corrupt these signals, causing connector lock/unlock failures.
- **Weather effects**: Rain increases multipath reflections and attenuates signals. Heat raises thermal noise in electronics.
- **On-site infrastructure**: WiFi access points, POS terminals, digital signage, kitchen equipment (microwaves), and security cameras all contribute to the RF environment.

The game models these effects at a gameplay-appropriate level of abstraction.

---

## 4. RF Environment Model

### 4.1 Core Concept

Each site maintains an `RfEnvironment` that is recomputed every frame. The model produces two key outputs:

- **Noise Floor**: aggregate RF noise level at the site (higher = worse)
- **SNR (Signal-to-Noise Ratio)**: effective signal quality (higher = better)

These drive the probability of `CommunicationError` faults and connector jam likelihood.

### 4.2 Noise Floor Computation

The noise floor is the sum of contributions from all noise sources at the site:

```
noise_floor = charging_noise + vehicle_noise + weather_noise + rush_hour_noise + amenity_noise
```

#### 4.2.1 Charging Session Noise

Each active charging session contributes EMI from the charger's power electronics:

```
charging_noise = 0.06 × active_sessions
```

Where `active_sessions` is the count of chargers with `is_charging == true` at this site.

**Charger tier modifier**: Value-tier chargers have cheaper EMI shielding and contribute more noise per session. This is handled at the per-charger susceptibility level (see Section 6.1), not in the aggregate noise floor.

#### 4.2.2 Vehicle Noise

Each vehicle on-site has active radios (Bluetooth, cellular, WiFi):

```
vehicle_noise = 0.03 × driver_count
```

Where `driver_count` is the current number of drivers at the site (any state except `LeftHappy`/`LeftAngry`).

#### 4.2.3 Weather Noise

| Weather     | Noise Contribution | Rationale                                      |
|-------------|--------------------|-------------------------------------------------|
| Sunny       | +0.00              | Baseline conditions                             |
| Overcast    | +0.03              | Minor atmospheric effects                       |
| Rainy       | +0.15              | Moisture attenuates signals, multipath reflections |
| Heatwave    | +0.10              | Thermal noise raises electronics noise floor    |
| Cold        | +0.05              | Cold affects battery-backed electronics slightly |

#### 4.2.4 Rush Hour Noise

Surrounding traffic and nearby building device density spike during commute hours:

| Time of Day | Noise Contribution | Rationale                    |
|-------------|--------------------|-----------------------------|
| 00:00–05:59 | +0.00              | Minimal ambient traffic      |
| 06:00–06:59 | +0.05              | Early commuters              |
| 07:00–08:59 | +0.12              | Morning rush peak            |
| 09:00–11:59 | +0.05              | Post-rush, moderate activity |
| 12:00–12:59 | +0.08              | Lunch hour device activity   |
| 13:00–16:59 | +0.05              | Afternoon baseline           |
| 17:00–18:59 | +0.15              | Evening rush peak            |
| 19:00–20:59 | +0.08              | Post-rush wind-down          |
| 21:00–23:59 | +0.03              | Late evening, minimal        |

#### 4.2.5 Amenity Noise

Each built amenity adds RF-emitting infrastructure to the site. Amenity noise scales with the amount of electronics each type introduces:

| Amenity            | Noise per Instance | Sources                                        |
|--------------------|--------------------|-------------------------------------------------|
| WiFi+Restrooms     | +0.05              | WiFi AP, hand dryer motors, motion sensors      |
| Lounge+Snacks      | +0.08              | Multiple WiFi devices, microwave, digital menus |
| Restaurant         | +0.12              | Full kitchen, multiple APs, POS systems, fridges |

Formula using `amenity_counts [wifi, lounge, restaurant]`:

```
amenity_noise = 0.05 × wifi_count + 0.08 × lounge_count + 0.12 × restaurant_count
```

### 4.3 Signal Strength and Boosters

The base signal strength represents the charger's built-in communication radio:

```
BASE_SIGNAL = 1.0
```

**RF Boosters** are buildable infrastructure (see Section 5) that increase effective signal strength at the site. Each booster acts as a repeater/antenna that improves coverage:

```
booster_bonus = 0.20 × booster_count^0.7
```

The exponent of 0.7 provides diminishing returns — the first booster gives the most benefit, and stacking beyond 3–4 units yields marginal improvement:

| Boosters | Bonus | Effective Signal |
|----------|-------|------------------|
| 0        | 0.00  | 1.00             |
| 1        | 0.20  | 1.20             |
| 2        | 0.33  | 1.33             |
| 3        | 0.43  | 1.43             |
| 4        | 0.52  | 1.52             |
| 5        | 0.60  | 1.60             |

### 4.4 SNR Computation

```
snr = (BASE_SIGNAL + booster_bonus) - noise_floor
snr = max(snr, 0.0)
```

SNR is the primary number players optimize. Higher SNR means fewer communication faults and fewer connector jams.

**Example scenarios:**

| Scenario                          | Noise Floor | Boosters | SNR  | Quality    |
|-----------------------------------|-------------|----------|------|------------|
| Empty site, sunny morning         | 0.00        | 0        | 1.00 | Excellent  |
| 4 sessions, 6 drivers, sunny      | 0.42        | 0        | 0.58 | Moderate   |
| 6 sessions, 8 drivers, rainy rush | 0.87        | 0        | 0.13 | Poor       |
| 6 sessions, 8 drivers, rainy rush | 0.87        | 2        | 0.46 | Moderate   |
| Full site, heatwave, amenities    | 1.10        | 3        | 0.33 | Fair       |

---

## 5. RF Booster (New Buildable Infrastructure)

### 5.1 Overview

The RF Booster is a small antenna/repeater unit that players can build to improve their site's signal-to-noise ratio. It counteracts the RF noise that naturally accumulates as sites grow.

### 5.2 Build Properties

| Property      | Value                  |
|---------------|------------------------|
| Name          | RF Booster             |
| Cost          | $25,000                |
| Footprint     | 1×1 tile               |
| Tile Content  | `BoosterPad`           |
| Build Panel   | Infrastructure         |
| Sell Refund   | 50% ($12,500)          |
| Max per Site  | No hard limit (diminishing returns provide soft cap) |

### 5.3 Visual

A small antenna post with signal wave indicators. Similar in visual footprint to a light pole but with a distinctive antenna element and status LED.

### 5.4 Placement Rules

- Requires an empty `Lot` tile (same as other infrastructure)
- No adjacency requirements
- Multiple boosters can be placed anywhere on the lot
- Boosters are purely passive infrastructure — no maintenance, no faults, no power draw

### 5.5 Strategic Considerations

- **Early game**: Usually unnecessary — few chargers, low noise floor, SNR stays high naturally.
- **Mid game**: First booster becomes valuable as the player adds 4–6 chargers and amenities. The jump from 0 to 1 booster is the most impactful (+0.20 signal).
- **Late game**: 2–3 boosters are typical for a dense site with restaurant and full charger bays during rush hour in bad weather. Beyond 3, the money is better spent elsewhere.
- **Cost tradeoff**: At $25k, a booster is cheaper than upgrading charger tiers but provides a different kind of benefit (site-wide vs per-charger).

---

## 6. How RF Environment Drives Faults

### 6.1 Communication Error Probability

Currently, `stochastic_fault_system` rolls a flat `fault_probability` per tick per charger and uses a fixed roulette to pick fault types (CommunicationError at 40%). The new model separates communication faults from hardware faults:

**Communication fault probability (per tick per charger):**

```
comm_fault_multiplier = clamp(1.0 - snr, 0.0, 2.0)
comm_fault_prob = base_comm_rate × comm_fault_multiplier × tier_susceptibility
```

Where:
- `base_comm_rate` is calibrated so that at SNR=0.5 the comm fault rate roughly matches the old system's rate
- `tier_susceptibility`: Value=1.3, Standard=1.0, Premium=0.6

When SNR is high (>1.0), `comm_fault_multiplier` drops to 0.0 — no communication faults in a clean RF environment. When SNR approaches 0.0, the multiplier hits the cap of 2.0 — double the baseline rate.

**Hardware fault probability (per tick per charger):**

The existing MTBF-based `fault_probability` continues to govern hardware faults (GroundFault, CableDamage, FirmwareFault, PaymentError). These are unaffected by RF conditions.

**Fault type selection when a fault fires:**

Instead of the old fixed roulette, if a comm fault triggers it is always `CommunicationError`. If a hardware fault triggers, the type is selected from the remaining types with adjusted weights:

| Fault Type         | Weight (hardware roulette) |
|--------------------|---------------------------|
| PaymentError       | 35%                        |
| FirmwareFault      | 30%                        |
| GroundFault        | 20%                        |
| CableDamage        | 15%                        |

### 6.2 Connector Jam Probability

CCS/CHAdeMO connectors use powerline communication (PLC) on the charging cable for lock/unlock negotiation. Conducted EMI from adjacent chargers can corrupt these signals.

**Modified connector jam check:**

```
jam_multiplier = clamp(1.5 - snr, 0.5, 2.5)
effective_jam_chance = charger.base_jam_chance × tier.jam_multiplier × jam_multiplier
```

| SNR  | jam_multiplier | Effect                              |
|------|---------------|--------------------------------------|
| 1.00 | 0.50          | Half the base jam rate (clean site)  |
| 0.75 | 0.75          | Slightly reduced                     |
| 0.50 | 1.00          | Baseline (matches old behavior)      |
| 0.25 | 1.25          | Elevated                             |
| 0.00 | 1.50          | 50% above baseline                   |

The `jam_multiplier` is clamped to `[0.5, 2.5]` to prevent jams from vanishing entirely or becoming absurdly frequent.

### 6.3 Staff Fault Reduction (Restaurant)

Having a Restaurant built on-site means staff are physically present during operating hours. Staff notice problems — sparking cables, error indicator lights, tripped breakers, disconnected ethernet — before they escalate.

**Effect on fault probability:**

```
staff_fault_multiplier = 0.85 ^ restaurant_count
```

| Restaurants | Multiplier | Effect                |
|-------------|------------|-----------------------|
| 0           | 1.00       | No reduction          |
| 1           | 0.85       | 15% fewer faults      |
| 2           | 0.72       | 28% fewer faults      |

This multiplier applies to **all** fault types (both comm and hardware), not just RF-related ones. It stacks multiplicatively with maintenance investment.

**Effect on fault detection:**

Without O&M software, faults are normally invisible until a driver tries to use the charger and discovers it. With restaurant staff on-site, faults are detected after approximately 60 game-seconds (1 minute), even without O&M:

| Detection Method     | Delay        |
|---------------------|--------------|
| No O&M, no staff    | Until a driver hits it (potentially hours) |
| Restaurant staff     | ~60 seconds  |
| O&M Detect tier     | ~10 seconds  |
| O&M Optimize tier   | ~10 seconds  |

Staff detection stacks with O&M — if both are present, O&M's faster detection wins.

This makes the Restaurant a net positive for reliability despite its high RF noise contribution (+0.12). The staff's physical presence more than compensates for the electromagnetic footprint.

---

## 7. Visible Stats (Operations Panel)

### 7.1 RF Environment Section

A new section in the Operations sidebar panel, placed between "O&M Statistics" and the fault list. Header: **"RF ENVIRONMENT"**.

### 7.2 Stats Display

| Stat            | Format          | Source                      | Example       |
|-----------------|----------------|-----------------------------|---------------|
| Noise Floor     | `XX dBm`       | Mapped from `noise_floor`   | `-82 dBm`    |
| SNR             | `XX dB`        | Mapped from `snr`           | `24 dB`      |
| Comm Fault Risk | Qualitative    | From `comm_fault_multiplier`| `Low`        |
| Boosters        | Count          | `booster_count`             | `2 active`   |

### 7.3 Display Unit Mapping

The internal `noise_floor` (0.0–1.5) and `snr` (0.0–1.5+) are unitless gameplay values. For display, they are mapped to dBm/dB ranges that feel realistic to players familiar with wireless:

**Noise Floor → dBm:**

```
displayed_noise_dbm = -90.0 + (noise_floor × 40.0)
```

| noise_floor | Displayed | Meaning        |
|-------------|-----------|----------------|
| 0.00        | -90 dBm   | Very quiet     |
| 0.25        | -80 dBm   | Low noise      |
| 0.50        | -70 dBm   | Moderate       |
| 0.75        | -60 dBm   | Noisy          |
| 1.00        | -50 dBm   | Very noisy     |
| 1.25        | -40 dBm   | Extremely noisy|

**SNR → dB:**

```
displayed_snr_db = snr × 30.0
```

| snr  | Displayed | Meaning    |
|------|-----------|------------|
| 1.00 | 30 dB     | Excellent  |
| 0.75 | 23 dB     | Good       |
| 0.50 | 15 dB     | Fair       |
| 0.25 | 8 dB      | Poor       |
| 0.10 | 3 dB      | Critical   |

### 7.4 Color Coding

| Stat        | Green        | Yellow         | Red          |
|-------------|-------------|----------------|--------------|
| Noise Floor | < -75 dBm   | -75 to -60 dBm | > -60 dBm   |
| SNR         | > 20 dB     | 10–20 dB       | < 10 dB      |

### 7.5 Comm Fault Risk Labels

| comm_fault_multiplier | Label        | Color  |
|----------------------|--------------|--------|
| 0.0–0.3              | Low          | Green  |
| 0.3–0.7              | Moderate     | Yellow |
| 0.7–1.2              | High         | Orange |
| > 1.2                | Critical     | Red    |

---

## 8. Data Model

### 8.1 RfEnvironment Struct

Per-site field on `SiteState` (not a global resource — each site has different physical conditions):

```rust
/// RF environment state for a single site.
/// Recomputed every frame by `rf_environment_system`.
#[derive(Debug, Clone)]
pub struct RfEnvironment {
    /// Raw noise level (0.0 = silent, 1.0+ = heavily congested)
    pub noise_floor: f32,

    /// Signal-to-noise ratio (higher = better comms reliability)
    pub snr: f32,

    /// Multiplier on CommunicationError fault probability (0.0 = no comm faults, 2.0 = max)
    pub comm_fault_multiplier: f32,

    /// Multiplier on connector jam probability (0.5 = half, 2.5 = max)
    pub jam_multiplier: f32,

    /// Multiplier on all fault probabilities from restaurant staff (1.0 = no effect)
    pub staff_fault_multiplier: f32,

    /// Whether restaurant staff provide faster fault detection
    pub staff_detection_bonus: bool,

    /// Number of RF boosters at this site
    pub booster_count: u32,
}

impl Default for RfEnvironment {
    fn default() -> Self {
        Self {
            noise_floor: 0.0,
            snr: 1.0,
            comm_fault_multiplier: 0.0,
            jam_multiplier: 1.0,
            staff_fault_multiplier: 1.0,
            staff_detection_bonus: false,
            booster_count: 0,
        }
    }
}
```

### 8.2 Constants

```rust
/// Base signal strength of charger radios (unitless, ~1.0)
const BASE_SIGNAL: f32 = 1.0;

/// Per-booster signal improvement (before diminishing returns exponent)
const BOOSTER_GAIN_PER_UNIT: f32 = 0.20;

/// Diminishing returns exponent for booster stacking
const BOOSTER_DIMINISHING_EXP: f32 = 0.7;

/// Restaurant staff fault reduction per restaurant (multiplicative)
const STAFF_FAULT_REDUCTION: f32 = 0.85;

/// Staff fault detection delay in game seconds (without O&M)
const STAFF_DETECTION_DELAY_SECS: f32 = 60.0;
```

---

## 9. System Architecture

### 9.1 System Ordering

```
GameSystemSet::Environment
  └── rf_environment_system          ← NEW: computes RfEnvironment per site

GameSystemSet::ChargerUpdate
  └── stochastic_fault_system        ← MODIFIED: uses comm_fault_multiplier, staff_fault_multiplier
  └── scripted_fault_system          (unchanged)
  └── charger_state_system           (unchanged)
  └── fault_detection_system         ← MODIFIED: uses staff_detection_bonus

GameSystemSet::ChargingUpdate
  └── driver_charging_system         ← MODIFIED: passes jam_multiplier to check_connector_jam
```

`rf_environment_system` runs in the `Environment` set, which executes before `ChargerUpdate` and `ChargingUpdate`. This ensures RF data is fresh when fault systems read it.

### 9.2 rf_environment_system

**Inputs:**
- `Query<&Charger, &BelongsToSite>` — count active sessions per site
- `Res<MultiSiteManager>` — read `driver_count`, `amenity_counts`, site grid (booster count)
- `Res<EnvironmentState>` — current weather
- `Res<GameClock>` — current hour for rush-hour calculation

**Outputs:**
- Writes `RfEnvironment` into each `SiteState`

**Pseudocode:**

```rust
for each site in multi_site.sites:
    // Count active charging sessions
    let active_sessions = chargers.iter()
        .filter(|c| c.belongs_to(site.id) && c.is_charging)
        .count();

    // Noise contributors
    let charging_noise = 0.06 * active_sessions as f32;
    let vehicle_noise = 0.03 * site.driver_count as f32;
    let weather_noise = match environment.current_weather {
        Sunny => 0.0, Overcast => 0.03, Rainy => 0.15,
        Heatwave => 0.10, Cold => 0.05,
    };
    let rush_noise = rush_hour_noise(game_clock.hour());
    let amenity_noise = 0.05 * amenity_counts[0] as f32   // wifi
                      + 0.08 * amenity_counts[1] as f32   // lounge
                      + 0.12 * amenity_counts[2] as f32;  // restaurant

    let noise_floor = charging_noise + vehicle_noise + weather_noise
                    + rush_noise + amenity_noise;

    // Boosters
    let booster_count = site.grid.count_tile(TileContent::BoosterPad);
    let booster_bonus = if booster_count > 0 {
        BOOSTER_GAIN_PER_UNIT * ops::powf(booster_count as f32, BOOSTER_DIMINISHING_EXP)
    } else {
        0.0
    };

    // SNR
    let snr = (BASE_SIGNAL + booster_bonus - noise_floor).max(0.0);

    // Derived multipliers
    let comm_fault_multiplier = (1.0 - snr).clamp(0.0, 2.0);
    let jam_multiplier = (1.5 - snr).clamp(0.5, 2.5);

    // Staff effects
    let restaurant_count = amenity_counts[2];
    let staff_fault_multiplier = ops::powf(STAFF_FAULT_REDUCTION, restaurant_count as f32);
    let staff_detection_bonus = restaurant_count > 0;

    site.rf_environment = RfEnvironment {
        noise_floor, snr, comm_fault_multiplier, jam_multiplier,
        staff_fault_multiplier, staff_detection_bonus, booster_count,
    };
```

### 9.3 Modified: stochastic_fault_system

**Before (current):**
```rust
let fault_prob = charger.fault_probability(delta_hours) * failure_mult;
if rng.random::<f32>() < fault_prob {
    let fault_type = match rng.random_range(0..100) {
        0..=40 => FaultType::CommunicationError,  // 40%
        41..=65 => FaultType::PaymentError,        // 25%
        66..=85 => FaultType::FirmwareFault,       // 20%
        86..=95 => FaultType::GroundFault,         // 10%
        _ => FaultType::CableDamage,               // 5%
    };
    inject_fault(&mut charger, fault_type, game_time);
}
```

**After (new):**
```rust
let rf = multi_site.get_site(site_id).rf_environment;

// Communication faults: driven by RF environment
let base_comm_rate = 0.4 * charger.fault_probability(delta_hours);
let comm_prob = base_comm_rate * rf.comm_fault_multiplier * rf.staff_fault_multiplier * failure_mult;
if rng.random::<f32>() < comm_prob {
    inject_fault(&mut charger, FaultType::CommunicationError, game_time);
    continue;
}

// Hardware faults: MTBF-based (unchanged formula, but staff multiplier applied)
let hw_prob = 0.6 * charger.fault_probability(delta_hours) * rf.staff_fault_multiplier * failure_mult;
if rng.random::<f32>() < hw_prob {
    let fault_type = match rng.random_range(0..100) {
        0..=34 => FaultType::PaymentError,     // 35%
        35..=64 => FaultType::FirmwareFault,   // 30%
        65..=84 => FaultType::GroundFault,     // 20%
        _ => FaultType::CableDamage,           // 15%
    };
    inject_fault(&mut charger, fault_type, game_time);
}
```

### 9.4 Modified: check_connector_jam

**Before:**
```rust
pub fn check_connector_jam(charger: &mut Charger, game_time: f32, tutorial_active: bool) -> bool {
    let effective_chance = charger.effective_jam_chance();
    if effective_chance > 0.0 {
        let mut rng = rand::rng();
        if rng.random::<f32>() < effective_chance {
            inject_fault(charger, FaultType::CableDamage, game_time);
            return true;
        }
    }
    false
}
```

**After:**
```rust
pub fn check_connector_jam(
    charger: &mut Charger,
    game_time: f32,
    tutorial_active: bool,
    rf_jam_multiplier: f32,
) -> bool {
    if tutorial_active { return false; }
    let effective_chance = charger.effective_jam_chance() * rf_jam_multiplier;
    if effective_chance > 0.0 {
        let mut rng = rand::rng();
        if rng.random::<f32>() < effective_chance {
            inject_fault(charger, FaultType::CableDamage, game_time);
            return true;
        }
    }
    false
}
```

### 9.5 Modified: fault_detection_system

Add staff detection as a fallback when O&M is not present:

```rust
let should_detect = match oem_tier.detection_delay_secs() {
    Some(delay) => elapsed >= delay,
    None => {
        // No O&M: check for restaurant staff detection
        if rf.staff_detection_bonus {
            elapsed >= STAFF_DETECTION_DELAY_SECS
        } else {
            false  // Not detected until a driver tries to charge
        }
    }
};
```

---

## 10. Balance Calibration

### 10.1 Design Targets

The RF system should be calibrated so that:

| Scenario | Target CommunicationError Rate | Player Experience |
|----------|-------------------------------|-------------------|
| Small site (2–3 chargers), no amenities, fair weather | ~0 comm faults/day | "Clean" early game — focus on learning other mechanics |
| Medium site (4–6 chargers), 1 amenity, mixed weather | 1–2 comm faults/day | Noticeable but manageable — prompts first booster purchase |
| Large site (8+ chargers), full amenities, rush hour | 3–5 comm faults/day without boosters | Pressure to invest in RF infrastructure |
| Large site with 2–3 boosters and restaurant | 1–2 comm faults/day | Rewarding payoff for investment |

### 10.2 Connector Jam Calibration

With the default `connector_jam_chance` of 0.02 (2%):

| Site Conditions | Effective Jam Rate | Player Experience |
|----------------|-------------------|-------------------|
| Low noise (SNR ≈ 1.0) | ~1% per session | Rare — barely noticed |
| Medium noise (SNR ≈ 0.5) | ~2% per session | Occasional — matches old behavior |
| High noise (SNR ≈ 0.1) | ~3.5% per session | Frequent enough to motivate boosters |

### 10.3 Tuning Levers

If the system needs balance adjustments during playtesting:

| Lever | Effect | Location |
|-------|--------|----------|
| `BASE_SIGNAL` | Shifts entire SNR curve up/down | Constants |
| `BOOSTER_GAIN_PER_UNIT` | How much each booster helps | Constants |
| `BOOSTER_DIMINISHING_EXP` | How quickly boosters lose effectiveness | Constants |
| Per-source noise weights | Individual contributor strength | `rf_environment_system` |
| `comm_fault_multiplier` clamp range | Cap on comm fault severity | `rf_environment_system` |
| `jam_multiplier` clamp range | Cap on connector jam severity | `rf_environment_system` |
| `STAFF_FAULT_REDUCTION` | Restaurant fault reduction strength | Constants |

---

## 11. Files Changed

| File | Changes |
|------|---------|
| `src/resources/multi_site.rs` | Add `RfEnvironment` struct and field to `SiteState` |
| `src/resources/site_grid.rs` | Add `TileContent::BoosterPad`, booster count helper |
| `src/resources/build_state.rs` | Add `BuildTool::RfBooster` (1×1, $25k) |
| `src/resources/asset_handles.rs` | Add `prop_rf_booster` image handle |
| `src/systems/charger.rs` | New `rf_environment_system`; modify `stochastic_fault_system`, `check_connector_jam` |
| `src/systems/driver.rs` | Pass `rf_jam_multiplier` to `check_connector_jam` calls |
| `src/systems/build_input.rs` | Handle `BuildTool::RfBooster` placement |
| `src/systems/scene.rs` | Render booster visual from grid |
| `src/systems/mod.rs` | Register `rf_environment_system` in `Environment` set |
| `src/ui/sidebar/operations_panel.rs` | Add "RF ENVIRONMENT" stats section |
| `src/ui/sidebar/build_panels.rs` | Add RF Booster to infrastructure build panel |
| `src/data/tiled_loader.rs` | Map `"BoosterPad"` to `TileContent::BoosterPad` |
| `tests/charger_systems_test.rs` | Update tests, add RF environment tests |
| `assets/props/prop_rf_booster.svg` | New antenna/repeater visual asset |

---

## 12. Relationship to Other Systems

| System | Interaction |
|--------|------------|
| **OCPP WebSocket** (`src/ocpp/`) | `CommunicationError` maps to `EVCommunicationError` in OCPP status notifications. RF-driven comm faults produce realistic OCPP traffic patterns. |
| **O&M Software** (`fault_detection_system`) | O&M detection delay overrides staff detection (O&M is faster). Auto-remediation of comm faults works regardless of RF conditions. |
| **Weather** (`EnvironmentState`) | Weather is a read-only input to RF. No feedback loop — RF doesn't affect weather. |
| **Power Dispatch** (`power_dispatch_system`) | No direct interaction. Power allocation is unaffected by RF. |
| **Demand/Drivers** (`driver_spawn_system`) | Driver count is a read-only input to RF noise. RF does not affect driver arrival or patience directly (only indirectly via faults). |
| **Security System** | No RF interaction — security cameras are wired, not wireless. |
| **Hacker System** (`src/systems/hacker.rs`) | Hackers could potentially exploit poor RF conditions in future expansions, but no interaction in this spec. |
