# Kilowatt Tycoon — Site & Vehicle Expansion

> *Scale your empire: From streetside stations to massive fleet depots*

---

## 1. Overview

This document describes **expansion content** beyond the MVP:

1. **6 new site archetypes** (targeting 12+ total sites)
2. **Heavy vehicle classes** (buses, semis, tractors)
3. **Power quality mechanics** (utility dips, fault tracking)
4. **Progression gating system** (empire building)

The expansion transforms the game from a single-site operation into a **multi-site charging empire builder**.

---

## 2. Design Philosophy

### Progression Through Scale
- Cannot afford large sites initially — rent cost gates access
- Must keep multiple sites running — passive income funds expansion
- Different sites serve different markets — no single optimal strategy
- Bigger sites = bigger problems — scale brings new failure modes

### Variety Through Constraints
Each site type has a unique "signature constraint" forcing different strategies:

| Site | Signature Constraint |
|------|---------------------|
| Pier | Poor power quality |
| Airport | Massive demand spikes |
| Bus Depot | Strict overnight windows |
| Truck Stop | 24/7 heavy-duty operation |
| Fleet Depot | Scale and contracts |
| Apartment | Fairness and shared infrastructure |

---

## 3. New Site Archetypes

### Current Sites (MVP)
1. **Parking Lot** — Starter site, balanced
2. **Gas Station** — High traffic, compact
3. **Mall** — Large capacity, excellent power
4. **Streetside** — Urban, power-constrained
5. **Workplace** — Weekday peaks
6. **Destination** — Hotels/resorts, reputation-critical

### 3.1 Pier / Marina
**Scale**: Medium | **Grid**: 50-75 kVA | **Rent**: $6,000

Aging waterfront electrical infrastructure with poor power quality — frequent voltage sags, harmonics, and +30% failure rates from salt air corrosion. Tourist/seasonal traffic. Solar canopy potential.

**Signature challenge**: Power Quality — players must invest in battery storage and voltage regulation to compensate for unreliable utility supply.

### 3.2 Airport
**Scale**: Large | **Grid**: 300+ kVA | **Rent**: $20,000

Rental car returns and rideshare fleets. Excellent power infrastructure but massive demand spikes every 30-90 minutes from flight arrivals (10-20 vehicles at once). Strict uptime SLAs with penalty clauses. No solar (airspace restrictions). Battery storage required.

**Signature challenge**: Demand Spikes — inrush events, thermal stress, and demand charge management during arrival waves.

### 3.3 Bus Depot (Transit)
**Scale**: Large | **Grid**: 200-300 kVA | **Rent**: $18,000

Electric buses only. Overnight charging windows (arrive 11 PM - 1 AM, must be 100% by 5 AM). Rooftop solar on depot building. Scheduled arrivals. High-power DC chargers (150-350 kW) with pull-through bays required.

**Signature challenge**: Overnight Windows — failure is not negotiable (buses can't run if not charged), demand charges are brutal over a narrow 6-hour peak, and contract penalties reach $2,000+ per missed bus.

### 3.4 Truck Stop / Highway Rest Area
**Scale**: Large | **Grid**: 150-200 kVA | **Rent**: $15,000

24/7 operation with mix of semi-trucks and passenger vehicles. Heavy-duty connectors (trucks are rough on equipment). Remote location means longer technician travel times. Solar canopies over truck parking.

**Signature challenge**: 24/7 Heavy-Duty Operation — no slow hours for maintenance, heavy-duty wear on equipment, impatient truckers on tight schedules, and semi batteries at 300-500+ kWh.

### 3.5 Fleet Depot (Commercial)
**Scale**: Very Large | **Grid**: 400+ kVA | **Rent**: $25,000

The "endgame" site: 50+ charging bays with mixed buses, delivery vans, trucks, and service vehicles. Multiple fleet contracts with different SLAs. Shift-change spikes (40 vehicles in 15 minutes). Solar + battery mandatory. Dedicated on-site staff.

**Signature challenge**: Scale & Complexity — 20+ simultaneous sessions, multiple vehicle types (350 kW buses alongside 50 kW vans), overlapping contracts, and constant transformer stress.

### 3.6 Apartment Complex / MURB
**Scale**: Medium | **Grid**: 80-100 kVA | **Rent**: $8,000

Multi-unit residential garage with shared infrastructure. Evening peak congestion (everyone arrives after work). Rooftop solar with building owner revenue share. Reputation heavily tied to "fairness" perception.

**Signature challenge**: Fairness & Shared Resources — tenants expect equal access, power can't serve everyone simultaneously at full power, and complaints escalate to contract-threatening levels.

---

## 4. Heavy Vehicle Classes

### Current Vehicles (MVP)
Compact, Sedan, SUV, Crossover, Pickup

### New Vehicle Types

| Type | Battery (kWh) | Max Power (kW) | Session | Patience | Sites |
|------|--------------|-----------------|---------|----------|-------|
| **Bus** | 300-400 | 150-350 DC | 2-4 hours | N/A (scheduled) | Bus Depot, Fleet Depot |
| **Semi** | 400-600 | 350 DC | 1-2 hours | Low | Truck Stop, Fleet Depot |
| **Tractor** | 200-300 | 50-150 | 2-4 hours | High | Fleet Depot, Workplace |

Key differences from passenger vehicles:
- **Buses** arrive on schedule (not random demand curve), require pull-through bays, and missing a charge means a bus doesn't run
- **Semis** have higher revenue ($0.50-0.60/kWh), lower patience, and cause more connector wear (1.5x)
- **Tractors** are seasonal, niche, and lower power requirements

### Vehicle Type Compatibility Matrix

| Site Type | Compact | Sedan | SUV | Bus | Semi | Tractor |
|-----------|---------|-------|-----|-----|------|---------|
| Parking Lot | Y | Y | Y | - | - | - |
| Gas Station | Y | Y | Y | - | Y | - |
| Mall | Y | Y | Y | - | - | - |
| Streetside | Y | Y | - | - | - | - |
| Workplace | Y | Y | Y | - | - | Y |
| Destination | Y | Y | Y | - | - | - |
| Pier | Y | Y | Y | - | - | - |
| Airport | Y | Y | Y | - | - | - |
| Bus Depot | - | - | - | Y | - | - |
| Truck Stop | Y | Y | Y | - | Y | - |
| Fleet Depot | Y | Y | Y | Y | Y | Y |
| Apartment | Y | Y | Y | - | - | - |

---

## 5. Power Quality Mechanics

### Power Quality Index (PQI)
Each site has a **Power Quality Index** (0.0 - 1.0) representing utility supply reliability:
- **1.0** = Perfect power (rare, new substations)
- **0.9** = Typical suburban
- **0.7** = Urban on loaded circuit
- **0.5** = Old infrastructure (pier)
- **0.3** = Problematic (immediate mitigation needed)

PQI affects fault session rate: `actual_fault_rate = base_fault_rate / PQI`

### Site-Specific PQI Values

| Site | PQI | Reason |
|------|-----|--------|
| Parking Lot | 0.85 | Suburban, modern infrastructure |
| Gas Station | 0.80 | Commercial area, moderate load |
| Mall | 0.90 | New construction, dedicated feeder |
| Streetside | 0.70 | Urban, shared circuit |
| Workplace | 0.85 | Industrial area, good power |
| Destination | 0.88 | Premium location, maintained |
| Pier | **0.55** | **Aging waterfront infrastructure** |
| Airport | 0.92 | Critical infrastructure |
| Bus Depot | 0.85 | Industrial zone, stable |
| Truck Stop | 0.75 | Rural/highway, long lines |
| Fleet Depot | 0.88 | Industrial, high-capacity |
| Apartment | 0.78 | Residential, shared transformer |

### PQI Mitigation
1. **Battery Storage**: Buffers utility problems, +0.1 to +0.2 effective PQI
2. **Voltage Regulation Equipment**: AVR + surge suppression, +0.15 PQI ($8,000)
3. **Power Quality Monitor**: Early warning, no PQI improvement ($2,000)
4. **Utility Upgrade**: If PQI < 0.6, pay $15,000 for guaranteed fix to 0.85+

### Dynamic PQI Events

| Event | Duration | PQI Impact |
|-------|----------|------------|
| Nearby industrial startup | 2-4 hours | -0.15 |
| Utility maintenance | 30-90 min | -0.25 |
| Storm / extreme weather | Variable | -0.20 |
| Capacitor bank switching | 5 min | -0.10 |

---

## 6. Progression & Empire Building

### Site Unlock Requirements

| Site | Rent | Prerequisites | Revenue Target |
|------|------|---------------|----------------|
| First Street Station | FREE | None | N/A |
| Gas Station | $5,000 | None | $500/day |
| Streetside | $8,000 | Reputation 60+ | $600/day |
| Workplace | $7,000 | None | $550/day |
| Pier | $6,000 | None | $500/day |
| Apartment | $8,000 | Reputation 65+ | $650/day |
| Mall | $12,000 | Own 2 sites | $1,000/day |
| Destination | $15,000 | Reputation 75+ | $1,200/day |
| Truck Stop | $15,000 | Own 3 sites | $1,200/day |
| Airport | $20,000 | Own 3 sites, Rep 80+ | $1,500/day |
| Bus Depot | $18,000 | Own 3 sites | $1,400/day |
| Fleet Depot | $25,000 | Own 5 sites, Rep 85+ | $2,000/day |

### Multi-Site Management
Running multiple sites introduces:
- **Resource allocation** — limited technician pool, capital priorities, attention management
- **Portfolio strategy** — stable base (workplace) funds risky expansion (streetside)
- **Background simulation** — unviewed sites continue running (sessions, faults, revenue)
- **Critical alerts** — thermal trips and breaker trips force attention regardless of current site

**Victory condition (sandbox)**: $10,000 net revenue per day across all sites.

---

## 7. Site Constraint Matrix

| Site | Grid (kVA) | Solar | Battery | Charger Types | Max Bays |
|------|------------|-------|---------|---------------|----------|
| Parking Lot | 100 | Optional | Optional | L2, DCFC50 | 12 |
| Gas Station | 150 | Canopy | Optional | DCFC50, DCFC150 | 8 |
| Mall | 250 | Limited | Optional | All | 20 |
| Streetside | 75 | No | Recommended | L2, DCFC50 | 6 |
| Workplace | 120 | Rooftop | Optional | L2, DCFC50 | 16 |
| Destination | 200 | Optional | Optional | All | 14 |
| **Pier** | 75 | Canopy | **Recommended** | L2, DCFC50 | 10 |
| **Airport** | 300 | **No** | **Required** | DCFC150, DCFC350 | 24 |
| **Bus Depot** | 250 | **Rooftop** | **Required** | DCFC150, DCFC350 | 18 |
| **Truck Stop** | 200 | Canopy | Optional | DCFC150, DCFC350 | 16 |
| **Fleet Depot** | 400 | **Rooftop** | **Required** | All | 50 |
| **Apartment** | 100 | **Rooftop** | Optional | L2, DCFC50 | 12 |

**Legend**: Optional = not required for profitability | Recommended = difficult without it | Required = cannot operate effectively without it | No = not allowed

---

*Document version: 1.1 — February 2026*
*Status: Design — Post-MVP Expansion*
