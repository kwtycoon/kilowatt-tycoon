# Kilowatt Tycoon — Ecosystem

## 1. Purpose
This document describes the **ecosystem simulated in Kilowatt Tycoon**. It defines the environments, actors, constraints, and external forces that shape how a Charging Point Operator (CPO) builds and operates a charging network.

The ecosystem is designed to feel **alive, imperfect, and reactive**, forcing players to make tradeoffs rather than pursue a single optimal strategy.

---

## 2. Site Types

Players expand their network by selecting and developing different **site archetypes**. Each site type has unique demand patterns, risks, and economics.

### 2.1 Parking Lot (`ParkingLot`)
- Open-air parking lot, balanced constraints
- Starter site (First Street Station is free)
- 1500 kVA grid capacity, moderate popularity
- Good space for solar canopies

Primary challenge: **learning the ropes** — gentle introduction to all mechanics

### 2.2 Gas Station (`GasStation`)
- Converted gas station with canopy
- High traffic (85 popularity), compact layout
- 500 kVA grid capacity — tighter power budget
- Mix of fast and slow chargers

Primary challenge: **throughput under power constraints** — high demand meets limited grid

### 2.3 Fleet Depot (`FleetDepot`)
- Massive commercial fleet charging facility (30x20 grid)
- 3000 kVA (3 MW) utility connection
- Shift-change demand spikes
- Highest rent cost ($35,000)

Primary challenge: **scale and power management** — the endgame site

### 2.4 Future Expansion

Additional site archetypes are planned (see [EXPANSION_SITES_VEHICLES.md](EXPANSION_SITES_VEHICLES.md)):
- Pier / Marina (poor power quality)
- Airport (massive demand spikes)
- Bus Depot (strict overnight windows)
- Truck Stop (24/7 heavy-duty operation)
- Apartment Complex (fairness and shared infrastructure)

---

## 3. Investment Categories

Each site requires upfront and ongoing investment.

### 3.1 Charger Hardware

Five charger pad types with different power ratings:

| Pad Type | Power | Type | Use Case |
|----------|-------|------|----------|
| L2 | 22 kW | AC Level 2 | Long dwell, low cost |
| DCFC50 | 50 kW | DC Fast | Budget fast charging |
| DCFC100 | 100 kW | DC Fast | Mid-tier (has ad screen) |
| DCFC150 | 150 kW | DC Fast | High-power |
| DCFC350 | 350 kW | DC Fast | Ultra-fast premium |

Each charger also has a **tier** (Value, Standard, Premium) affecting:
- MTBF (mean time between failures)
- Efficiency
- Connector jam rate
- Remote action success bonus (defined but not yet applied in action system)

### 3.2 Charger Reliability

Chargers track a **reliability score** (0.0–1.0) that degrades with use and can be recovered through:
- Maintenance investment (continuous $/hr slider)
- O&M upgrade tiers (auto-dispatch, passive recovery)
- Successful repairs

### 3.3 Grid Hookup & Power
- Utility connection with contracted capacity (kVA)
- Transformer purchase and placement
- Three-phase power distribution
- Demand charge tracking (15-minute rolling average)

### 3.4 Solar & Battery

| Investment | Purpose | Implementation |
|-----------|---------|----------------|
| Solar panels | Reduce energy costs, weather-dependent | `SolarState` resource |
| Battery (BESS) | Peak shaving, backup power | `BessState` resource |

### 3.5 Amenities

Players can invest in site amenities that affect reputation and revenue:

| Amenity | Effect |
|---------|--------|
| `WifiRestrooms` | Baseline comfort |
| `LoungeSnacks` | Improved experience |
| `Restaurant` | Premium experience |

### 3.6 Revenue Enhancements
- **Video advertisements** on DCFC100 chargers (`video_ad_enabled`)
- **Anti-theft cables** to reduce cable theft risk (tiered pricing)
- **Dynamic pricing** via `ServiceStrategy` (energy price, idle fees)

### 3.7 Not Yet Implemented
- Payment terminals as separate purchasable hardware
- Government subsidies and tax credits
- Regulatory compliance and fines
- Discrete maintenance actions (inspection, firmware updates, connector replacement) — currently maintenance is a continuous investment slider

---

## 4. Maintenance & Degradation

### 4.1 Continuous Maintenance
Maintenance is controlled via a `maintenance_investment` slider ($/hr) in the `ServiceStrategy`. Higher investment reduces failure probability through a `failure_rate_multiplier()`.

### 4.2 Fault Types

| Fault | Repair Cost | Duration | Technician Required |
|-------|------------|----------|---------------------|
| CommunicationError | $0 | 0s | No |
| CableDamage | $0 | 0s | Yes |
| PaymentError | $0 | 0s | No |
| GroundFault | $200 | 900s | Yes |
| FirmwareFault | $350 | 1200s | No |
| CableTheft | Dynamic | 1200s | Yes |

### 4.3 Degradation
- Charger `operating_hours` increase with use
- Higher hours increase `fault_probability()`
- Reliability score decays and recovers based on maintenance investment and O&M tier

---

## 5. External Threats

### 5.1 Cable Theft (Robber System)
A robber system spawns NPCs that attempt cable theft:
- Robbers approach, steal, and flee
- Anti-theft cables reduce success rate
- Security cameras and alarms provide deterrence
- Tracked via `DailyRobberyTracker`

### 5.2 Weather

Weather affects multiple game systems:

| Weather | Solar | Charger Health | Demand | Patience |
|---------|-------|---------------|--------|----------|
| Sunny | 1.0 | 1.0 | Normal | Normal |
| Overcast | 0.6 | 1.0 | Normal | Normal |
| Rainy | 0.3 | 0.98 | Reduced | Lower |
| Heatwave | 1.2 | 0.85 | Increased | Lower |
| Cold | 0.9 | 0.95 | Normal | Normal |

> **Note**: Weather does NOT currently affect technician travel time (planned but not implemented).

### 5.3 News Events
The `EnvironmentState` includes a news system (`active_news`, `news_demand_multiplier`) that temporarily modifies demand. News events roll randomly and create demand spikes or lulls.

### 5.4 Not Yet Implemented
- Grid events (partial outages, rolling blackouts, voltage instability)
- Weather-affected technician travel times
- Storm-related outages

---

## 6. Energy Systems & Upgrades

### 6.1 Solar Generation
- Reduces energy costs; generation varies by weather and time of day
- Limited by available space (site-specific `solar_positions`)
- Configured via `SiteEnergyConfig`

### 6.2 Battery Storage (BESS)
- Peak shaving (automatic discharge when load exceeds threshold)
- Backup power potential
- SOC tracking, charge/discharge cycles
- Configured via `BessState`

### 6.3 Carbon Credits
- `CarbonCreditMarket` resource tracks carbon credit pricing
- Revenue from clean energy use

---

## 7. Ecosystem Philosophy
- The ecosystem is **not stable**
- External forces constantly apply pressure
- Resilience beats optimization

Players who invest only for best-case scenarios will fail.

---

## 8. Design Goal
The ecosystem exists to answer one question:

> *Why is running a charging network hard, even when demand is high?*

The answer emerges naturally through play, not tutorials.
