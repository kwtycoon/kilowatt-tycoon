# Vehicle Flow

## Movement system

Vehicles use **bevy_northstar** for tile-based pathfinding on a
`CardinalGrid` (4-direction, no diagonals).

```
Spawn → AgentPos + Pathfind(goal)
    → northstar computes Path (sequence of tiles)
    → each frame: NextPos assigned → vehicle interpolates toward it
    → arrives at NextPos → AgentPos updated → NextPos removed → repeat
    → reaches goal → Parked / Exited
```

## The `Blocking` component

When an entity has `Blocking`, northstar registers it in `BlockingMap`
at its `AgentPos`.  Before giving another blocking agent a `NextPos`,
northstar checks whether that tile is occupied.  If it is, northstar
tries a local reroute within `avoidance_distance(5)`.  If reroute
fails → `RerouteFailed`.

**Blocking must stay on all vehicles.**  Without it, two vehicles can
occupy the same tile, which looks wrong -- in real life they'd collide.

## Road layout

```
  ┌─────────────────────────────────────────────┐
  │  RoadLaneTop   (center-line at bottom edge) │  upper lane
  ├─────────────────────────────────────────────┤
  │  RoadLaneBottom (center-line at top edge)   │  drive lane
  │  E ─────────────────────────────────────► X │  entry/exit here
  └─────────────────────────────────────────────┘
        ↓ vehicles turn off road into lot
```

Roads are 2 tiles wide.  Entry and exit sit on the bottom (drive)
lane -- right-hand traffic.  The upper lane is the passing/oncoming
lane.

## How flow works with Blocking + 2-tile road

Vehicles moving the same direction at similar speeds naturally form a
**convoy**: vehicle B gets a `NextPos` of the tile vehicle A just
vacated.  They follow each other single-file without overlap, like
cars on a highway.

If the lead vehicle stalls for any reason, the vehicle behind it
reroutes to the other lane (within `avoidance_distance(5)`) and
continues.  This is why the road must be 2 tiles wide -- a 1-tile
road has zero reroute options, causing `RerouteFailed` cascades.

The "2-wide wall" scenario (two blocking vehicles side-by-side at the
same x-coordinate) is:

- **Rare** -- vehicles spawn one-per-frame at a single entry tile, so
  they naturally stagger
- **Transient** -- both vehicles are moving, so it clears in ~0.6s
- **Not fatal** -- the vehicle behind waits one step, unlike the old
  permanent gridlock on a 1-tile road

## Entry gate

The spawn system checks whether the entry tile and adjacent road tiles
are occupied before spawning a new vehicle.  This uses an entity-query
scan of cardinal neighbors that are `is_public_road()`, ensuring
vehicles are spaced out at the entry.

## Lifecycle summary

```
Phase       │ Blocking │ Speed     │ Notes
────────────┼──────────┼───────────┼──────────────────────────
Arriving    │ YES      │ ~100 px/s │ Convoy flow on 2-tile road
Parked      │ YES      │ 0         │ Northstar routes around bay
Departing   │ NO       │ ~180 px/s │ Removed to avoid exit deadlock
```

Departing vehicles have `Blocking` removed so they don't deadlock
against arriving traffic on the shared road.  Since they're heading
to the exit and won't stop, overlap with other departing vehicles is
brief and acceptable.

## What we changed (and why)

| Change | Why |
|--------|-----|
| 2-tile road (TMX + SiteGrid) | Gives northstar a reroute lane when a vehicle stalls |
| Distinct lane tiles (top/bottom) | Visual center-line between lanes |
| Entry/exit on bottom lane | Right-hand traffic, vehicles drive on correct side |
| Entry check from tile content | Detects adjacent road tiles dynamically, not hardcoded |
