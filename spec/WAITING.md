# Waiting Tile System

Cars that arrive when all parking bays are occupied should wait on a nearby
driveable tile instead of driving through. When a charger frees up the
waiting car pathfinds to the now-available bay and begins charging.

---

## Current behaviour

```
Spawn at entry
  |
  +-- Bay available? --yes--> Pathfind to bay --> Arrive --> Charge
  |
  +-- No free bay ---------> Drive-through to exit (lost customer)
```

1. `driver_spawn_system` builds `occupied_bays` from active drivers, then
   filters `get_charger_bays()` against it.
2. When `available_compatible` is empty the car is spawned with
   `MovementPhase::DepartingHappy`, pathfinds to the exit, and is gone.
   (see `src/systems/driver.rs` lines 178-249 for the scheduled path and
   lines 436-506 for the procedural path.)
3. If a car *does* get a bay but the charger is busy on arrival, it joins
   `ChargerQueue` in `DriverState::Queued` but physically stays parked in
   the bay, blocking it for future arrivals.

Problems:
- No queueing means lost revenue when the lot is momentarily full.
- Queued cars sit in bays, reducing available bays for new arrivals.

---

## Target behaviour

```
Spawn at entry
  |
  +-- Bay available? --yes--> Pathfind to bay --> Arrive --> Charge
  |
  +-- No free bay
        |
        +-- Waiting tile found? --yes--> Pathfind to tile --> Park --> Queue
        |                                                        |
        |                        Charger frees up <--------------+
        |                              |
        |                        Assign bay + re-pathfind --> Arrive --> Charge
        |
        +-- No tile found -----> Drive-through (fallback)
```

---

## Files to change

| File | Change |
|------|--------|
| `src/resources/site_grid.rs` | Add `find_waiting_tile` BFS method |
| `src/components/driver.rs` | Add `waiting_tile` field to `Driver` |
| `src/systems/driver.rs` | Spawn to waiting tile; arrival at waiting tile; queue assignment re-pathfinding |
| `src/systems/northstar_movement.rs` | Arrival detection for waiting tile |

---

## Implementation rules

These are hard constraints for the implementation so behavior stays consistent
with current systems:

1. **No reservation model**
   - Do not add charger reservation state/fields.
   - Do not mark chargers busy until a driver actually reaches a bay and starts charging.
2. **FCFS queue discipline**
   - Keep first-in-first-out ordering in existing `ChargerQueue` lanes.
   - Only promote a queued driver if they are still `DriverState::Queued` and still present.
3. **Patience decides waiting lifetime**
   - Do not add custom waiting timers.
   - Existing patience depletion remains the single mechanism that ejects long waiters.
4. **Pathfinding-first movement**
   - All movement transitions must continue to use `Pathfind` + `AgentPos` + `NextPos`.
   - Do not add custom per-frame transform interpolation for waiting behavior.
5. **Non-destructive fallback**
   - If no waiting tile exists, preserve current drive-through behavior unchanged.

---

## Pathfinding integration rules (bevy_northstar)

To fit cleanly with existing movement systems in `src/systems/northstar_movement.rs`:

1. **Reuse existing components exactly**
   - Spawn to waiting tile by inserting `Pathfind::new_2d(wait_x, wait_y)`.
   - Re-path to bay by re-inserting `Pathfind::new_2d(bay_x, bay_y)`.
2. **Preserve movement phases**
   - Set `MovementPhase::Arriving` when routing either to waiting tile or to bay.
   - Arrival detection (`northstar_arrival_detection`) is the only place that flips to `Parked`.
3. **Leverage existing failure handlers**
   - Do not bypass `northstar_handle_pathfinding_failed` or `northstar_handle_reroute_failed`.
   - Waiting routes inherit current cooldown and forced-exit behavior automatically.
4. **Keep departure unchanged**
   - `northstar_trigger_departure` remains source of truth for exit routing.
   - Waiting drivers who lose patience still leave via current departure flow.
5. **System ordering contract**
   - Queue assignment should only create a route (Arriving + Pathfind).
   - Actual session start must remain in `driver_arrival_system` after the car parks at bay.

---

## 1. `SiteGrid::find_waiting_tile`

**File:** `src/resources/site_grid.rs` (add after `get_charger_bays` ~line 1211)

Find the nearest driveable waiting tile close to the charging area in
general (not tied to a specific charger). Uses a bounded BFS (max 5 tiles)
expanding outward from all charger-bay positions.

### Tile priority

1. **Best:** `Lot` — off the main road, adjacent to charger area.
2. **Acceptable:** other `is_driveable()` tiles that are NOT:
   - `ParkingBayNorth` / `ParkingBaySouth` (reserved for charging)
   - `Entry` / `Exit` (must stay clear for traffic)
   - `Road` (avoid blocking the public road; accept only as last resort)
3. **Excluded:** tiles in `occupied_waiting` (already taken by another
   waiting car) or `occupied_bays` (bay with a driver assigned).

### Algorithm

```text
BFS seeds = all charger bay positions
visited = HashSet
lot_candidate = None     (first Lot tile found)
other_candidate = None   (first non-Lot driveable tile, excl. Road)
road_candidate = None    (Road as absolute last resort)

while queue not empty AND depth <= MAX_WAIT_DISTANCE (5):
    (x, y) = dequeue
    for each cardinal neighbour (nx, ny):
        skip if out of bounds or visited
        skip if in occupied_waiting or occupied_bays
        content = get_content(nx, ny)
        if content == Lot           -> set lot_candidate if unset
        elif content is_driveable()
             AND NOT is_parking()
             AND NOT Entry/Exit     -> set other_candidate if unset
        elif content == Road        -> set road_candidate if unset
        push (nx, ny) to queue

return lot_candidate.or(other_candidate).or(road_candidate)
```

### Signature

```rust
pub fn find_waiting_tile(
    &self,
    occupied_waiting: &[(i32, i32)],
    occupied_bays: &[(i32, i32)],
) -> Option<(i32, i32)>
```

This helper is intentionally charger-agnostic. It only picks a good staging
tile; actual charger selection still happens later through normal queue logic.

---

## 2. `Driver.waiting_tile` field

**File:** `src/components/driver.rs`

Add one field to the `Driver` struct (line 166, after `assigned_bay`):

```rust
pub assigned_bay: Option<(i32, i32)>,
pub waiting_tile: Option<(i32, i32)>,   // <-- NEW
```

And in `Default`:

```rust
assigned_bay: None,
waiting_tile: None,   // <-- NEW
```

This field distinguishes three cases at `MovementPhase::Parked`:

| `assigned_bay` | `waiting_tile` | Meaning |
|----------------|----------------|---------|
| `Some` | `None` | Normal: parked at charger bay |
| `None` | `Some` | New: parked at waiting tile, in queue |
| `None` | `None` | Drive-through (shouldn't park) |

---

## 3. Modified `driver_spawn_system`

**File:** `src/systems/driver.rs`

### a) Collect occupied waiting tiles (alongside `occupied_bays`)

```rust
let occupied_waiting: Vec<(i32, i32)> = existing_drivers
    .iter()
    .filter(|(_, b, _)| b.site_id == *site_id)
    .filter(|(d, _, _)| !matches!(
        d.state,
        DriverState::Leaving | DriverState::LeftAngry | DriverState::Complete
    ))
    .filter_map(|(d, _, _)| d.waiting_tile)
    .collect();
```

### b) Replace drive-through block with waiting-tile attempt

Where the code currently says `if available_compatible.is_empty() { /* drive-through */ }`,
insert a waiting-tile attempt **before** the drive-through fallback.

This must happen in **both** the scheduled path (~line 178) and the
procedural path (~line 436). The logic is identical:

```rust
if available_compatible.is_empty() {
    // --- NEW: try to find a waiting tile ---
    let wait_tile = site_state
        .grid
        .find_waiting_tile(&occupied_waiting, &occupied_bays);

    if let Some((wx, wy)) = wait_tile {
        // Spawn heading to waiting tile instead of driving through
        let driver = Driver {
            // ...same fields as normal spawn...
            assigned_bay: None,          // no bay yet
            waiting_tile: Some((wx, wy)),
            state: DriverState::Arriving,
            ..default()
        };

        let pathfind = Pathfind::new_2d(wx as u32, wy as u32);

        commands.entity(root_entity).with_children(|parent| {
            parent.spawn((
                driver, movement, footprint,
                agent_pos, pathfind,
                Blocking,   // collision avoidance while driving
                Transform::from_translation(pos),
                GlobalTransform::default(),
                Visibility::default(),
                BelongsToSite::new(*site_id),
            ));
        });

        // track this tile as occupied for remaining spawns this frame
        occupied_waiting.push((wx, wy));
        // advance schedule / mark spawned
        ...
        break;
    }

    // --- EXISTING: drive-through fallback (unchanged) ---
    // 20% chance to leave angry ...
}
```

Key details:
- `Blocking` is added so the car participates in collision avoidance.
- `occupied_waiting` is pushed to so a second car this frame won't pick
  the same tile.
- `DriverArrivedEvent` is emitted as usual.

---

## 4. Modified `northstar_arrival_detection`

**File:** `src/systems/northstar_movement.rs` (line 165-174)

The existing arrival check only looks at `assigned_bay`. Add a second arm
for waiting tiles:

```rust
MovementPhase::Arriving => {
    if let Some((bay_x, bay_y)) = driver.assigned_bay {
        // existing: check arrival at bay
        let at_bay = agent_pos.0.x == bay_x as u32 && agent_pos.0.y == bay_y as u32;
        if at_bay {
            movement.phase = MovementPhase::Parked;
            info!("Vehicle {} arrived at bay and parked", driver.id);
        }
    } else if let Some((wx, wy)) = driver.waiting_tile {
        // NEW: check arrival at waiting tile
        let at_wait = agent_pos.0.x == wx as u32 && agent_pos.0.y == wy as u32;
        if at_wait {
            movement.phase = MovementPhase::Parked;
            info!("Vehicle {} arrived at waiting tile ({}, {}) and parked", driver.id, wx, wy);
        }
    }
}
```

---

## 5. Modified `driver_arrival_system`

**File:** `src/systems/driver.rs` (line 615-750)

Currently the system only processes drivers with
`state == Arriving && phase == Parked`. We need to handle two cases:

### Case A: Parked at a bay (existing logic, no change)

`driver.assigned_bay.is_some()` — resolve charger, start charging or queue.

### Case B: Parked at a waiting tile (new)

`driver.assigned_bay.is_none() && driver.waiting_tile.is_some()`:

```rust
if driver.waiting_tile.is_some() && driver.assigned_bay.is_none() {
    // At waiting tile - join queue for compatible charger types
    driver.state = DriverState::Queued;
    for charger_type in driver.vehicle_type.compatible_charger_types() {
        match charger_type {
            ChargerType::DcFast => site_state.charger_queue.join_dcfc_queue(driver_entity),
            ChargerType::AcLevel2 => site_state.charger_queue.join_l2_queue(driver_entity),
        }
    }
    info!("Driver {} parked at waiting tile, joined queue", driver.id);
    continue;  // skip the charger-lookup logic
}
```

Insert this check **before** the existing charger-resolution block so
waiting-tile drivers don't fall through into the "no charger assigned"
path that also queues but doesn't expect a waiting tile.

---

## 6. Modified `queue_assignment_system`

**File:** `src/systems/driver.rs` (line 754-837)

No reservation model: the system should always serve the next queued driver
that still has patience and can physically move into a currently free bay.
It should not reserve a charger while a driver is still traveling from a
waiting tile.

### System signature changes

The system needs `Commands`, `VehicleMovement`, and queue-order-first logic:

```rust
pub fn queue_assignment_system(
    mut commands: Commands,        // NEW
    mut driver_move_q: Query<(
        Entity,
        &mut Driver,
        &mut VehicleMovement,      // NEW
        &crate::components::BelongsToSite,
    )>,
    chargers: Query<(&Charger, &crate::components::BelongsToSite)>,
    all_drivers: Query<(&Driver, &crate::components::BelongsToSite)>,  // NEW: occupancy view
    mut multi_site: ResMut<crate::resources::MultiSiteManager>,
    tech_state: Res<TechnicianState>,  // still used to exclude chargers under repair
)
```

Implementation note: avoid Bevy borrow conflicts by using `ParamSet` if both
mutable and immutable `Driver` views are required in the same system.

### Assignment logic

```rust
// Queue-driven (FCFS) flow:
// 1) Look at queue front (per charger class as today).
// 2) If front driver still Queued and patience > 0, try to find any currently free compatible bay.
// 3) If found, move that driver toward the bay (Arriving + Pathfind), remove from queues.
// 4) Do NOT set charger.is_charging and DO NOT reserve charger here.
// 5) Charging only starts when driver physically arrives at bay in driver_arrival_system.
// 6) If front driver cannot be moved this tick (no reachable bay), do not reorder queue.
```

Important: no reservation is introduced. If a driver cannot be moved to a bay
yet, they remain queued and keep waiting as long as patience lasts.

---

## 7. Edge cases

### Patience while at waiting tile

No change needed. `patience_system` already drains patience for
`DriverState::Queued`. If patience hits zero the driver transitions to
`LeftAngry` and `northstar_trigger_departure` handles exit pathfinding.

### Day-ending while at waiting tile

Drivers in `Queued` state are already kicked by `day_ending_system`.
The departure system handles pathfinding to exit regardless of whether
the driver has `assigned_bay` or `waiting_tile`.

### Waiting tile blocked by another vehicle

The BFS skips tiles in `occupied_waiting` and `occupied_bays`. If a
pathfind to the waiting tile fails, the existing `PathfindCooldown` retry
and `MAX_STUCK_TIME` forced-exit logic applies.

### Multiple charger types

`find_waiting_tile` is not scoped to charger type or charger id. Vehicles wait
in a shared staging area and queue logic later picks an available compatible
bay in FCFS order.

### Max waiting capacity

As a natural limit: once all Lot tiles near chargers are occupied by
waiting cars, `find_waiting_tile` returns `None` and the fallback
drive-through behaviour kicks in. No explicit cap is needed.

---

## Summary of state transitions

```
                              +---------+
                              | Spawned |
                              +----+----+
                                   |
                      +------------+------------+
                      |                         |
               Bay available              No free bay
                      |                         |
                      v                         v
             Arriving (to bay)      Arriving (to waiting tile)
                      |                         |
                      v                         v
               Parked at bay           Parked at waiting tile
                      |                         |
                      v                         v
             Resolve charger              DriverState::Queued
              (existing flow)                   |
                      |                  Charger frees up
                      |                         |
                      |                  Assign bay + re-pathfind
                      |                         |
                      |                  Arriving (to bay)
                      |                         |
                      |                  Parked at bay
                      |                         |
                      v                         v
                  Charging  <-------------------+
                      |
                      v
                  Complete --> Leaving --> Exited
```
