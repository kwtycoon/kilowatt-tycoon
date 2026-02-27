# Kilowatt Tycoon — Grid & Power Simulation

> **Note**: Code examples in this document are pseudocode/GDScript for illustration. The actual implementation is in Rust (see `src/systems/power_dispatch.rs`, `src/systems/power.rs`, `src/systems/utility_billing.rs`).

## 1. Purpose
This document specifies the **electrical simulation** underpinning Kilowatt Tycoon. The goal is to make power management a strategic challenge where careless expansion causes cascading failures.

---

## 2. Design Philosophy
- **Electricity is invisible but consequential.** Players learn through failures.
- **Realism serves gameplay.** We simplify real-world physics but preserve the *shape* of real problems.
- **Headroom is survival.** Operating at 100% capacity means operating at 100% risk.

---

## 3. Power Fundamentals: kW vs kVA

The game distinguishes between **Real Power (kW)** and **Apparent Power (kVA)** to simulate real-world grid constraints and equipment stress.

### 3.1 Charger Efficiency

Charging equipment is not 100% efficient. Energy is lost as heat during AC-to-DC conversion.

- **Formula**: P_input_kW = P_output_kW / η
- **Tier-based Efficiency**:
  - **Value Tier**: 88% (η = 0.88)
  - **Standard Tier**: 92% (η = 0.92)
  - **Premium Tier**: 96% (η = 0.96)

### 3.2 Power Factor

Power Factor represents how effectively equipment uses the current drawn from the grid. Inductive and capacitive loads cause "reactive power" that doesn't deliver work but still occupies grid capacity.

- **Formula**: S_kVA = P_input_kW / PF
- **Standard PF**: 0.95 (DC Fast Chargers usually have active power factor correction)

### 3.3 Total Site Load Calculation

To find the total apparent power required from the utility:

```
Site kVA = Σ (Charger Output kW / (Efficiency × Power Factor))
```

### 3.4 Effective Site Capacity

The actual usable capacity is the lesser of utility and transformer ratings:

```
Effective Capacity = min(Utility kVA, Transformer kVA)
```

If a player places a 500 kVA transformer on a 1500 kVA site, they can only pull 500 kVA without specialized strategies.

---

## 4. Electrical Model Overview

```
[GRID CONNECTION]
       │
       ▼
[MAIN TRANSFORMER] ──thermal_limit, tap_position
       │
       ├──[PHASE A]──┬──[Charger 1]
       │             └──[Charger 2]
       │
       ├──[PHASE B]──┬──[Charger 3]
       │             └──[Charger 4]
       │
       └──[PHASE C]──┬──[Charger 5]
                     └──[Charger 6]
```

Each site has:
- One **grid connection** with a contracted capacity (kVA)
- One or more **transformers** stepping down voltage
- **Three-phase distribution** to chargers
- Individual **charger power electronics**

---

## 5. Core Electrical Parameters

### 5.1 Site-Level Parameters

| Parameter | Unit | Description |
|-----------|------|-------------|
| `contracted_capacity` | kVA | Maximum power the utility allows before penalties |
| `grid_voltage` | V | Nominal voltage at point of connection (e.g., 480V, 400V) |
| `grid_impedance` | Ω | Affects voltage drop under load |
| `transformer_rating` | kVA | Transformer's nameplate capacity |
| `transformer_thermal_limit` | °C | Max operating temperature before trip |
| `transformer_tap_position` | int | Voltage adjustment setting (−5 to +5) |

### 5.2 Phase-Level Parameters

| Parameter | Unit | Description |
|-----------|------|-------------|
| `phase_current` | A | Current flowing through phase |
| `phase_voltage` | V | Actual voltage on phase (varies under load) |
| `phase_power_factor` | ratio | Ratio of real power to apparent power |
| `phase_imbalance` | % | Deviation from ideal equal loading |

### 5.3 Charger-Level Parameters

| Parameter | Unit | Description |
|-----------|------|-------------|
| `rated_power` | kW | Charger nameplate power |
| `actual_power` | kW | Power currently delivered (may be derated) |
| `input_voltage` | V | Voltage at charger input terminals |
| `efficiency` | % | Conversion efficiency (85–95% typical) |
| `derate_factor` | ratio | 0.0–1.0, applied to rated_power |

---

## 6. Voltage Drop Simulation

### 6.1 Problem Statement
When chargers draw power, voltage drops across the distribution system. Low voltage causes:
- Reduced charging speed
- Charger faults (undervoltage protection)
- Power supply stress

### 6.2 Voltage Drop Formula
Simplified single-phase approximation (applied per phase):

```
V_drop = I × (R × cos(φ) + X × sin(φ)) × L × 2
```

Where:
- `I` = Current (A)
- `R` = Resistance per km (Ω/km)
- `X` = Reactance per km (Ω/km)
- `cos(φ)` = Power factor
- `L` = Cable length (km)

### 6.3 Implementation
For gameplay, we simplify to:

```gdscript
func calculate_voltage_drop(load_kw: float, distance_m: float, cable_size: CableSize) -> float:
    var resistance_per_m: float = cable_size.resistance_ohm_per_m
    var current: float = load_kw * 1000.0 / nominal_voltage
    var v_drop: float = 2.0 * current * resistance_per_m * distance_m
    return v_drop
```

### 6.4 Voltage Drop Effects

| Voltage Level | Effect |
|---------------|--------|
| >95% nominal | Normal operation |
| 90–95% nominal | −10% charging speed (automatic derate) |
| 85–90% nominal | −25% charging speed; "Low Voltage" warning |
| 80–85% nominal | −50% charging speed; driver complaint likely |
| <80% nominal | Charger trips on undervoltage protection |

### 6.5 Mitigation Strategies (Player Actions)
- **Upgrade cables**: Thicker cables = lower resistance
- **Shorten runs**: Place chargers closer to transformer
- **Reduce load**: Fewer simultaneous sessions
- **Adjust transformer tap**: Raise output voltage (but risks overvoltage when unloaded)

---

## 7. Phase Balancing

### 7.1 Problem Statement
Three-phase systems work best when load is equally distributed across phases. Imbalance causes:
- Neutral current (wasted energy, heat)
- Voltage asymmetry
- Transformer stress
- In extreme cases: protection trips

### 7.2 Phase Imbalance Formula

```
imbalance_percent = (max_phase_load - min_phase_load) / average_phase_load × 100
```

### 7.3 Imbalance Effects

| Imbalance | Effect |
|-----------|--------|
| <10% | Normal operation |
| 10–20% | Transformer runs warm; efficiency −2% |
| 20–30% | Audible transformer hum; efficiency −5%; warning |
| 30–50% | High neutral current; efficiency −10%; potential nuisance trips |
| >50% | Protection trip; site goes offline |

### 7.4 Implementation
```gdscript
func calculate_phase_imbalance(phase_loads: Array[float]) -> float:
    var max_load: float = phase_loads.max()
    var min_load: float = phase_loads.min()
    var avg_load: float = (phase_loads[0] + phase_loads[1] + phase_loads[2]) / 3.0
    if avg_load == 0.0:
        return 0.0
    return ((max_load - min_load) / avg_load) * 100.0
```

### 7.5 Mitigation Strategies (Player Actions)
- **Assign chargers to phases deliberately**: When installing, choose phase assignment
- **Dynamic load balancing (upgrade)**: Automatic rotation of new sessions to least-loaded phase
- **Three-phase chargers**: Large DC chargers draw equally from all phases

---

## 8. Grid Stability & Inrush Events

### 8.1 Problem Statement
Starting a high-power charging session causes a momentary **inrush current** that can:
- Cause voltage sag across the site
- Trip breakers if too many sessions start simultaneously
- Damage equipment over time

### 8.2 Inrush Model
When a charger starts:
1. **Inrush spike**: 3–5× rated current for 50–200ms
2. **Ramp-up**: Current rises from 0 to target over 5–30 seconds (configurable)
3. **Steady state**: Stable power delivery

### 8.3 Inrush Effects

| Scenario | Effect |
|----------|--------|
| 1 charger starts | Normal; brief voltage dip (2–5%) |
| 2 chargers start within 5 seconds | Visible voltage sag (5–10%); other sessions may derate |
| 3+ chargers start within 5 seconds | High probability of breaker trip (30–60%) |
| Large DC charger (>150kW) starts | Significant voltage dip; may affect adjacent chargers |

### 8.4 Implementation
```gdscript
signal session_started(charger_id: String, inrush_kw: float)

var inrush_events: Array[Dictionary] = []  # {time: float, power: float}
const INRUSH_WINDOW: float = 5.0  # seconds
const INRUSH_MULTIPLIER: float = 4.0

func on_session_start(charger: Charger) -> void:
    var inrush_power: float = charger.rated_power * INRUSH_MULTIPLIER
    inrush_events.append({"time": game_time, "power": inrush_power})
    _check_for_trip()

func _check_for_trip() -> void:
    var recent_inrush: float = 0.0
    for event in inrush_events:
        if game_time - event.time < INRUSH_WINDOW:
            recent_inrush += event.power
    
    if recent_inrush > site.contracted_capacity * 1.5:
        _trigger_breaker_trip()
```

### 8.5 Mitigation Strategies (Player Actions)
- **Stagger session starts**: Use queue management to space out connections
- **Soft-start chargers (upgrade)**: Reduces inrush multiplier from 4× to 2×
- **Install capacitor bank (upgrade)**: Absorbs inrush, prevents voltage sag
- **Upgrade contracted capacity**: Higher limit = more headroom

---

## 9. Transformer Thermal Model

### 9.1 Problem Statement
Transformers heat up under load. Continuous high load causes thermal degradation and eventual failure.

### 9.2 Thermal Model
Simplified first-order thermal dynamics:

```gdscript
const THERMAL_TIME_CONSTANT: float = 600.0  # seconds (10 minutes)
const AMBIENT_TEMP: float = 25.0  # °C
const MAX_TEMP_RISE: float = 65.0  # °C above ambient at 100% load

func update_transformer_temp(delta: float, load_percent: float) -> void:
    var target_temp: float = AMBIENT_TEMP + (load_percent / 100.0) * MAX_TEMP_RISE
    var temp_diff: float = target_temp - transformer_temp
    transformer_temp += temp_diff * (delta / THERMAL_TIME_CONSTANT)
```

### 9.3 Thermal Effects

| Temperature | Effect |
|-------------|--------|
| <70°C | Normal operation |
| 70–85°C | "Hot" warning; accelerated aging |
| 85–100°C | Critical warning; automatic load shedding (derate all chargers 20%) |
| 100–110°C | Emergency shutdown imminent; derate 50% |
| >110°C | Thermal trip; transformer offline for 30 minutes (cooldown) |

### 9.4 Aging Model
Transformers have a **lifetime** measured in "thermal hours":
- Operating <70°C: 1:1 aging (1 hour of operation = 1 hour of life used)
- Operating 70–85°C: 2:1 aging
- Operating 85–100°C: 4:1 aging
- Operating >100°C: 8:1 aging

When lifetime is exhausted: transformer fails permanently and must be replaced.

### 9.5 Overloading Mechanics

Advanced players can **Override** utility limits via the Service Strategy, allowing the site to draw up to **150%** of its rated capacity.

- **Thermal Death Spiral**: High load causes voltage sag (95% → 80%), forcing chargers to draw even more current to maintain power, accelerating heating.
- **Catastrophic Failure**: If `health` reaches 0% or `current_temp` exceeds 120°C, the transformer enters an `IsOnFire` state:
  - Site power is immediately cut (Blackout)
  - Massive repair costs or total replacement required
  - Reputation penalty with the utility company
  - Potential damage to adjacent chargers or amenities

### 9.6 Economic Trade-offs

| Strategy | Benefit | Risk |
| :--- | :--- | :--- |
| **Strict Compliance** | Zero maintenance risk, stable grid. | Slower charging, lower throughput, lost revenue. |
| **Moderate Overload** | Higher peak throughput during rushes. | Slight health decay, occasional voltage sag warnings. |
| **Aggressive Overload** | Maximum possible revenue. | High risk of fire, frequent equipment failure, expensive replacement. |

### 9.7 Mitigation Strategies (Player Actions)
- **Don't overload**: Keep average load <80% of transformer rating
- **Install cooling (upgrade)**: Fans reduce effective temperature by 10–15°C
- **Upgrade transformer**: Higher-rated transformer runs cooler at same load
- **Load shedding**: Manually reduce capacity during peak periods

---

## 10. Demand Charges & Peak Penalties

### 10.1 Problem Statement
Utilities charge based on **peak demand** (highest 15-minute average power), not just energy consumed.

### 10.2 Demand Charge Model
```gdscript
var peak_demand_kw: float = 0.0  # highest rolling 15-min average this billing period
var current_15min_avg: float = 0.0
var demand_samples: Array[float] = []

func update_demand(current_load_kw: float, delta: float) -> void:
    demand_samples.append(current_load_kw)
    if demand_samples.size() > SAMPLES_PER_15_MIN:
        demand_samples.pop_front()
    current_15min_avg = demand_samples.reduce(func(a, b): return a + b) / demand_samples.size()
    peak_demand_kw = max(peak_demand_kw, current_15min_avg)
```

### 10.3 Cost Formula
```
monthly_demand_charge = peak_demand_kw × demand_rate_per_kw
```

Example: If `demand_rate_per_kw = $15` and `peak_demand_kw = 200`, demand charge = $3,000/month.

### 10.4 Penalty for Exceeding Contract
If `peak_demand_kw > contracted_capacity`:
- Overage charge: 2× normal rate for excess kW
- Utility warning: 3 overages in 12 months = forced contract renegotiation (higher base rate)

### 10.5 Mitigation Strategies (Player Actions)
- **Battery storage**: Shave peaks by discharging during high demand
- **Dynamic pricing**: Raise prices during peak to reduce demand
- **Queue management**: Limit simultaneous sessions
- **Scheduled charging**: Offer discounts for off-peak sessions

---

## 11. Battery Storage Integration

### 11.1 Overview
Battery storage is an **expensive but powerful** tool for managing grid constraints.

### 11.2 Battery Parameters

| Parameter | Unit | Description |
|-----------|------|-------------|
| `capacity` | kWh | Total energy storage |
| `max_power` | kW | Maximum charge/discharge rate |
| `state_of_charge` | % | Current energy level (0–100%) |
| `round_trip_efficiency` | % | Energy out / energy in (85–92% typical) |
| `cycle_count` | int | Number of full cycles used |
| `max_cycles` | int | Lifetime limit before replacement |

### 11.3 Battery Modes

| Mode | Behavior |
|------|----------|
| **Peak Shaving** | Discharge when site load approaches contracted capacity |
| **Backup** | Discharge only during grid outage |
| **Arbitrage** | Charge during low-price hours; discharge during high-price hours |
| **Manual** | Player-controlled charge/discharge |

### 11.4 Peak Shaving Logic
```gdscript
const PEAK_SHAVE_THRESHOLD: float = 0.85  # start discharging at 85% capacity

func update_battery(site_load_kw: float, delta: float) -> float:
    var headroom: float = contracted_capacity - site_load_kw
    var threshold_kw: float = contracted_capacity * PEAK_SHAVE_THRESHOLD
    
    if site_load_kw > threshold_kw and state_of_charge > 0.1:
        var discharge_needed: float = site_load_kw - threshold_kw
        var discharge_actual: float = min(discharge_needed, max_power, available_energy / delta)
        state_of_charge -= (discharge_actual * delta) / (capacity * 3600.0)
        return discharge_actual  # reduces apparent grid draw
    else:
        # Charge during low load
        if site_load_kw < threshold_kw * 0.5 and state_of_charge < 0.95:
            var charge_headroom: float = threshold_kw * 0.5 - site_load_kw
            var charge_actual: float = min(charge_headroom, max_power)
            state_of_charge += (charge_actual * delta * round_trip_efficiency) / (capacity * 3600.0)
            return -charge_actual  # increases apparent grid draw
    return 0.0
```

---

## 12. Failure Modes Summary

| Failure Type | Cause | Effect | Recovery |
|--------------|-------|--------|----------|
| **Undervoltage Trip** | Voltage <80% nominal | Charger offline | Auto-reset when voltage recovers |
| **Breaker Trip** | Inrush overload or short circuit | Phase or site offline | Manual reset (technician) or remote reset |
| **Transformer Thermal Trip** | Overtemperature (>110°C) | Site offline | 30-minute cooldown |
| **Transformer Failure** | Lifetime exhausted | Site offline | Replacement (expensive, 24-hour lead time) |
| **Phase Imbalance Trip** | >50% imbalance | Affected phase offline | Rebalance loads, then reset |
| **Grid Outage** | External (utility) | Entire site offline | Wait for grid restoration (or use battery backup) |

---

## 13. UI Indicators

### 13.1 Site Overview Panel
- **Power gauge**: Current load vs contracted capacity (green/yellow/red zones)
- **Phase bars**: Three vertical bars showing load per phase
- **Transformer temp**: Thermometer icon with temperature and status color
- **Voltage indicator**: Percentage of nominal (dims when low)

### 13.2 Charger Tooltips
- Actual vs rated power
- Input voltage
- Derate reason (if applicable)

### 13.3 Alerts
- "Approaching Capacity Limit" (>85%)
- "Phase Imbalance Warning" (>20%)
- "Transformer Running Hot" (>70°C)
- "Voltage Sag Detected" (<90%)
- "Breaker Trip!" (immediate action required)

---

## 14. Tuning Parameters (Data-Driven)

All electrical parameters should be defined in data files for easy balancing:

```json
{
  "site_defaults": {
    "grid_voltage": 400,
    "grid_impedance": 0.02,
    "demand_rate_per_kw": 15.0
  },
  "transformer_types": [
    {
      "id": "small_100kva",
      "rating_kva": 100,
      "thermal_limit_c": 110,
      "max_temp_rise_c": 65,
      "cost": 15000,
      "lifetime_hours": 100000
    },
    {
      "id": "medium_250kva",
      "rating_kva": 250,
      "thermal_limit_c": 115,
      "max_temp_rise_c": 60,
      "cost": 35000,
      "lifetime_hours": 120000
    }
  ],
  "cable_sizes": [
    {"id": "16mm", "resistance_ohm_per_m": 0.00115, "cost_per_m": 8},
    {"id": "35mm", "resistance_ohm_per_m": 0.000524, "cost_per_m": 15},
    {"id": "70mm", "resistance_ohm_per_m": 0.000262, "cost_per_m": 28}
  ]
}
```

---

## 15. Open Questions

1. **Power factor simulation**: Include reactive power (kVAR) or simplify to real power only?
   - *Assumption for MVP*: Real power only; power factor = 0.95 constant.

2. **Weather effects on grid**: Model temperature-dependent transformer ratings?
   - *Assumption for MVP*: Yes, ambient temperature affects thermal headroom.

3. **Harmonics**: Model harmonic distortion from charger power electronics?
   - *Assumption for MVP*: No; too complex for initial release.

4. **Multi-site grid interaction**: Can two nearby sites affect each other?
   - *Assumption for MVP*: No; each site is electrically independent.

---

## 16. Summary
This spec defines a **layered electrical simulation**:
1. **Site level**: Contracted capacity, demand charges, transformer limits
2. **Phase level**: Three-phase distribution, imbalance penalties
3. **Charger level**: Voltage drop, derating, inrush events

The complexity is revealed gradually through gameplay consequences, not tutorials.

