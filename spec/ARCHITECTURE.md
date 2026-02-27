# Kilowatt Tycoon Architecture

This document describes the high-level architecture of Kilowatt Tycoon.

## Overview

Kilowatt Tycoon is a top-down 2D EV charging station management simulation built with the [Bevy](https://bevyengine.org/) game engine (v0.17+). It follows the Entity-Component-System (ECS) architectural pattern with event-driven communication.

## Core Concepts

### Entity-Component-System (ECS)

- **Entities**: Unique identifiers for game objects (chargers, drivers, vehicles, transformers)
- **Components**: Data attached to entities (`Charger`, `Driver`, `Transform`, `BelongsToSite`)
- **Systems**: Functions that process entities with specific components
- **Resources**: Global shared data (`GameClock`, `GameState`, `MultiSiteManager`)
- **Events**: Cross-system communication (`ChargerFaultEvent`, `SiteSwitchEvent`)

### Game Flow

```
                          ┌───────────────────┐
┌─────────────┐     ┌─────────────┐     ┌─────────────┐
│   Loading   │────>│ CharSetup   │────>│   Playing   │<───┐
│  (default)  │     └─────────────┘     └──────┬──────┘    │
└─────────────┘                                │           │
       ^                                       v           │
┌─────────────┐     ┌─────────────┐     ┌─────────────┐    │
│  GameOver   │<────│   DayEnd    │<────│   Paused    │────┘
└─────────────┘     └─────────────┘     └─────────────┘
```

**States (`AppState` in `src/states/mod.rs`):**

| State | Description | Key Systems |
|-------|-------------|-------------|
| `Loading` (default) | Asset loading, progress bar | `update_loading_progress`, `check_loading_complete` |
| `CharacterSetup` | Character selection and name input | `character_setup_system` |
| `Playing` | Active gameplay | All game systems (gated by `is_station_open`) |
| `Paused` | Frozen state, ESC to toggle | `pause_menu_system` |
| `DayEnd` | Daily summary modal | `day_end_system` |
| `GameOver` | Win/lose screen | `game_over_system` |

> **Note**: `MainMenu` exists in code but is currently disabled. The game starts at `Loading`.

States use `OnEnter`/`OnExit` schedules for setup and cleanup.

## Module Structure

### `src/lib.rs` - Main Plugin

The `ChargeOpsPlugin` bundles all subsystems across two `add_plugins()` calls (Bevy's tuple limit is 15):

```rust
// First batch: core game systems
app.add_plugins((
    ErrorsPlugin,       // Global error handling (logs instead of panics)
    StatesPlugin,       // Game state machine
    AudioPlugin,        // Sound effects and music
    ResourcesPlugin,    // Global resources initialization
    ComponentsPlugin,   // Component registration
    EventsPlugin,       // Event types
    ObserversPlugin,    // Entity lifecycle observers
    HooksPlugin,        // Component lifecycle hooks
    HelpersPlugin,      // Utility systems (camera, debug, pointer)
    DataPlugin,         // JSON and Tiled data loading
    SystemsPlugin,      // Game logic systems
    UiPlugin,           // User interface
));

// Second batch: external integrations
app.add_plugins((
    ApiPlugin,                                          // Supabase leaderboard API
    #[cfg(feature = "ocpp")]
    OcppPlugin,                                         // OCPP 1.6J protocol (feature-gated)
    NorthstarPlugin::<CardinalNeighborhood>::default(),  // Pathfinding
    TiledPlugin::default(),                             // Tiled map rendering
));
```

---

## Components (`src/components/`)

### Charger (`charger.rs`)
- `Charger`: Main charger data (id, type, state, fault, power allocation, reliability, tier)
- `ChargerState`: `Available`, `Charging`, `Warning`, `Offline`, `Disabled`
- `ChargerType`: `DcFast`, `AcLevel2`
- `ChargerPadType`: `L2`, `DCFC50`, `DCFC100`, `DCFC150`, `DCFC350`
- `ChargerTier`: `Value`, `Standard`, `Premium` (affects MTBF, efficiency, jam rate)
- `FaultType`: `CommunicationError`, `CableDamage`, `PaymentError`, `GroundFault`, `FirmwareFault`, `CableTheft`
- `RemoteAction`: `SoftReboot`, `HardReboot`, `ReleaseConnector`, `Disable`, `Enable`

### Driver (`driver.rs`)
- `Driver`: Driver data (id, patience, charge needs, target charger, vehicle type)
- `DriverState`: `Arriving`, `WaitingForCharger`, `Queued`, `Charging`, `Frustrated`, `Complete`, `Leaving`, `LeftAngry`, `DepartingHappy`, `DepartingAngry`
- `DriverMood`: `Neutral`, `Impatient`, `Angry`, `Happy`
- `VehicleType`: `Compact`, `Sedan`, `Suv`, `Crossover`, `Pickup`, `Bus`, `Semi`, `Tractor`, `Scooter`, `Motorcycle`
- `PatienceLevel`: `VeryLow`, `Low`, `Medium`, `High`

### Power (`power.rs`)
- `Transformer`: Site transformer with thermal modeling (rating, temperature, load)
- `PhaseLoads`: Per-phase electrical load tracking (A, B, C)
- `VoltageState`: Voltage tracking

### Ticket (`ticket.rs`)
- `Ticket`: Support ticket data (type, SLA deadline, status, resolution)
- `TicketType`: `BillingDispute`, `SessionDidntStart`, `ConnectorStuck`, `SlowCharging`, `AppError`
- `TicketStatus`: `Created`, `Acknowledged`, `InProgress`, `Resolved`, `Escalated`, `Closed`
- `TicketResolution`: `Acknowledge`, `Apologize`, `PartialRefund`, `FullRefund`, `DispatchTechnician`, `Ignore`

### Technician (`technician.rs`)
- `Technician`: Repair technician entity with target charger, phase, work timer

### Site (`site.rs`)
- `BelongsToSite`: Tags entities to their owning site (critical for multi-site)
- `SiteRoot`: Root entity for each site's entity hierarchy

### Emotion (`emotion.rs`)
- `DriverEmotion`: Detailed emotional state (mood, reason, duration, speech text)
- `EmotionMood`: `VeryHappy`, `Happy`, `Neutral`, `Skeptical`, `Frustrated`, `Angry`
- `EmotionReason`: `WaitingTooLong`, `ChargingStarted`, `ChargingAlmostDone`, `FrustrationBusy`, `FrustrationDidntWork`, `FrustrationTooExpensive`, etc.
- `TechnicianEmotion`: Technician emotional state

### Traffic (`traffic.rs`)
- `VehicleFootprint`: Tile footprint for vehicles
- `VehicleTileRoute`: Tile-based route data

### Robber (`robber.rs`)
- `Robber`: Cable theft NPC entity
- `RobberPhase`: `Approaching`, `Stealing`, `Fleeing`, etc.
- `RobberVariant`: Visual variant selection

---

## Systems (`src/systems/`)

Systems are organized into ordered `GameSystemSet`s:

```rust
enum GameSystemSet {
    TimeUpdate,       // Game clock advancement
    Environment,      // Weather and environmental updates
    BuildInput,       // Build mode placement/removal
    Input,            // User input processing
    DriverSpawn,      // Driver and transformer spawning
    MovementUpdate,   // Vehicle pathfinding and movement
    ChargerUpdate,    // Charger state, faults, cooldowns
    PowerDispatch,    // Power allocation (FCFS with constraints)
    ChargingUpdate,   // Charging sessions, queue assignment
    PatienceUpdate,   // Driver patience and emotions
    TicketUpdate,     // Ticket SLA tracking
    PowerUpdate,      // Transformer temperature, phase loads
    UtilityBilling,   // Energy costs, demand charges
    ActionExecution,  // Remote actions, technician dispatch
    WinLoseCheck,     // Win/lose condition evaluation
    SpriteUpdate,     // Visual sprite updates
    UiUpdate,         // UI updates
}
```

### Key System Files

| File | Purpose |
|------|---------|
| `time.rs` | Game clock and speed control |
| `driver.rs` | Driver spawning, arrival, departure, queue assignment |
| `charger.rs` | Charger state, faults (scripted/stochastic), cooldowns, reliability |
| `power_dispatch.rs` | Power allocation with solar/BESS integration |
| `power.rs` | Transformer thermal model, phase balancing |
| `utility_billing.rs` | Energy cost and demand charge calculation |
| `ticket.rs` | Support ticket SLA tracking |
| `technician.rs` | Technician dispatch, travel, and repair |
| `actions.rs` | Remote action processing |
| `scene.rs` | Grid-to-entity synchronization, infrastructure gauges |
| `northstar_movement.rs` | bevy_northstar pathfinding integration |
| `site_switching.rs` | Multi-site switching and camera movement |
| `site_visibility.rs` | Entity visibility per site |
| `site_roots.rs` | Site root entity management |
| `ambient_traffic.rs` | Background vehicle simulation |
| `emotion.rs` | Driver emotional state evaluation |
| `win_lose.rs` | Win/lose condition checking (**currently a no-op stub**) |
| `demand_warnings.rs` | Demand charge monitoring and warning events |
| `robber.rs` | Robber spawning, movement, stealing, cleanup |
| `achievements.rs` | Achievement checking |
| `interaction.rs` | Charger click selection, keyboard shortcuts |
| `sprite.rs` | All sprite spawn/update systems (vehicles, chargers, VFX) |
| `build_input.rs` | Build mode placement and sell cursor |
| `tiled_maps.rs` | Tiled map spawning and visibility |
| `screenshot.rs` | Screenshot automation |
| `environment.rs` | Weather, temperature, news events |

---

## Resources (`src/resources/`)

### Core Game State

| Resource | Purpose |
|----------|---------|
| `GameClock` | Game time, day counter, speed (Normal=1440x, Fast=2880x) |
| `GameState` | Economy (cash, revenue), reputation, session stats, daily history |
| `MultiSiteManager` | Multi-site management (owned sites, viewed site) |
| `BuildState` | Build mode state (open/closed, placement mode) |
| `PlayerProfile` | Character selection, player name, Supabase ID |

### Per-Site State (in `SiteState`)

Each owned site maintains independent state:
- `SiteGrid`: Tile-based placement grid
- `ChargerQueue`: Driver queues per charger type
- `DriverSchedule`: Site-specific driver spawn schedule
- `UtilityMeter`: Energy cost and demand charge tracking
- `SiteEnergyConfig`: Solar/BESS configuration
- `SiteUpgrades`: O&M tier, purchased upgrades
- `ServiceStrategy`: Player strategy (pricing, power density, maintenance, amenities)
- `DemandState`: Peak demand tracking

### Global Resources

| Resource | Purpose |
|----------|---------|
| `TechnicianState` | Global technician status (travels between sites) |
| `EnvironmentState` | Weather, news events, temperature |
| `ImageAssets` / `AudioAssets` | Loaded image/sprite/sound handles |
| `SiteTemplateCache` | Parsed site template data |
| `GameDataAssets` | Loaded JSON asset handles |
| `TiledMapRegistry` | Tiled map tracking |
| `TutorialState` | Tutorial step tracking |
| `AchievementState` | Achievement system |
| `LeaderboardData` | Leaderboard data |
| `UnitSystem` | Metric/imperial display toggle |
| `CarbonCreditMarket` | Carbon credit trading |
| `DailyRobberyTracker` | Robbery event tracking |

---

## Events (`src/events/`)

Events use Bevy 0.17's `Event` derive for cross-system communication:

### Charger Events
- `ChargerFaultEvent`: Fault occurred on charger
- `ChargerFaultResolvedEvent`: Fault was resolved

### Driver Events
- `DriverArrivedEvent`: Driver entered site
- `DriverLeftEvent`: Driver departed (with mood/revenue)
- `ChargingCompleteEvent`: Session finished

### Ticket Events
- `TicketCreatedEvent`: New support ticket
- `TicketResolvedEvent`: Ticket resolved
- `TicketEscalatedEvent`: Ticket SLA breached

### Economy Events
- `CashChangedEvent`: Money added/removed
- `ReputationChangedEvent`: Reputation changed

### Site Events (`events/site.rs`)
- `SiteSwitchEvent`: Player switched viewed site
- `SiteSoldEvent`: Site was sold

### Demand Events (`events/demand.rs`)
- `PeakIncreasedEvent`: New peak demand recorded
- `PeakRiskEvent`: Approaching peak threshold (defined but not currently emitted)
- `BessSavedPeakEvent`: Battery prevented peak increase (defined but not currently emitted)
- `BessLowSocEvent`: Battery SOC low during high load (defined but not currently emitted)
- `DemandBurdenEvent`: High demand burden warning (the only demand event actively emitted)

### Action Events
- `RemoteActionRequestEvent`: Remote action requested
- `RemoteActionResultEvent`: Remote action result

### Game Events
- `GameEndedEvent`: Game over triggered
- `TransformerWarningEvent`: Transformer temperature warning
- `SpeedChangedEvent`: Game speed changed
- `ShowTooltipEvent` / `HideTooltipEvent`: Tooltip display

### Technician Events
- `TechnicianDispatchEvent`: Technician dispatched
- `RepairCompleteEvent`: Repair finished
- `RepairFailedEvent`: Repair failed

### Other Events
- `AchievementUnlockedEvent`: Achievement unlocked
- `PlaySfx`: Sound effect trigger (`src/audio.rs`)

---

## Observers (`src/observers/`)

Observers provide entity-targeted event handling:

```rust
// Entity events (triggered on specific entities)
ChargerFaulted      // When a charger develops a fault
ChargerRepaired     // When a fault is resolved
RemoteActionPerformed // When a remote action is taken
ChargingStarted     // When a charging session begins
ChargingCompleted   // When a charging session ends
TicketOpened        // When a ticket is created
TicketClosed        // When a ticket is resolved
TicketEscalated     // When a ticket SLA is breached
DriverArrived       // When a driver arrives
DriverDeparted      // When a driver leaves
```

Key observers:
- `on_charger_fault_global`: Creates tickets, updates stats
- `on_charger_repair_global`: Logs repairs, updates stats
- `on_ticket_created_global`: Initializes ticket tracking
- `on_ticket_resolved_global`: Updates reputation, revenue

---

## Hooks (`src/hooks/`)

Component lifecycle hooks maintain data integrity:

### ChargerIndex (`charger_hooks.rs`)
Maintains a fast lookup from charger ID to entity:
- `on_charger_added`: Adds to index when charger spawns
- `on_charger_removed`: Removes from index when charger despawns

### Transformer Hooks (`power_hooks.rs`)
- `on_transformer_added`: Initializes transformer tracking

---

## UI (`src/ui/`)

UI modules organized by feature:

| Module | Purpose |
|--------|---------|
| `hud.rs` | Top bar (cash, time, reputation, speed controls) |
| `top_nav.rs` | Panel switcher tabs, weather display, news ticker |
| `sidebar/` | Unified sidebar with multiple panels |
| `sidebar/build_panels.rs` | Build mode panels (charger placement) |
| `sidebar/strategy_panels.rs` | Strategy panels (pricing, power, OpEx) |
| `sidebar/operations_panel.rs` | Operations panel |
| `sidebar/rent_panel.rs` | Site rental panel |
| `sidebar/start_day.rs` | Start Day button |
| `sidebar/power_panel_inline.rs` | Inline power/demand display |
| `power_panel.rs` | Floating power grid visualization |
| `radial_menu.rs` | Charger selection radial menu |
| `site_tabs.rs` | Multi-site tab switcher |
| `speech_bubbles.rs` | Driver/technician dialogue bubbles |
| `toast.rs` | Notification toasts |
| `demand_toasts.rs` | Demand charge warning toasts |
| `template_picker.rs` | Site selection carousel |
| `overlay.rs` | Game info overlay |
| `tutorial.rs` | Interactive tutorial system |
| `achievement_modal.rs` | Achievement display modal |
| `leaderboard_modal.rs` | Leaderboard display modal |
| `leaderboard_systems.rs` | Leaderboard data fetching/submission |

---

## Pathfinding (`bevy_northstar`)

Vehicle movement uses `bevy_northstar` for pathfinding:

### Integration
- `src/resources/northstar_grid.rs`: Converts `SiteGrid` to pathfinding grid
- `src/systems/northstar_movement.rs`: Movement systems

### Key Systems
- `northstar_move_vehicles`: Smooth visual interpolation
- `northstar_arrival_detection`: Detect when vehicles reach targets
- `northstar_trigger_departure`: Initiate departure pathfinding
- `northstar_cleanup_exited`: Remove vehicles that left the site
- `northstar_cleanup_ambient`: Remove ambient vehicles that exited
- `northstar_handle_pathfinding_failed`: Handle blocked paths (with cooldown)
- `northstar_handle_reroute_failed`: Handle reroute failures (with cooldown)
- `northstar_clear_cooldown_on_success`: Clear cooldowns on success

### Grid Management
- Each site has its own pathfinding grid
- Grids are rebuilt when chargers/structures are placed
- `rebuild_site_pathfinding_grids`: Updates nav data on grid changes
- `transfer_pathfinding_grid_on_site_switch`: Swaps active grid on site switch

---

## Multi-Site Architecture

### Entity Tagging Pattern

All site-specific entities are tagged with `BelongsToSite`:

```rust
commands.spawn((
    Charger { ... },
    BelongsToSite::new(site_id),
    Transform::from_xyz(...),
    Visibility::default(),
));
```

### Concurrent Operation

All owned sites simulate concurrently:
- Power systems process all sites each frame
- Drivers spawn and charge at their respective sites
- Revenue accrues to global `GameState`
- Only the "viewed" site is rendered

### Site Switching

```
User clicks site tab
    ↓
SiteSwitchEvent fired
    ↓
handle_site_switch()
  - Updates viewed_site_id
    ↓
update_site_entity_visibility()
  - Shows entities matching viewed site
  - Hides other site entities
    ↓
update_camera_for_site()
  - Pans camera to site world position
```

### Spatial Separation

Sites are positioned 2000px apart in world space:
```rust
const SITE_SPACING: f32 = 2000.0;

// Method on SiteState
pub fn world_offset(&self) -> Vec2 {
    Vec2::new(self.id.0 as f32 * SITE_SPACING, 0.0)
}
```

---

## Data Flow

### Gameplay Loop (per frame)

1. **TimeUpdate**: Advance game clock
2. **Environment**: Update weather and news events
3. **BuildInput**: Process build mode placement/removal
4. **Input**: Process user input
5. **DriverSpawn**: Create new customers from schedule
6. **MovementUpdate**: Pathfind and animate vehicles
7. **ChargerUpdate**: Process faults, cooldowns, reliability
8. **PowerDispatch**: Allocate power within constraints
9. **ChargingUpdate**: Deliver energy, collect revenue
10. **PatienceUpdate**: Track driver moods and emotions
11. **TicketUpdate**: Check SLA deadlines
12. **PowerUpdate**: Update transformer temperature
13. **UtilityBilling**: Calculate energy/demand costs
14. **ActionExecution**: Process remote actions
15. **WinLoseCheck**: Evaluate end conditions (currently no-op)
16. **SpriteUpdate**: Update visuals
17. **UiUpdate**: Refresh UI

### Event Flow Example

```
Charger develops fault
    ↓
Observer: on_charger_fault_global
  ├── Creates Ticket entity
  └── Fires ChargerFaultEvent
    ↓
UI systems receive event
  └── Spawn toast notification
```

---

## Level Rendering (`src/data/`, `bevy_ecs_tiled`)

### Tiled-Based Architecture

Levels are designed in Tiled and rendered using a hybrid approach:

- **bevy_ecs_tiled** renders all base tiles (terrain, walls, parking, decorations) from TMX files
- **Entity overlays** handle infrastructure with dynamic state (transformers, solar, batteries, chargers)
- **SiteGrid** maintains gameplay state (pathfinding, placement validation) initialized from TMX

### Data Files

| Path | Purpose |
|------|---------|
| `assets/maps/01_first_street.tmx` | Level 1 Tiled map |
| `assets/maps/02_quick_charge_express.tmx` | Level 2 Tiled map |
| `assets/maps/03_central_fleet_plaza.tmx` | Level 3 Tiled map |
| `assets/data/chargers/mvp_chargers.chargers.json` | Charger definitions |
| `assets/data/scenarios/mvp_drivers.scenario.json` | Driver schedules |

### Loading Flow

1. `Loading` state entered
2. `start_asset_loading`: Queue JSON and TMX assets
3. `populate_template_cache`: Parse site templates from TMX properties
4. `populate_game_data_from_assets`: Extract charger/driver data
5. `check_loading_complete`: Transition to `CharacterSetup`

---

## Additional Modules

### Audio (`src/audio.rs`)
- `AudioPlugin`: Sound effects via `PlaySfx` event
- `SoundEnabled` resource: Toggle sounds on/off

### API (`src/api/`)
- `ApiPlugin`: Supabase integration for leaderboards
- `SupabaseConfig` resource: API configuration
- `leaderboard.rs`: Fetch and submit scores

### OCPP (`src/ocpp/`, feature-gated)
- `OcppPlugin`: OCPP 1.6J protocol support
- Generates OCPP messages from game events
- WebSocket streaming to Central System
- Optional disk writer for native builds

---

## Performance Considerations

- Systems run in parallel where data dependencies allow
- Site filtering uses runtime `BelongsToSite` checks (efficient for <1000 entities)
- Visibility system hides non-viewed sites (no rendering cost)
- Change detection optimizes queries (`Changed<T>`, `Added<T>`)
- State-gated systems avoid unnecessary work

---

## Extending the Game

### Adding a New Charger Type

1. Add variant to `ChargerPadType` enum in `charger.rs`
2. Update display name and pricing methods
3. Add sprite loading in `asset_handles.rs`
4. Update `ServiceStrategy` pricing

### Adding a New Fault Type

1. Add variant to `FaultType` enum
2. Update `FaultType::display_name()` and repair cost/duration
3. Map to `TicketType` in `TicketType::from_fault()`
4. Add handling in `actions.rs`

### Adding a New Site Type

1. Add variant to `SiteArchetype` enum in `multi_site.rs`
2. Update display methods and site-specific parameters
3. Create TMX map file in `assets/maps/`
4. Update site tab colors in `site_tabs.rs`

### Adding a New UI Panel

1. Create module in `src/ui/`
2. Add setup system with state condition
3. Add update systems to `UiPlugin`
4. Export from `src/ui/mod.rs`
