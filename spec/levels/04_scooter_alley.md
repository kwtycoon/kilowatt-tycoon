# Level 4: Scooter Alley

![Concept](04_scooter_alley_concept.png)

## Site Info

| Property | Value |
|----------|-------|
| Archetype | ScooterHub |
| Grid Size | 30 x 20 |
| Grid Capacity | 800 kVA |
| Rent Cost | $28,000 |
| Popularity | 95 (Highest in the game) |
| Challenge Level | 4 (Expert) |
| Climate | Hot & Humid (HCMC tropical, +15 F offset) |

## Visual Identity

A dense urban EV scooter charging alley inspired by Ho Chi Minh City:
- Herringbone-angled scooter parking bays packed into tight rows
- Overhead solar canopy providing shade across the main dock area
- Concrete service lanes between dock rows
- Grass utility strips on the edges for transformer / solar / battery placement
- Road running along the bottom with heavy two-wheeler ambient traffic
- Tropical feel: hot climate warning on the site card

## Traffic Profile

This site is overwhelmingly dominated by electric scooters and motorcycles:

| Vehicle Type | Ambient Traffic | Procedural Customers |
|-------------|----------------|---------------------|
| Scooter | 74% | 78% |
| Motorcycle | 23% | 19% |
| All others combined | 3% | 3% |

Typical battery sizes are 1.5 - 3.0 kWh with charge sessions completing in minutes rather than hours. Expect extremely high turnover and constant queue pressure.

## Recommended Chargers

| Charger | Count | Cost | Power |
|---------|-------|------|-------|
| L2 (7 kW manifold) | 20 | $60,000 | 140 kW |
| L2 (22 kW standard) | 4 | $12,000 | 88 kW |

**Total Investment**: ~$72,000
**Total Power Draw**: ~228 kW (well within 800 kVA limit)

The scooter hub rewards dense L2 placement over any DCFC. A single 22 kW L2 circuit can be thought of as a multi-headed pedestal split between several scooter docks. The 7 kW compact manifolds are purpose-built for scooter batteries and should be the backbone of the layout.

## Challenges

### Bronze: Scooter Swarm
Complete 80 charging sessions - High-volume, rapid turnover target

### Silver: Alley Revenue
Earn $8,000 in revenue - Many small transactions add up

### Gold: HCMC Operator
Earn $15,000 while maintaining 85%+ rating - Sustained quality under relentless demand

## Strategy Tips

- **Volume is king**: Scooter batteries are tiny (1.5 - 3 kWh) so sessions finish fast. Pack as many L2 chargers as you can afford. Every empty dock is lost revenue.
- **Load balancing matters**: 20+ chargers pulling simultaneously can stress even the 800 kVA connection. The power dispatch system rotates full power to nearly-empty scooters and trickle-charges the rest. Watch the transformer temperature.
- **Tropical heat**: The +15 F climate offset means transformer thermal limits are reached sooner than on other sites. Consider battery storage to buffer peak load and keep the transformer cool.
- **Monsoon faults**: Scripted events simulate monsoon-like conditions (ground faults, communication errors). IP67-rated reliability upgrades and quick remote reboots are essential.
- **No DCFC needed**: Scooters and motorcycles only support L2 chargers. Don't waste money or grid capacity on fast chargers nobody can use.
- **Popularity 95**: This is the highest-traffic site in the game. Demand is relentless. If you don't have enough bays, frustrated riders will leave angry and tank your reputation.
- **Solar canopy synergy**: The canopy zone is ideal for solar arrays which both offset grid draw and keep battery temperatures down during charging.
