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

## Vietnam-Inspired Mechanics

This level introduces several mechanics drawn from real-world challenges facing EV charging infrastructure in Vietnam.

### Monsoon Flooding

Scripted monsoon events simultaneously trigger ground faults on multiple chargers (2-6 depending on severity) and temporarily reduce site capacity by 30% for 30 game-minutes. You cannot rely on municipal drainage to protect high-voltage equipment. IP67-rated reliability upgrades and quick technician dispatch are essential.

### Grid Brownouts

Vietnam's fragile hydropower-dependent grid suffers brownouts during heatwaves. When a brownout fires, your effective grid capacity drops 40% and import prices spike 2.5x while export prices jump 8x. BESS peak-shaving becomes critical for surviving brownouts profitably.

### Battery Swap Competitor

A battery-swap station (modeled after Vietnam's Selex Motors / V-Green networks) opens nearby mid-scenario. While active, demand from impatient (low-patience) riders drops by 35%. This creates pressure to either accept the demand loss or counter with the Driver Rest Lounge amenity.

### Residential Ban Demand Surge

Vietnamese apartment complexes like HH Linh Dam (30,000+ residents) have begun banning EV basement charging due to fire safety fears. A scripted demand surge event floods your station with displaced apartment dwellers, spiking demand 60% temporarily.

### Punitive Capacity Charge

ScooterHub uses a Vietnam-style two-component tariff: a 30-minute demand window (vs. 15 min on other sites) and a $25/kW capacity charge (vs. $15 default). Unmanaged peak loads are devastating. Dynamic Load Management via solar + BESS peak-shaving is mandatory for profitability.

### Driver Rest Lounge

A new amenity ($25,000, 3x3 tiles) inspired by HCMC's "charging dormitory" phenomenon. Gig-economy drivers sleep or rest while their scooters charge. The lounge reduces driver patience depletion by 40% and generates ancillary revenue. It directly counters the battery-swap competitor: drivers who value the rest stay despite longer L2 charge times.

## Challenges

### Bronze: Scooter Swarm
Complete 80 charging sessions - High-volume, rapid turnover target

### Silver: Alley Revenue
Earn $8,000 in revenue while surviving a monsoon flood

### Gold: HCMC Operator
Earn $15,000 while maintaining 85%+ rating with Driver Rest Lounge built

## Strategy Tips

- **Volume is king**: Scooter batteries are tiny (1.5 - 3 kWh) so sessions finish fast. Pack as many L2 chargers as you can afford. Every empty dock is lost revenue.
- **Load balancing matters**: 20+ chargers pulling simultaneously can stress even the 800 kVA connection. The power dispatch system rotates full power to nearly-empty scooters and trickle-charges the rest. Watch the transformer temperature.
- **Tropical heat**: The +15 F climate offset means transformer thermal limits are reached sooner than on other sites. Consider battery storage to buffer peak load and keep the transformer cool.
- **Monsoon faults**: Scripted events simulate monsoon-like conditions (simultaneous ground faults, capacity reduction). IP67-rated reliability upgrades and quick remote reboots are essential.
- **No DCFC needed**: Scooters and motorcycles only support L2 chargers. Don't waste money or grid capacity on fast chargers nobody can use.
- **Popularity 95**: This is the highest-traffic site in the game. Demand is relentless. If you don't have enough bays, frustrated riders will leave angry and tank your reputation.
- **Solar canopy synergy**: The canopy zone is ideal for solar arrays which both offset grid draw and keep battery temperatures down during charging.
- **Capacity charge defense**: The 30-minute demand window and $25/kW rate make unmanaged peaks ruinous. Deploy BESS in PeakShaving mode and keep an eye on the 30-min rolling average.
- **Counter the swap station**: When the battery-swap competitor arrives, your demand from impatient riders drops 35%. Build a Driver Rest Lounge to retain gig workers who value rest amenities over raw speed.
- **Ride the residential surge**: When the apartment ban demand surge hits, you need spare charger capacity. Keep a few bays free or expand proactively.
- **Brownout survival**: Grid brownouts slash your capacity 40% and spike import costs. BESS stored energy becomes your lifeline. Charge batteries off-peak, discharge during brownouts.

## Scenario Events Timeline

| Time (s) | Event | Effect |
|-----------|-------|--------|
| 90 | Charger Fault | Communication error on one charger |
| 170 | Monsoon Flood (sev. 2) | Ground faults on 4 chargers, 30% capacity reduction |
| 200 | Transformer Warning | Thermal pressure at 78°C |
| 350 | Battery Swap Competitor | 35% demand reduction from impatient riders |
| 500 | Demand Surge | 60% demand increase from apartment-ban refugees |
| 650 | Grid Brownout | 40% capacity reduction, 2.5x import prices |
