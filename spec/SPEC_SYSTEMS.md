# Kilowatt Tycoon — Detailed System Documentation

This document provides detailed specifications for gameplay systems not fully covered in the main ARCHITECTURE.md.

---

## 1. Emotion System

### 1.1 Overview

The emotion system provides visual feedback on driver and technician states through speech bubbles and facial expressions. It helps players understand why drivers are happy or frustrated.

### 1.2 Driver Emotions

#### EmotionMood (detailed display state)

| Mood | Character | Meaning |
|------|-----------|---------|
| VeryHappy | 😄 | Exceptional experience |
| Happy | 🙂 | Satisfied customer |
| Neutral | 😐 | Default state |
| Skeptical | 🤨 | Concerned about price/wait |
| Frustrated | 😠 | Long wait or issues |
| Angry | 😡 | About to leave without charging |

#### DriverMood (simplified state on Driver component)

The `Driver` component has a separate `DriverMood` enum synced from `EmotionMood` via `sync_mood_with_emotion`:

| DriverMood | Maps From |
|------------|-----------|
| Happy | VeryHappy, Happy |
| Neutral | Neutral, Skeptical |
| Impatient | Frustrated |
| Angry | Angry |

#### Emotion Triggers

| Reason | Mood | Sample Speech |
|--------|------|---------------|
| JustArrived | Neutral | "Let's charge up!" |
| PriceTooHigh | Skeptical | "$0.50/kWh?!" |
| PriceFair | Neutral | "Reasonable price" |
| PriceGreat | Happy | "Great deal!" |
| FoundCharger | Happy | "Found one!" |
| MustWait | Skeptical | "I'll wait..." |
| WaitingTooLong | Frustrated | "This is taking forever..." |
| ChargingStarted | Happy | "Finally!" |
| ChargingAlmostDone | Happy | "Almost there!" |
| ChargingComplete | VeryHappy | "All set!" |
| ChargerBroken | Angry | "Are you kidding me?!" |
| LeavingAngry | Angry | "Never coming back!" |
| SwitchedCharger | Happy | "Found another one!" |
| NoPower | Frustrated | "Zero kilowatts?!" |
| FrustrationBusy | Angry | "Too busy" |
| FrustrationDidntWork | Angry | "Broken" |
| FrustrationTooExpensive | Angry | "Too expensive" |
| FrustrationNoPower | Angry | "Zero power!" |

#### Implementation

```rust
#[derive(Component)]
pub struct DriverEmotion {
    pub mood: EmotionMood,
    pub reason: EmotionReason,
    pub set_at: f32,                                    // Real-time when set
    pub duration: f32,                                  // Real-seconds to display
    pub speech_text: Option<&'static str>,
    pub last_driver_state: Option<DriverState>,         // Tracks state changes
    pub last_frustration_reason: Option<EmotionReason>, // Why they left angry
}
```

**Key Design Decision**: Emotion durations are measured in **real seconds** (wall clock), not game time. This ensures speech bubbles remain readable at any game speed.

### 1.3 Technician Emotions

Technicians also have emotions for key moments:

| Reason | Sample Speech |
|--------|---------------|
| ArrivingAtSite | "On my way!" |
| StartingRepair | "Let's see what we got." |
| Repairing | "Working on it..." |
| RepairComplete | "Good as new!" |
| RepairFailed | "Hmm, that didn't work..." |
| LeavingSite | "Job complete." |
| NextJob | "On to the next one!" |

### 1.4 Frustration Tracking

The system tracks the last frustration reason for drivers who leave angry. This data surfaces in the HUD to help players understand churn causes.

---

## 2. Ambient Traffic System

### 2.1 Overview

The ambient traffic system creates visual life by spawning vehicles that drive past the charging station on the public road. These vehicles use pathfinding but don't interact with customers or chargers.

### 2.2 Key Components

| Component | Location | Purpose |
|-----------|----------|---------|
| `AmbientVehicle` | `systems/ambient_traffic.rs` | Marker for non-customer traffic |
| `VehicleFootprint` | `components/traffic.rs` | Tile footprint tracking |
| `VehicleTileRoute` | `components/traffic.rs` | Route tile data |
| `InterestedVehicle` | `systems/ambient_traffic.rs` | Marks vehicles that slow down near station |

### 2.3 Spawn Behavior

- Timer-based spawning every ~1.2 seconds
- Random vehicle type selection via global probability distribution (roll 0–100):
  - Sedan, SUV, Compact are most common
  - Bus, Semi, Tractor, Scooter, Motorcycle are rarer
- Direction is hardcoded to **LeftToRight** (not random)
- Small chance to be "interested" (slows down near station)

### 2.4 Vehicle Types

All 10 `VehicleType` variants can appear as ambient traffic:

`Compact`, `Sedan`, `Suv`, `Crossover`, `Pickup`, `Bus`, `Semi`, `Tractor`, `Scooter`, `Motorcycle`

> **Note**: Vehicle type weighting is global, not per-site-archetype.

### 2.5 Pathfinding

Ambient vehicles use `bevy_northstar` for pathfinding but:
- **No collision avoidance** with other ambient vehicles
- **Do avoid** static obstacles and parked customer vehicles
- Travel on the designated road row (entry_pos.1)

---

## 3. Site Roots System

### 3.1 Overview

The site roots system manages parent entities for each site's entity hierarchy. This enables automatic transform propagation, efficient visibility toggling, and per-site pathfinding grids.

### 3.2 Entity Hierarchy

```
SiteRoot (site_id, Transform at world_offset)
├── Charger entities (children)
├── Driver entities (children)
├── Tile sprites (children via Tiled)
└── Prop sprites (children)
```

### 3.3 Components

- `SiteRoot`: Marker component with `site_id`
- `BelongsToSite`: Tags all site-specific entities for filtering

### 3.4 Root Entity Spawning

When a site is added to `MultiSiteManager.owned_sites`, the `spawn_missing_site_roots` system:

1. Creates a root entity with `Transform` at the site's world offset
2. Attaches `CardinalGrid` (pathfinding) only to the currently viewed site
3. Stores the root entity reference in `SiteState.root_entity`

### 3.5 World Offset

Sites are spatially separated using `SiteState::world_offset()`:

```rust
const SITE_SPACING: f32 = 2000.0;

pub fn world_offset(&self) -> Vec2 {
    Vec2::new(self.id.0 as f32 * SITE_SPACING, 0.0)
}
```

### 3.6 Pathfinding Grid Management

Only one site can have a `CardinalGrid` at a time (bevy_northstar limitation). The `ActivePathfindingGrid` resource and `transfer_pathfinding_grid_on_site_switch` system handle grid transfer between sites.

---

## 4. Vehicle Movement System (bevy_northstar)

### 4.1 Overview

Vehicle movement uses `bevy_northstar` for grid-based A* pathfinding.

### 4.2 Key Components

| Component | Purpose |
|-----------|---------|
| `AgentPos` | Current grid position |
| `NextPos` | Next target grid cell |
| `Pathfind` | Pathfinding request (uses `.mode(PathfindMode::AStar)`) |
| `Blocking` | Marks entities as obstacles |

> **Note**: There is no `Target` component. Pathfind destinations are set directly via `Pathfind::new(goal)`.

### 4.3 Movement Phases

```rust
pub enum DriverState {
    Arriving,         // Entering site, heading to charger
    WaitingForCharger,// At entry, no charger available
    Queued,           // In queue waiting for charger
    Charging,         // At charger, receiving power
    Frustrated,       // Waited too long
    Complete,         // Charging finished
    Leaving,          // Heading to exit
    LeftAngry,        // Left without charging
    DepartingHappy,   // Leaving satisfied (triggers exit pathfinding)
    DepartingAngry,   // Leaving frustrated (triggers exit pathfinding)
}
```

### 4.4 Movement Flow

1. **Spawn**: Vehicle entity created at entry point
2. **Pathfind**: `Pathfind::new(charger_pos)` set on entity
3. **Movement**: `northstar_move_vehicles` interpolates position each frame
4. **Arrival**: `northstar_arrival_detection` triggers state transition
5. **Departure**: `northstar_trigger_departure` sets exit pathfinding when state becomes `DepartingHappy`/`DepartingAngry`
6. **Cleanup**: `northstar_cleanup_exited` removes vehicle at exit

### 4.5 Error Handling Systems

| System | Purpose |
|--------|---------|
| `northstar_handle_pathfinding_failed` | Retries with `PathfindCooldown` |
| `northstar_handle_reroute_failed` | Retries with `RerouteCooldown` |
| `northstar_clear_cooldown_on_success` | Clears cooldowns after success |
| `northstar_cleanup_ambient` | Removes exited ambient vehicles |

### 4.6 Grid Synchronization

When the player places or removes chargers:
1. `SiteGrid.revision` is incremented
2. `rebuild_site_pathfinding_grids` detects revision change
3. Grid nav is rebuilt via `sync_grid_nav()`
4. Active pathfind requests are invalidated

### 4.7 Driver Decision Rules

Three rules govern how drivers choose chargers and decide to leave. These
rules create a realistic information asymmetry between drivers and the
player (who has full visibility into charger health, grid allocation, etc.).

#### Rule 1: OCPI-Only Information (pre-plug-in)

Before plugging in, a driver can only use data visible on a public charging
app backed by the OCPI 2.3 feed (`src/ocpi/types.rs`):

| Available (OCPI)                | NOT available (internal)         |
|---------------------------------|----------------------------------|
| `EvseStatus` (Available, …)     | `charger.health`                 |
| `Connector.max_electric_power`  | `charger.reliability`            |
| `Connector.standard` (CCS/J1772)| `charger.get_derated_power()`    |
| `Connector.power_type` (AC/DC)  | Grid allocation / queue lengths  |

**Charger scoring** uses `rated_power_kw` (the OCPI-advertised max), not
health, reliability, or derated power. A 150 kW charger at 50% health still
looks like a 150 kW charger to the driver.

#### Rule 2: Direct Experience (post-plug-in)

Once plugged in, a driver observes their charging rate on the vehicle
dashboard. They can detect 0 kW delivery and poor power ratios.

**Zero-energy departure:** If `allocated_power_kw == 0` for
`ZERO_POWER_TOLERANCE_GAME_SECONDS` (120 game-seconds), the driver leaves
angry. The `zero_power_seconds` field on `Driver` tracks this.

#### Rule 3: Visual Observation (at the site)

A driver physically at the site can see which bays are empty and which
charger screens show errors. This justifies the alternative-charger search
when a driver is frustrated or receiving zero power.

**Alternative-charger search:** `frustrated_driver_system` (which handles
both `Frustrated` and `WaitingForCharger` states) checks all chargers at
the site each frame and reassigns the driver to the best available
alternative before falling through to patience drain.

#### Best-Charger Selection at Spawn

`driver_spawn_system` picks the bay whose linked charger has the highest
OCPI-advertised power (`rated_power_kw`). Random selection is only used as
a tiebreaker. This replaces the previous purely-random bay assignment.

#### Implementation

| Helper | Location | Purpose |
|--------|----------|---------|
| `score_charger_ocpi()` | `systems/driver.rs` | Returns `rated_power_kw` (Rule 1) |
| `find_best_alternative_charger()` | `systems/driver.rs` | Best available charger by OCPI score |
| `collect_charger_candidates()` | `systems/driver.rs` | Snapshots charger query to avoid borrow conflicts |

---

## 5. Demand Warnings System

### 5.1 Overview

The demand warnings system monitors power load and emits events when demand charges become significant.

### 5.2 Events

| Event | Status | Trigger |
|-------|--------|---------|
| `DemandBurdenEvent` | **Active** | Demand charge share exceeds threshold |
| `PeakIncreasedEvent` | Defined, not emitted | 15-min avg exceeds previous peak |
| `PeakRiskEvent` | Defined, not emitted | Load 90-100% of current peak |
| `BessSavedPeakEvent` | Defined, not emitted | BESS discharge prevented peak |
| `BessLowSocEvent` | Defined, not emitted | SOC < 20% during high load |

> **Note**: Only `DemandBurdenEvent` is actively emitted from `demand_warnings.rs`. The other four event structs are defined in `events/demand.rs` but no system currently writes them.

### 5.3 DemandBurdenEvent Fields

```rust
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

### 5.4 Cooldowns and Gating

- Single 60-second `ALERT_COOLDOWN_REAL` (wall clock time)
- Share-delta gating: `MIN_SHARE_DELTA = 0.03` (only alert when demand share changes meaningfully)
- Minimum charge threshold: `MIN_DEMAND_CHARGE = 500.0` (no alerts for small charges)

### 5.5 Toast Display

Demand events spawn toasts using real-time duration (see [SPEC_DEMAND_CHARGES.md](SPEC_DEMAND_CHARGES.md) for details).

---

## 6. Win/Lose Checking System

> **Status: NOT YET IMPLEMENTED** — The system is a no-op stub. The game operates day-by-day without explicit win/lose conditions.

### 6.1 Current Implementation

```rust
pub fn win_lose_system(_build_state: Res<BuildState>) {
    // No win/lose conditions - game continues day by day
    // Players can go into debt or lose reputation without triggering game over
}
```

### 6.2 Defined But Unused

The `GameResult` enum exists with the intended variants:

```rust
pub enum GameResult {
    InProgress,
    Won,
    LostBankruptcy,
    LostReputation,
    LostTimeout,
}
```

These are not checked or set by any system. The `GameEndedEvent` struct exists with a `reason: String` field but is never fired.

### 6.3 Planned Conditions (when implemented)

| Condition | Check |
|-----------|-------|
| Bankruptcy | `game_state.cash < 0.0` |
| Reputation Collapse | `game_state.reputation < threshold` |
| Revenue Target | `game_state.gross_revenue >= target` |
| Timeout | Complete N days without meeting revenue target |

---

## 7. Environment System

### 7.1 Overview

The environment system manages weather, news events, and their effects on gameplay.

### 7.2 Weather States

| Weather | Solar Mult | Charger Health | Demand Mult | Patience Mult |
|---------|------------|----------------|-------------|---------------|
| Sunny | 1.0 | 1.0 | 1.0 | 1.0 |
| Overcast | 0.6 | 1.0 | 1.0 | 1.0 |
| Rainy | 0.3 | 0.98 | 0.8 | 0.9 |
| Heatwave | 1.2 | 0.85 | 1.2 | 0.8 |
| Cold | 0.9 | 0.95 | 1.0 | 0.95 |

### 7.3 News Events

The `EnvironmentState` includes a news system:
- `active_news`: Current news headline (if any)
- `news_demand_multiplier`: Temporary demand modifier
- `roll_news_event()`: Random roll for new news events
- `forecast`: Upcoming weather preview

News events create temporary demand spikes or lulls and display in the top navigation ticker.

### 7.4 Time-of-Day Effects

Solar generation follows a bell curve based on game time:
- 0% at sunrise/sunset
- 100% at solar noon
- Modified by weather multiplier

### 7.5 Environmental Impact

| Factor | Affected Systems |
|--------|------------------|
| Solar generation | Power dispatch, BESS charging |
| Charger health | Fault probability (weather multiplier) |
| Demand | Driver spawn rates |
| Patience | Driver patience depletion rate |

---

*Document version: 2.0*
*Last updated: February 2026*
*Related: [ARCHITECTURE.md](ARCHITECTURE.md), [SPEC_OPERATIONS.md](SPEC_OPERATIONS.md)*
