# Technician And Multi-Site Invariants

This document captures the current invariants for multi-site day rollover, technician dispatch, and site-sale cleanup.

It exists to make the fault-repair pipeline easier to reason about and to keep future refactors from reintroducing view-coupled bugs.

## Things To Watch Out For

This area is easy to break because the visible technician flow is not the same thing as the authoritative technician state.

In particular:

- `TechnicianState` and `RepairRequestRegistry` are authoritative; the on-screen technician avatar is only a view-scoped projection of that state
- only the viewed site has a live Northstar grid, so technician execution and priority rules are intentionally view-coupled
- some invariants are enforced by schedule wiring and run conditions, not just by local logic inside one technician system
- site sale cleanup is event-driven through `SiteSoldEvent`, so alternate sale paths must emit the same event or they can bypass required cleanup

When changing this area, check the logical state, the viewed-site avatar path, and the schedule/event path together. Most regressions happen when only one of those layers is updated.

## Scope

There are three different kinds of state in this area of the game:

- Global state: one logical technician, `GameClock`, `GameState`, and the `RepairRequestRegistry`
- Per-site state: each `SiteState` inside `MultiSiteManager.owned_sites`
- Viewed-site-only presentation state: the single active pathfinding grid and the on-screen technician avatar

The important rule is that most game state remains durable across all sites, but technician execution is intentionally coupled to the currently viewed site. The viewed site controls the live world representation, the active pathfinding grid, and which site's technician work gets priority when multiple sites need service.

## Day Boundary Model

Day-end uses two phases:

1. `prepare_day_end_report()` reads all owned sites, aggregates report inputs, and flushes ledger-backed per-site costs.
2. `on_exit_day_end()` advances the calendar and resets every owned site for the next day.

### What is aggregated at day end

- Carbon credit revenue is summed across all owned sites
- Zero-grid-day achievement is evaluated from all owned sites
- Utility, maintenance, amenity, and warranty costs are flushed from all owned sites

### What resets for every site

`MultiSiteManager::reset_all_sites_for_new_day()` is the single reset entry point for per-site daily state. It currently resets:

- `charger_queue`
- `utility_meter`
- `grid_events`
- `driver_schedule.next_driver_index`
- `driver_schedule.next_event_index`
- `energy_delivered_kwh_today`
- `sessions_today`

### Ledger-drain invariant

`GameState::flush_site_costs()` must drain, not just read, per-site cost accumulators.

After `flush_site_costs()` runs:

- `utility_meter` is reset
- `pending_maintenance` is `0`
- `pending_amenity` is `0`
- `pending_warranty` is `0`

This prevents offscreen sites from posting the same day’s costs twice.

## Technician Model

The technician has two layers:

- Logical state: `TechnicianState` and `RepairRequestRegistry`
- View state: spawned `Technician` entity plus Bevy Northstar pathfinding components

### Logical state is authoritative

`TechnicianState` is the source of truth for:

- current logical location
- active request
- queued requests
- travel time
- repair time
- leaving-site state

`RepairRequestRegistry` is the durable work-order layer for charger faults that require a technician.

### Repair-request reconciliation rule

`RepairRequestRegistry` is not just a queue backing store. It is continuously reconciled against live charger fault state.

That means:

- technician-required faults must have an open repair request
- faults that no longer exist, no longer require a technician, or point at despawned chargers must resolve or cancel their request
- active technician work must still match an open request, the same charger, the same `site_id`, and a still-live technician-required fault
- queued dispatches that become invalid must be resolved rather than silently left dangling

This keeps the logical technician pipeline aligned with the actual charger world state.

### View state gates technician repair

The game only supports one live Northstar grid at a time, attached to the viewed site.

Because of that:

- En-route travel is always logical and view-independent
- Offscreen arrival stops at a waiting state and does not start repair work
- `TechStatus::WaitingAtSite` means the technician has reached the site but still needs a visible on-site walk
- `TechStatus::WalkingOnSite` is only valid while the viewed site can host a live pathfinding avatar
- A repair may enter `Repairing` only from `technician_arrival_detection()` once the visible technician reaches `target_bay`
- If the active repair site becomes viewed while a job is paused or was previously offscreen, the technician must be reconstructed as a walking avatar from a valid origin instead of spawning directly into `Working`
- If the active repair site is no longer viewed, the avatar can be despawned, but repair progress must pause until visible at-charger presence is re-established

### Visible-site priority rule

The single technician is global, but the currently viewed site gets dispatch priority.

That means:

- if the viewed site has technician-required work queued, it should start before older queued work on other sites
- if the technician is already en route to or waiting at an offscreen site, viewed-site work may preempt that offscreen job
- preempted offscreen work must be parked back into the durable queue/request layer rather than dropped
- preemption must preserve request identity and dispatch-cost billing history so resuming later does not double-bill
- offscreen `WaitingAtSite` work must never monopolize the technician when the viewed site has pending technician work

### Build-phase and day-end rule

Physical technician avatar movement should not advance during the closed/build phase.

Logical technician progress follows the normal simulation gating:

- queued requests persist across days
- active technician runtime state does not persist across day end; day-end reset clears the technician and normalizes open requests back to re-dispatchable states
- travel and repair timers only advance while the station is open, because technician action systems run inside the open-station simulation schedule
- visible on-site technician movement is separately blocked by `BuildState.is_open`
- once the station opens, progress resumes

### Leaving-site rule

Leaving-site is treated as a blocking transition before the next queued job starts.

There are two legal ways to exit that state:

- normal path: the viewed-site avatar walks to the exit and `cleanup_exited_technicians()` returns the technician to idle
- orphan recovery path: if the technician is logically `LeavingSite` but no avatar exists, `recover_orphaned_leaving_technician_system()` returns the technician to idle and starts the next queued job

This prevents hidden deadlocks when the logical job completes without a matching avatar.

## Site Sale Transaction

Selling a site must be treated as a data cleanup operation, not just an entity despawn.

`SiteSoldEvent` is the cleanup trigger for both normal gameplay and screenshot automation. The cleanup chain must invalidate technician and repair-request state for the sold `site_id` before despawning the site root hierarchy.

Current sale cleanup requirements:

- cancel open `RepairRequest`s for the sold site
- remove queued technician dispatches for the sold site
- clear `TechnicianState.current_site_id` if it points at the sold site
- abort active technician work targeting or leaving the sold site
- despawn any technician avatar attached to the sold site
- only then despawn the site root hierarchy

The screenshot automation path must emit `SiteSoldEvent` too, so it goes through the same cleanup flow as normal gameplay.

## Failure And Retry Semantics

When a repair fails:

- the request is marked `NeedsRetry`
- O&M auto-dispatch can requeue the same request
- the retry must not be dropped even if the technician is still in a leaving-site transition

When same-site chaining cannot resolve a routable next charger:

- the queued request must remain pending
- the system must not silently consume the queue entry

## Test Coverage

The following regression cases are currently covered and should stay covered:

- all-site daily reset, including offscreen sites
- per-site meter draining after ledger flush
- offscreen travel completion pausing at `WaitingAtSite`
- viewed repair-site activation spawning a walking technician rather than a working technician
- repair progression and billing staying blocked without a visible technician at the charger
- same-site chaining redirecting to visible walking instead of immediate repair
- sold-site cleanup while the technician is leaving
- same-site fallback preserving queued work
- auto-redispatch after repair failure with O&M enabled

These regressions currently live primarily in `tests/day_boundary_hardening_test.rs` and `tests/charger_systems_test.rs`.

If one of these invariants changes, update both the tests and this document in the same change.
