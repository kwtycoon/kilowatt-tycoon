# Kilowatt Tycoon — Demand Charge System

This document specifies the demand charge mechanics, user experience, and implementation details.

---

## 1. Overview

### What Are Demand Charges?

Utilities charge a fee based on your **highest 15-minute average power draw** in a billing period. This creates a strategic layer where players must manage peak loads, not just total energy consumption.

**Example:**
- Peak: 300 kW
- Rate: $15/kW
- Monthly demand charge: $4,500

### Why This Matters for Gameplay

- Creates tension between "charge fast" and "minimize costs"
- Battery storage becomes valuable (peak shaving)
- Power density decisions have real trade-offs
- End-of-day costs are visible and controllable

---

## 2. Design Goals

### Player Experience Goals

1. **Visibility**: Peak cost shown in real-time, not just end-of-day
2. **Predictability**: Warnings appear before consequences
3. **Agency**: Clear actions available when warnings appear
4. **Learning**: Cause-effect is visible (warnings → costs)
5. **Mastery**: Advanced players can optimize threshold dancing

### UX Principles

- **Progressive Disclosure**: Basic info visible, details on demand
- **Tier 1 (Always Visible)**: Progress bar, peak cost, BESS status
- **Tier 2 (Event-Driven)**: Toast warnings with cooldowns
- **Tier 3 (On Demand)**: Power panel, configuration

---

## 3. Visual Feedback System

### Peak Meter Progress Bar

The peak meter shows current load relative to capacity:

| Zone | Load Range | Color | Meaning |
|------|------------|-------|---------|
| Safe | 0-70% | Green | Good headroom |
| Caution | 70-85% | Yellow | Approaching BESS threshold |
| Risk | 85-100% | Orange | BESS protecting you |
| Danger | >100% | Red | New peak being set |

```
Safe Zone:
[▓▓▓▓▓▓░░░░] 320/500 kW - Safe 🟢

Caution Zone:
[▓▓▓▓▓▓▓▓░░] 385/453 kW - Caution 🟡

Risk Zone (BESS Active):
[▓▓▓▓▓▓▓▓▓░] 453/500 kW - Risk 🟠

Danger Zone (New Peak):
[▓▓▓▓▓▓▓▓▓▓] 520/500 kW - NEW PEAK! 🔴
```

### Peak Cost Display

```
Peak Demand: 450 kW → 💰 $6,750
Last set: 2:45 PM
[▓▓▓▓▓▓▓▓▓░] 450/500 kW
```

The cost is always visible, creating immediate awareness of consequences.

---

## 4. BESS (Battery Energy Storage System)

### Status States

| State | Icon | Meaning |
|-------|------|---------|
| Standby | 💤 | Ready but not needed |
| Peak Shaving | 🛡️ | Actively preventing peak increase |
| Charging | ⚡ | Off-peak opportunity |
| Low SOC | 🪫 | Limited protection available |

### Peak Shaving Display

```
Battery: 🛡️ PEAK SHAVING
         Preventing +$750
         50 kW discharge @ 80% SOC
```

When BESS is actively shaving peaks, the dollar amount saved is prominently displayed.

---

## 5. Toast Warning System

### Toast Types

| Type | Trigger | Duration | Cooldown |
|------|---------|----------|----------|
| Peak Risk | Load 90-100% of peak | 7-8 sec | 60 sec |
| New Peak Set | Peak actually increases | 10-15 sec | None |
| BESS Low SOC | SOC < 20% during high load | 10-12 sec | 180 sec |
| Confirmation | Player takes action | 5-6 sec | None |

### Real-Time vs Game-Time

**Critical Design Decision:** Demand warning toasts use **real time** (wall clock), not game time.

**Why:** At 10x game speed, a 15-second game-time toast would only last 1.5 real seconds—unreadable!

```rust
// Demand toasts use RealTimeToast component
commands.spawn((
    ToastNotification { /* legacy fields */ },
    RealTimeToast {
        created_at_real: time.elapsed_secs(),  // Wall clock
        duration_real: 10.0,                    // Real seconds
    },
));
```

| Toast Type | At 1x Speed | At 10x Speed |
|------------|-------------|--------------|
| NEW PEAK SET | 10 seconds | **10 seconds** (same!) |
| PEAK RISK | 7 seconds | **7 seconds** (same!) |
| BESS LOW | 10 seconds | **10 seconds** (same!) |

### Toast Sequences

**Peak Increase Warning:**
```
┌────────────────────────────────────┐
│ ⚡ PEAK RISK                       │
│ Current: 495 kW                    │
│ Peak: 450 kW                       │
│                                    │
│ If load increases: +$$$            │
│ [Dismiss]                          │
└────────────────────────────────────┘
```

**New Peak Set:**
```
┌────────────────────────────────────┐
│ ⚠️ NEW PEAK SET                    │
│                                    │
│ 450 kW → 520 kW                    │
│ Demand: $6,750 → $7,800 (+$1,050)  │
│                                    │
│ [Reduce Load] [Dismiss]            │
└────────────────────────────────────┘
```

**BESS Saves the Day:**
```
┌────────────────────────────────────┐
│ 🛡️ BATTERY ACTIVATED               │
│                                    │
│ Load spike: 575 kW detected        │
│ BESS discharged: 75 kW             │
│ Peak held at: 500 kW               │
│                                    │
│ Saved: $1,125 in demand charges!   │
│ [Awesome!]                         │
└────────────────────────────────────┘
```

**Battery Low Warning:**
```
┌────────────────────────────────────┐
│ 🪫 BATTERY LOW                     │
│ SOC: 18% - Limited protection      │
│                                    │
│ Consider reducing load             │
│ [Reduce to 80%] [I'll manage]      │
└────────────────────────────────────┘
```

---

## 6. Events

### Demand Charge Events

```rust
// Defined but NOT currently emitted from demand_warnings.rs
#[derive(Event)]
pub struct PeakIncreasedEvent {
    pub old_peak_kw: f32,
    pub new_peak_kw: f32,
    pub demand_rate: f32,
    pub game_time: f32,
}

// Defined but NOT currently emitted
#[derive(Event)]
pub struct PeakRiskEvent {
    pub current_load_kw: f32,
    pub peak_kw: f32,
}

// Defined but NOT currently emitted
#[derive(Event)]
pub struct BessSavedPeakEvent {
    pub load_before_kw: f32,
    pub load_after_kw: f32,
    pub prevented_peak_kw: f32,
    pub savings: f32,
}

// Defined but NOT currently emitted
#[derive(Event)]
pub struct BessLowSocEvent {
    pub soc_percent: f32,
    pub current_load_kw: f32,
    pub peak_kw: f32,
    pub can_protect: bool,
}

// ** ACTIVELY EMITTED ** from demand_warnings.rs
#[derive(Event)]
pub struct DemandBurdenEvent {
    pub site_id: SiteId,
    pub demand_charge: f32,
    pub energy_cost: f32,
    pub revenue_today: f32,
    pub margin: f32,
    pub demand_share: f32,
    pub grid_kva: f32,
    pub peak_kw: f32,
    pub demand_rate: f32,
}
```

> **Note**: Only `DemandBurdenEvent` is actively emitted. The other four events are defined in `events/demand.rs` but no system currently writes them. They are reserved for future UX improvements.

### Event Emission Logic

```rust
// DemandBurdenEvent - the only active demand warning
// Emitted when demand_share changes by MIN_SHARE_DELTA (0.03) or more
// Gated by:
//   - ALERT_COOLDOWN_REAL: 60 seconds (wall clock)
//   - MIN_DEMAND_CHARGE: $500 (no alerts for small charges)
//   - demand_share delta threshold
```

---

## 7. State Transitions

```
[SAFE OPERATION]
    Grid < 70% capacity
    Color: Green
    BESS: Standby
    │
    ├─→ Load increases ─────────────────────┐
    │                                       │
    ↓                                       ↓
[CAUTION]                            [CHARGING]
Grid 70-85%                          Grid < 50% + Off-Peak
Color: Yellow                        BESS: Charging ⚡
BESS: Standby (Ready)                │
    │                                │
    ├─→ Load increases ────────┐     │
    │                          │     │
    ↓                          ↓     │
[APPROACHING]              [PROTECTED]
Grid 90-100% of peak      Grid > threshold
Color: Yellow             Color: Orange
Toast: "Peak Risk"        BESS: Active 🛡️
    │                     Toast: "Saving!"
    │                          │
    ├─→ Exceeds peak ──────────┤
    │                          │
    ↓                          ↓
[NEW PEAK]                [BESS EXHAUSTED]
Grid > previous peak      BESS SOC < 20%
Color: Red                Color: Red
Toast: "Peak Set!"        Toast: "Battery Low!"
    │                          │
    └─→ Manual intervention ←──┘
              │
              ↓
    [Player reduces load]
    [Player adjusts BESS]
    [Player accepts cost]
              │
              └─→ Returns to appropriate state
```

---

## 8. End-of-Day Summary

### Demand Breakdown

```
┌────────────────────────────────────────────────┐
│        Day 1 Complete                    [X]   │
├────────────────────────────────────────────────┤
│ Revenue:        +$4,250.00                     │
│ Energy Cost:    -$1,380.00                     │
│ Demand Charge:  -$6,750.00 ▼                   │
│   └─ Peak: 450 kW @ $15/kW                     │
│       Set at: 2:45 PM (afternoon rush)         │
│       BESS prevented: +$2,250 (avoided 600 kW) │
│       Net vs no battery: saved 25%             │
│ ──────────────────────────────────             │
│ Net Profit:     -$3,880.00                     │
└────────────────────────────────────────────────┘
```

### Opportunities Section

```
💡 Opportunities for Tomorrow:
 • Peak occurred during 2-4 PM rush
   Consider: Reduce power density 2-4 PM
 • BESS was low (18%) at peak time
   Consider: Charge battery earlier in day
 • 3 chargers active simultaneously at peak
   Consider: Stagger session starts
```

---

## 9. Color Palette

### Peak Status Colors

| State | Color | Hex |
|-------|-------|-----|
| Safe (0-70%) | Green | `#4CAF50` |
| Caution (70-85%) | Amber | `#FFC107` |
| Risk (85-100%) | Orange | `#FF9800` |
| Danger (>100%) | Red | `#F44336` |

### BESS Status Colors

| State | Color | Hex |
|-------|-------|-----|
| Standby | Blue Grey | `#90A4AE` |
| Peak Shaving | Blue | `#2196F3` |
| Charging | Yellow | `#FFEB3B` |
| Low SOC | Deep Orange | `#FF5722` |

### Financial Colors

| Type | Color | Hex |
|------|-------|-----|
| Positive (Savings) | Green | `#4CAF50` |
| Negative (Costs) | Red | `#F44336` |
| Warning | Orange | `#FF9800` |

---

## 10. Animation Timing

| Interaction | Response Time | Animation | Total |
|-------------|---------------|-----------|-------|
| Peak meter color change | <16ms | 200ms smooth | 200ms |
| Toast notification | Immediate | 300ms slide-in | 300ms |
| BESS activation | Same frame | 1500ms pulse | Immediate |
| Slider adjustment | Live | 200ms smooth | 200ms |

### Design Principle

- **Critical feedback** (peak change, BESS activation): <100ms
- **Celebratory feedback** (savings toast): 300-500ms delay
- **Educational content** (tutorial): 500ms+ delay

---

## 11. Player Behavior Journey

### Novice Phase

1. Places chargers, charges all vehicles at once
2. Peak display turns YELLOW → ORANGE → RED
3. Toast: "⚠️ NEW PEAK SET - +$1,500 charge"
4. End of day: "Demand Charge: -$9,000"
5. **Learning:** "I need to be careful with simultaneous charging"

### Intermediate Phase (After buying BESS)

1. Enables 4 chargers again
2. Peak display shows ORANGE (not RED!)
3. Toast: "🛡️ BATTERY ACTIVATED - Saved $1,050!"
4. **AHA moment:** "My battery is protecting me!"
5. **Learning:** BESS is valuable investment

### Advanced Phase

1. Opens BESS configuration
2. Adjusts threshold from 85% → 90%
3. Monitors SOC carefully during peaks
4. Manually reduces power density when needed
5. End of day: "Demand Charge: $4,500 (down from $7,500)"
6. **Mastery:** Understands 15-minute rolling window, threshold tuning, manual intervention timing

---

## 12. Implementation Notes

### Key Files

| File | Purpose |
|------|---------|
| `src/events/demand.rs` | Demand charge events |
| `src/systems/utility_billing.rs` | 15-min rolling average, peak tracking |
| `src/systems/demand_warnings.rs` | Event emission logic |
| `src/ui/demand_toasts.rs` | Toast spawning and display |
| `src/ui/sidebar/power_panel_inline.rs` | Peak meter, BESS status |

### 15-Minute Rolling Average

The demand charge is based on a 15-minute rolling average:

```rust
// UtilityMeter tracks samples for rolling average
pub struct UtilityMeter {
    pub demand_samples: VecDeque<(f32, f32)>,  // (time, load_kw)
    pub peak_demand_kw: f32,
    pub demand_charge: f32,
}

impl UtilityMeter {
    pub fn update_rolling_average(&mut self, current_time: f32, load_kw: f32) {
        // Add new sample
        self.demand_samples.push_back((current_time, load_kw));
        
        // Remove samples older than 15 minutes
        let cutoff = current_time - 900.0; // 15 min in seconds
        while let Some(&(time, _)) = self.demand_samples.front() {
            if time < cutoff {
                self.demand_samples.pop_front();
            } else {
                break;
            }
        }
        
        // Calculate average
        let avg = /* calculate average of samples */;
        
        // Update peak if exceeded
        if avg > self.peak_demand_kw {
            self.peak_demand_kw = avg;
        }
    }
}
```

---

*Document version: 1.1*
*Last updated: February 2026*
*Status: Partially implemented (DemandBurdenEvent active; other events defined but not emitted)*
