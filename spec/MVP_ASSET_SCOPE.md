# Kilowatt Tycoon — MVP Asset Scope

> **Purpose**: Define exactly what's IN vs OUT for the first playable slice, and link every asset to a specific gameplay or UI use-case.

---

## Scope Definition

**MVP Target**: Single ParkingLot site with day-by-day gameplay, scripted driver scenarios, and $100 revenue target.

**In Scope**:
- One site environment (parking lot)
- 4 chargers (2 DCFC, 2 L2)
- ~10 drivers with vehicles
- 1 technician
- Core HUD and feedback systems

**Out of Scope (Post-MVP)**:
- Multiple sites / site selection
- Weather effects
- Day/night cycle
- Additional site types (workplace, residential, destination)
- Vehicle color variants
- Advanced props (solar canopy, battery storage visual)

---

## Asset → Use-Case Mapping

### 1. World Tiles (6 assets)

| Asset | Gameplay Use-Case |
|-------|-------------------|
| `tile_asphalt_clean.png` | Primary ground surface for parking lot; most of the map uses this |
| `tile_asphalt_lines.png` | Parking spot rows; players need to see where vehicles park |
| `tile_concrete.png` | Sidewalk / pedestrian areas; visual separation from driving areas |
| `tile_grass.png` | Site boundary / landscaping; defines edges of the playable area |
| `tile_curb_asphalt_concrete.png` | Transition between parking and sidewalk; visual polish |
| `tile_curb_asphalt_grass.png` | Transition between parking and landscaping; visual polish |

**Why these matter**: The environment needs to read as "parking lot" instantly. Players mentally model where cars can go vs. where chargers are.

---

### 2. World Decals (5 assets)

| Asset | Gameplay Use-Case |
|-------|-------------------|
| `decal_oil_stain.png` | Visual variety; breaks up monotony (P2, can ship without) |
| `decal_crack.png` | Visual variety; suggests wear (P2, can ship without) |
| `decal_arrow.png` | Traffic flow direction; helps players understand site layout |
| `decal_handicap.png` | Accessible parking indicator; regulatory realism |
| `decal_ev_parking.png` | **Critical**: Marks EV charging spots; players need to identify valid parking |

**Why these matter**: Decals add realism without adding gameplay complexity. The EV parking decal is the only P0 here.

---

### 3. Site Props (6 assets)

| Asset | Gameplay Use-Case |
|-------|-------------------|
| `prop_bollard.png` | Protects chargers from vehicle collision; visual indicator of charger zones |
| `prop_wheel_stop.png` | Parking spot definition; shows where vehicles stop |
| `prop_light_pole.png` | Visual landmark; helps players orient on the map |
| `prop_utility_cabinet.png` | **Critical**: Represents the site's electrical infrastructure; clicking shows power status |
| `prop_trash_can.png` | Visual variety (P2, can ship without) |
| `prop_transformer.png` | **Critical**: Visual representation of transformer thermal state; player monitors this |

**Why these matter**: Props ground the simulation in physical reality. The transformer prop is directly tied to the grid thermal system — it must show hot/critical states.

---

### 4. Chargers — L2 (7 assets)

| Asset | Gameplay Use-Case |
|-------|-------------------|
| `charger_l2_base.png` | Compositing base (may not be used directly) |
| `charger_l2_available.png` | **Critical**: Shows charger is ready; green status light visible |
| `charger_l2_charging.png` | **Critical**: Shows active session; blue status light; player knows it's earning revenue |
| `charger_l2_warning.png` | **Critical**: Shows degraded state; yellow; player knows intervention may be needed |
| `charger_l2_offline.png` | **Critical**: Shows fault; red; player must act to restore revenue |
| `charger_l2_cable_connected.png` | Shows cable plugged into vehicle; visual feedback during session |
| `charger_l2_cable_stuck.png` | **Critical**: Shows "connector stuck" incident; player sees the problem visually |

**Why these matter**: Charger states are the core readability of the game. Players scan the map for color-coded status lights to prioritize action.

---

### 5. Chargers — DCFC (7 assets)

| Asset | Gameplay Use-Case |
|-------|-------------------|
| `charger_dcfc_base.png` | Compositing base (may not be used directly) |
| `charger_dcfc_available.png` | **Critical**: Same as L2; larger footprint, higher stakes |
| `charger_dcfc_charging.png` | **Critical**: High-revenue session in progress; worth more attention |
| `charger_dcfc_warning.png` | **Critical**: Degraded DC charger = significant revenue risk |
| `charger_dcfc_offline.png` | **Critical**: DC charger down = major incident; high priority |
| `charger_dcfc_cable_connected.png` | Visual feedback for active session |
| `charger_dcfc_cable_stuck.png` | **Critical**: DC cable issues are more urgent due to higher utilization |

**Why these matter**: DCFC chargers are the high-value assets. Visual distinction from L2 (larger size) helps players prioritize.

---

### 6. Vehicles (5 assets, 3 required)

| Asset | Gameplay Use-Case | MVP Status |
|-------|-------------------|------------|
| `vehicle_compact.png` | **Critical**: Small EV (Bolt, Leaf); common driver type | Required |
| `vehicle_suv.png` | **Critical**: Mid-size EV (Model Y, ID.4); common driver type | Required |
| `vehicle_pickup.png` | **Critical**: Large EV (F-150 Lightning, Rivian); high power demand | Required |
| `vehicle_sedan.png` | Visual variety; Model 3, EQS | Nice to have |
| `vehicle_crossover.png` | Visual variety | Post-MVP |

**Why these matter**: Vehicles communicate demand type. Pickups draw more power; compacts are lower demand. Players learn to anticipate grid stress from vehicle mix.

---

### 7. Characters (6 assets)

| Asset | Gameplay Use-Case |
|-------|-------------------|
| `character_driver_neutral.png` | **Critical**: Default driver state; waiting for charger or during session |
| `character_driver_happy.png` | **Critical**: Session complete, no issues; positive feedback |
| `character_driver_impatient.png` | **Critical**: Patience 50–75%; warning to player |
| `character_driver_angry.png` | **Critical**: Patience <50%; urgent; player about to lose reputation |
| `character_technician_idle.png` | **Critical**: Technician available for dispatch |
| `character_technician_working.png` | Technician on task; visual feedback that work is happening |

**Why these matter**: Driver emotion is the primary feedback for customer satisfaction. Players scan for angry drivers to prevent reputation loss.

---

### 8. UI Icons — HUD (11 assets)

| Asset | Gameplay Use-Case |
|-------|-------------------|
| `icon_cash.png` | **Critical**: Current cash balance display |
| `icon_revenue_target.png` | **Critical**: Progress toward win condition ($100 target) |
| `icon_time.png` | **Critical**: Elapsed time display; player knows how long they have |
| `icon_reputation.png` | **Critical**: Reputation score display; lose condition at <20 |
| `icon_ticket.png` | **Critical**: Ticket queue indicator; shows pending customer issues |
| `icon_technician.png` | **Critical**: Technician status/availability indicator |
| `icon_power.png` | **Critical**: Site power usage vs. capacity |
| `icon_pause.png` | **Critical**: Pause button |
| `icon_speed_1x.png` | **Critical**: Normal speed indicator |
| `icon_speed_10x.png` | **Critical**: Fast speed indicator (default) |
| `icon_speed_30x.png` | Very fast speed indicator; less critical |

**Why these matter**: HUD icons are always visible. They must be instantly recognizable at a glance.

---

### 9. UI Icons — Grid/Power (6 assets)

| Asset | Gameplay Use-Case |
|-------|-------------------|
| `icon_phase_a.png` | **Critical**: Phase A load indicator in power panel |
| `icon_phase_b.png` | **Critical**: Phase B load indicator |
| `icon_phase_c.png` | **Critical**: Phase C load indicator |
| `icon_transformer_temp.png` | **Critical**: Transformer temperature indicator |
| `icon_voltage_warning.png` | Voltage sag alert icon |
| `icon_breaker_trip.png` | Breaker trip alert icon |

**Why these matter**: The complex grid simulation needs visual feedback. Phase balance is a core mechanic.

---

### 10. UI Icons — Actions (9 assets)

| Asset | Gameplay Use-Case |
|-------|-------------------|
| `icon_action_soft_reboot.png` | **Critical**: Remote reboot button; most common fix |
| `icon_action_hard_reboot.png` | **Critical**: Hard reboot button; escalation from soft reboot |
| `icon_action_release.png` | **Critical**: Release connector button; fixes stuck cable |
| `icon_action_refund.png` | **Critical**: Refund button; closes billing tickets at cost |
| `icon_action_dispatch.png` | **Critical**: Dispatch technician button |
| `icon_action_disable.png` | Disable charger button |
| `icon_action_enable.png` | Enable charger button |
| `icon_action_acknowledge.png` | **Critical**: Acknowledge ticket button; buys time |
| `icon_action_apologize.png` | Apologize button; soft ticket resolution |

**Why these matter**: Action icons are the primary interaction surface. Players click these constantly.

---

### 11. UI Icons — Status (5 assets)

| Asset | Gameplay Use-Case |
|-------|-------------------|
| `icon_warning.png` | **Critical**: Generic warning indicator; used in multiple contexts |
| `icon_fault.png` | **Critical**: Fault/error indicator; appears on broken chargers |
| `icon_success.png` | **Critical**: Checkmark for successful actions |
| `icon_info.png` | Info indicator; used in tutorials |
| `icon_sla_timer.png` | **Critical**: SLA countdown indicator on tickets |

**Why these matter**: Status icons provide at-a-glance feedback on action outcomes.

---

### 12. UI Panels (7 assets)

| Asset | Gameplay Use-Case |
|-------|-------------------|
| `ui_panel_bg.png` | **Critical**: Background for all UI panels (9-slice) |
| `ui_button_default.png` | **Critical**: Default button state |
| `ui_button_hover.png` | Button hover state |
| `ui_button_pressed.png` | Button pressed state |
| `ui_progress_bg.png` | **Critical**: Progress bar background (revenue target, SLA timer) |
| `ui_progress_fill.png` | **Critical**: Progress bar fill |
| `ui_tooltip_bg.png` | Tooltip background for hover info |

**Why these matter**: Consistent UI chrome makes the game feel polished and professional.

---

### 13. VFX / Indicators (8 assets)

| Asset | Gameplay Use-Case |
|-------|-------------------|
| `vfx_light_pulse_green.png` | **Critical**: Animated status light for available chargers |
| `vfx_light_pulse_blue.png` | **Critical**: Animated status light for charging |
| `vfx_light_pulse_yellow.png` | **Critical**: Animated status light for warning |
| `vfx_light_pulse_red.png` | **Critical**: Animated status light for fault |
| `vfx_float_money.png` | **Critical**: "+$X" floating text when revenue earned |
| `vfx_float_rep_loss.png` | **Critical**: "−rep" floating text when reputation lost |
| `vfx_selection.png` | Selection highlight when charger/driver is clicked |
| `vfx_urgent_pulse.png` | **Critical**: Pulsing alert for SLA breach imminent |

**Why these matter**: VFX provides immediate feedback. The floating money/rep tokens are critical for understanding cause and effect.

---

## MVP Asset Summary

### Critical (P0) — Must ship

| Category | Count |
|----------|-------|
| Tiles | 4 |
| Decals | 1 |
| Props | 2 |
| Chargers (L2) | 5 |
| Chargers (DCFC) | 5 |
| Vehicles | 3 |
| Characters | 5 |
| UI Icons (HUD) | 10 |
| UI Icons (Power) | 4 |
| UI Icons (Actions) | 6 |
| UI Icons (Status) | 4 |
| UI Panels | 4 |
| VFX | 7 |
| **Total P0** | **60** |

### Should Have (P1) — Ship if possible

| Category | Count |
|----------|-------|
| Tiles | 2 |
| Decals | 2 |
| Props | 3 |
| Chargers (cables) | 4 |
| Vehicles | 1 |
| Characters | 1 |
| UI Icons | 5 |
| UI Panels | 3 |
| VFX | 1 |
| **Total P1** | **22** |

### Nice to Have (P2) — Post-MVP

| Category | Count |
|----------|-------|
| Decals | 2 |
| Props | 1 |
| Vehicles | 1 |
| **Total P2** | **4** |

---

## Production Order (Recommended)

### Phase 1: Golden Assets (Day 1–2)
Create and approve style templates:
1. `charger_dcfc_available.png` — establishes charger visual language
2. `vehicle_compact.png` — establishes vehicle visual language
3. `tile_asphalt_clean.png` — establishes environment style
4. `icon_cash.png` — establishes icon style

### Phase 2: Core Chargers & States (Day 3–5)
All charger states for both L2 and DCFC.

### Phase 3: Characters & Vehicles (Day 6–7)
All driver states, technician, and 3 vehicle types.

### Phase 4: Environment (Day 8–9)
Remaining tiles, decals, and props.

### Phase 5: UI & VFX (Day 10–12)
All icons, panels, and VFX.

### Phase 6: Polish & Variants (Day 13–14)
P1 assets, hover states, additional variants.

---

## Validation Checklist

Before declaring "art complete" for MVP:

- [ ] All P0 assets exist and pass acceptance criteria
- [ ] Charger states are distinguishable at zoomed-out view
- [ ] Driver emotions are readable at game scale
- [ ] HUD icons are instantly recognizable
- [ ] VFX provides clear feedback on actions
- [ ] All assets placed together on a test scene look cohesive
- [ ] No perspective drift between assets
- [ ] Color palette is consistent across all categories

---

*Document version: 1.1 — February 2026*
*Related docs: [STYLE_GUIDE.md](STYLE_GUIDE.md)*

