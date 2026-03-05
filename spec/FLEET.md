# Fleet Contracts

Fleet contracts are agreements between the player (CPO) and commercial vehicle operators.
A fleet company sends a guaranteed number of vehicles per day during specific time windows.
The player earns a high daily retainer plus per-kWh charging revenue. In exchange, the
player must keep chargers operational during fleet windows -- missed vehicles incur cash
penalties, reputation hits, and eventual contract termination.

## Value Proposition

Fleet contracts monetize **idle off-peak capacity**:

- **High daily retainer** -- the flat fee alone is significant income ($8,000/day for transit
  buses, $2,000/day for delivery scooters). This is the primary incentive.
- **Off-peak time windows** -- fleet vehicles arrive during slow hours (early morning, late
  night, midday lull). Chargers would otherwise sit idle, so even the discounted per-kWh rate
  is pure upside.
- **The risk** -- broken, overloaded, or insufficient chargers during fleet windows trigger
  penalties that eat into the retainer. Accumulate too many breaches across days and the
  contract is terminated permanently.

A well-managed fleet contract roughly **doubles** daily revenue. A poorly managed one is a
net loss.

## Applicable Levels

| Level | Site | Fleet Company | Vehicle Type |
|-------|------|---------------|--------------|
| 3 | Central Fleet Plaza (FleetDepot) | Metro Transit Authority | Buses |
| 4 | Scooter Alley (ScooterHub) | GrabFood Saigon | Scooters |

## Contract Terms

| Term | Description |
|------|-------------|
| `company_name` | Display name of the fleet operator |
| `company_color` | Tint color applied to fleet vehicle sprites |
| `vehicle_types` | Which vehicle types the fleet sends |
| `vehicles_per_day` | Total vehicles expected per day |
| `time_windows` | When vehicles arrive (off-peak focused) |
| `contracted_price_per_kwh` | Negotiated rate (below retail -- volume discount) |
| `daily_payment` | Flat retainer paid at day-end if contract is active |
| `penalty_per_miss` | Cash deducted per fleet vehicle that leaves uncharged |
| `reputation_penalty_per_miss` | Reputation hit per missed vehicle |
| `max_breaches_before_termination` | Cumulative misses (across all days) before contract ends |

## Player Interaction

1. **Day start**: A non-blocking banner offers available contracts. Accept or decline.
2. **During gameplay**: Fleet vehicles spawn during contracted time windows with company
   color tint and floating badge. They use the standard driver pipeline.
3. **Breach tracking**: When a fleet vehicle leaves angry, a penalty is applied immediately.
   Breaches accumulate across days.
4. **Day end**: Fleet contract summary shows vehicles charged vs. expected, penalties
   incurred, retainer earned, and contract health.
5. **Termination**: If cumulative breaches exceed the threshold, the contract is terminated
   permanently.

## Cross-Day Persistence

`FleetContractManager` persists across `DayEnd` -> `Playing` transitions:

- **Reset each day**: `vehicles_spawned_today`, `vehicles_charged_today`, `vehicles_missed_today`
- **Persist forever**: `breaches_total`, `terminated`, `day_accepted`

## Financial Integration

| Account | Type | Description |
|---------|------|-------------|
| `FleetContract` | Income | Daily retainer credited at day-end |
| `FleetPenalty` | Expense | Per-miss penalties debited in real-time |

## Example Contracts

### Metro Transit Authority (Level 3)

- 8 buses/day: 5-7 AM (4), 10 PM-midnight (4)
- $0.25/kWh, **$8,000/day retainer**
- $500 + 5 rep per miss, terminated after 10 breaches

### GrabFood Saigon (Level 4)

- 25 scooters/day: 5-7 AM (10), 2-4 PM (8), 10 PM-midnight (7)
- $0.10/kWh, **$2,000/day retainer**
- $40 + 2 rep per miss, terminated after 15 breaches

## OCPI 2.3.0 Integration

Fleet sessions produce protocol-accurate OCPI data that distinguishes them from walk-in
(retail) sessions. OCPP 1.6J needs no fleet-specific changes -- fleet vehicles are local
sessions using the standard `StartTransaction` / `StopTransaction` flow.

### Fleet Token Identity

Walk-in local drivers use `party_id = "KWT"` and `contract_id = "US-KWT-{evcc_id}"`.
Fleet drivers use a distinct party ID so CDR exports can be filtered:

| Field | Walk-in (local) | Fleet |
|-------|-----------------|-------|
| `party_id` | `KWT` | `FLT` |
| `contract_id` | `US-KWT-{evcc_id}` | `US-FLT-{evcc_id}` |
| `auth_method` | `Whitelist` | `Whitelist` |

Roaming drivers remain unchanged (`party_id = "EVC"`, `contract_id = "US-EVC-{evcc_id}"`).

### Fleet Tariff

Site-based tariffs (e.g. `KWT-FLAT-1`, `KWT-TOU-2`) reflect the player's retail pricing.
Fleet contracts negotiate a fixed per-kWh rate, so a separate tariff is emitted:

| Tariff ID | Description |
|-----------|-------------|
| `KWT-FLEET-metro_transit` | Metro Transit Authority @ $0.25/kWh |
| `KWT-FLEET-grabfood_saigon` | GrabFood Saigon @ $0.10/kWh |

Fleet sessions and CDRs reference the fleet tariff instead of the site tariff. This ensures
the OCPI data accurately reflects the contracted price, not the retail rate.

### What Changes Where

| OCPI Message | Walk-in | Fleet |
|--------------|---------|-------|
| Session PUT | `cdr_token.party_id = "KWT"`, site tariff | `cdr_token.party_id = "FLT"`, fleet tariff |
| Session PATCH | Site pricing, site tariff | Contracted pricing, fleet tariff |
| CDR POST | `cdr_token.party_id = "KWT"`, site tariff | `cdr_token.party_id = "FLT"`, fleet tariff |
| Tariff PUT | Site tariffs only | Site tariffs + fleet tariffs |
