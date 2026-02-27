# Kilowatt Tycoon — Operations & Support Mechanics

## 1. Purpose

This document specifies the **manual chores, technician control, and customer support systems** that create the "messy" operational feel. The goal is to make players *feel* the complexity of running a real charging network.

---

## 2. Design Philosophy

- **Chaos is the default state.** Things break, drivers complain, cables get tangled.
- **Player attention is the bottleneck.** You can't fix everything instantly.
- **Every chore has a cost.** Ignoring problems costs money; fixing them costs time.

---

## 3. Remote Operations (HUD Chores)

### 3.1 Overview

Remote operations are actions the player can take from the HUD without dispatching a technician. They are **fast but limited**.

### 3.2 Remote Actions

| Action | Trigger | Effect | Cooldown | Success Rate |
|--------|---------|--------|----------|--------------|
| **Soft Reboot** | Communication Error, Firmware Fault, Payment Error | Restarts OCPP connection | 30 seconds | 70% |
| **Hard Reboot** | After Soft Reboot fails | Full power cycle | 2 minutes | 90% |
| **Release Connector** | Cable Damage (connector jam) | Unlocks connector remotely | 10 seconds | 80% |
| **Disable Charger** | Player choice | Takes charger offline | Instant | 100% |
| **Enable Charger** | Player choice | Brings charger back online | 5 seconds | 95% |

### 3.3 Remote Action Implementation

```rust
/// Remote action types
pub enum RemoteAction {
    SoftReboot,
    HardReboot,
    ReleaseConnector,
    Disable,
    Enable,
}

impl RemoteAction {
    /// Cooldown duration in game seconds
    pub fn cooldown_seconds(&self) -> f32 {
        match self {
            RemoteAction::SoftReboot => 30.0,
            RemoteAction::HardReboot => 120.0,
            RemoteAction::ReleaseConnector => 10.0,
            RemoteAction::Disable => 0.0,
            RemoteAction::Enable => 5.0,
        }
    }

    /// Base success rate (0.0 - 1.0)
    pub fn success_rate(&self) -> f32 {
        match self {
            RemoteAction::SoftReboot => 0.70,
            RemoteAction::HardReboot => 0.90,
            RemoteAction::ReleaseConnector => 0.80,
            RemoteAction::Disable => 1.0,
            RemoteAction::Enable => 0.95,
        }
    }
}
```

### 3.4 Fault Types and Resolvability

| Fault Type | Remote Fix? | Repair Cost | Repair Duration | Technician? |
|------------|-------------|-------------|-----------------|-------------|
| Communication Error | Yes (reboot) | $0 | Instant | No |
| Firmware Fault | Yes (reboot) | $0 | Instant | No |
| Payment Error | Yes (reboot) | $0 | Instant | No |
| Ground Fault | No | $200 | 15 minutes | Yes |
| Cable Damage | Yes (ReleaseConnector) | $350 | 20 minutes | Yes (if remote fails) |
| **Cable Theft** | **No** | **Dynamic** | **20 minutes** | **Yes** |

> **Note**: `CableTheft` is triggered by the robber system. Repair cost is dynamic based on cable replacement pricing. Anti-theft cable upgrades reduce theft risk.

```rust
impl FaultType {
    /// Check if this fault requires a technician (vs remote fix)
    pub fn requires_technician(&self) -> bool {
        matches!(self, FaultType::GroundFault | FaultType::CableDamage | FaultType::CableTheft)
    }

    /// Get the repair cost for this fault type (parts + dispatch)
    pub fn repair_cost(&self) -> f32 {
        match self {
            FaultType::CommunicationError => 0.0,
            FaultType::PaymentError => 0.0,
            FaultType::FirmwareFault => 0.0,
            FaultType::GroundFault => 200.0,
            FaultType::CableDamage => 350.0,
            FaultType::CableTheft => /* dynamic cable cost */,
        }
    }

    pub fn repair_duration_secs(&self) -> f32 {
        match self {
            FaultType::CommunicationError => 0.0,
            FaultType::PaymentError => 0.0,
            FaultType::FirmwareFault => 0.0,
            FaultType::GroundFault => 900.0,       // 15 minutes
            FaultType::CableDamage => 1200.0,      // 20 minutes
            FaultType::CableTheft => 1200.0,       // 20 minutes
        }
    }
}
```

### 3.5 Remote Ops UI

- Remote actions appear as **quick-action buttons** in the radial charger menu.
- Failed actions show a toast notification with failure reason.
- Cooldowns are tracked per-charger and visible on action buttons.
- Charger tier success bonus is defined (Premium: +15%, Value: -10%) but **not yet applied** in the action system (dead code).

---

## 4. Physical Tasks (Technician Control)

### 4.1 Overview

Some problems cannot be solved remotely. These require a **technician** to travel to the site and perform physical work.

### 4.2 Technician State Model

The game has a **single shared technician** who travels between sites:

```rust
/// Technician status
pub enum TechStatus {
    Idle,          // Available for dispatch
    EnRoute,       // Traveling to site
    WalkingOnSite, // Walking from entry to charger
    Repairing,     // Performing repair at charger
    LeavingSite,   // Walking from charger to exit
}

/// Technician state - single shared technician
pub struct TechnicianState {
    pub status: TechStatus,
    pub target_charger: Option<Entity>,
    pub current_site_id: Option<SiteId>,
    pub destination_site_id: Option<SiteId>,
    pub travel_remaining: f32,      // Game seconds
    pub travel_total: f32,          // For progress bar
    pub repair_remaining: f32,      // Game seconds
    pub job_time_elapsed: f32,      // For billing
    pub dispatch_queue: Vec<QueuedDispatch>,
}
```

### 4.3 Travel Time Calculation

Travel time depends on distance between site archetypes:

| Distance | Example | Time |
|----------|---------|------|
| Same archetype | ParkingLot → ParkingLot | 5 min |
| Same cluster | ParkingLot → GasStation | 10 min |
| Across town | Any → FleetDepot | 35 min |
| First dispatch (no current site) | — | 60 min |

```rust
pub fn calculate_travel_time(from: SiteArchetype, to: SiteArchetype) -> f32 {
    const BASE_TRAVEL: f32 = 10.0 * 60.0;  // 10 min
    const LONG_TRAVEL: f32 = 35.0 * 60.0;  // 35 min

    if from == to {
        return 5.0 * 60.0;
    }

    match (from, to) {
        // ParkingLot and GasStation are close
        (SiteArchetype::ParkingLot, SiteArchetype::GasStation)
        | (SiteArchetype::GasStation, SiteArchetype::ParkingLot) => BASE_TRAVEL,

        // FleetDepot is far from everything
        (SiteArchetype::FleetDepot, _) | (_, SiteArchetype::FleetDepot) => LONG_TRAVEL,

        _ => BASE_TRAVEL,
    }
}
```

> **Note**: When the technician has no `current_site_id` (first dispatch ever), a `BASE_TRAVEL_TIME` of 3600s (1 hour) is used.

### 4.4 Technician Cost

```rust
/// Technician hourly rate for OpEx calculations
pub const TECHNICIAN_HOURLY_RATE: f32 = 250.0; // $/hour

impl TechnicianState {
    /// Calculate total cost for current job (travel + repair time * hourly rate)
    pub fn calculate_job_cost(&self) -> f32 {
        let hours = self.job_time_elapsed / 3600.0;
        hours * TECHNICIAN_HOURLY_RATE
    }
}
```

### 4.5 Repair Failure

Repairs have a **50% failure chance**. A failed repair:
- Leaves the fault intact on the charger
- Still costs labor time (technician was working)
- The technician can be re-dispatched to try again

### 4.6 Task Assignment Flow

1. Player clicks faulted charger (shows fault in radial menu)
2. Player selects "Dispatch Technician" action
3. Technician is queued (if busy) or dispatched immediately
4. Technician travels to site, walks to charger, attempts repair
5. On success: cost deducted, charger returns to Available
6. On failure (50%): fault remains, labor cost still applies

### 4.7 Same-Site Job Chaining

If the technician has another queued job at the same site, they skip the exit-and-re-enter cycle and pathfind directly to the next charger.

### 4.8 O&M Auto-Dispatch

When the O&M upgrade tier is active, the `om_auto_dispatch_system` automatically dispatches the technician when a fault occurs (no player intervention needed).

### 4.9 On-Site Movement

When the technician arrives at a site:
1. `TechStatus::EnRoute` → `TechStatus::WalkingOnSite`
2. Technician entity spawned at site entry point
3. Uses bevy_northstar pathfinding to walk to charger
4. On arrival: `TechStatus::Repairing` with repair timer
5. When complete: `TechStatus::LeavingSite`, walks to exit
6. On exit: `TechStatus::Idle`, ready for next job

---

## 5. Customer Support System

### 5.1 Overview

Drivers generate **support tickets** when they encounter problems. Unresolved tickets damage reputation and can escalate.

### 5.2 Ticket Lifecycle

```
[Created] → [Acknowledged] → [InProgress] → [Resolved | Escalated]
                                                    ↓
                                              [Closed]
```

### 5.3 Ticket Types

| Type | Trigger | SLA Timer | Base Priority |
|------|---------|-----------|---------------|
| Billing Dispute | Payment issue | 5 minutes | 60 |
| Session Didn't Start | Charger fault | 3 minutes | 70 |
| Connector Stuck | Cable damage | 2 minutes | 80 |
| Slow Charging | Power throttling | 10 minutes | 30 |
| App Error | Firmware fault | 5 minutes | 40 |

```rust
pub enum TicketType {
    BillingDispute,
    SessionDidntStart,
    ConnectorStuck,
    SlowCharging,
    AppError,
}

impl TicketType {
    /// SLA timer in game seconds
    pub fn sla_seconds(&self) -> f32 {
        match self {
            TicketType::BillingDispute => 300.0,    // 5 minutes
            TicketType::SessionDidntStart => 180.0, // 3 minutes
            TicketType::ConnectorStuck => 120.0,    // 2 minutes
            TicketType::SlowCharging => 600.0,      // 10 minutes
            TicketType::AppError => 300.0,          // 5 minutes
        }
    }

    /// Base priority (0-100, higher = more urgent)
    pub fn base_priority(&self) -> i32 {
        match self {
            TicketType::BillingDispute => 60,
            TicketType::SessionDidntStart => 70,
            TicketType::ConnectorStuck => 80,
            TicketType::SlowCharging => 30,
            TicketType::AppError => 40,
        }
    }
}
```

### 5.4 Ticket Component

```rust
#[derive(Component)]
pub struct Ticket {
    pub id: String,
    pub ticket_type: TicketType,
    pub charger_id: String,
    pub driver_id: Option<String>,
    pub session_value: f32,
    pub created_at: f32,         // Game time
    pub sla_deadline: f32,       // Game time
    pub status: TicketStatus,
    pub priority: i32,
    pub driver_message: String,
    pub sla_paused_until: Option<f32>,
}

impl Ticket {
    pub fn new(
        id: String,
        ticket_type: TicketType,
        charger_id: String,
        game_time: f32,
        session_value: f32,
    ) -> Self {
        Self {
            id,
            ticket_type,
            charger_id,
            session_value,
            created_at: game_time,
            sla_deadline: game_time + ticket_type.sla_seconds(),
            priority: ticket_type.base_priority(),
            driver_message: generate_driver_message(ticket_type),
            ..default()
        }
    }

    pub fn is_breached(&self, game_time: f32) -> bool {
        if let Some(paused_until) = self.sla_paused_until
            && game_time < paused_until
        {
            return false;
        }
        game_time >= self.sla_deadline
    }
}
```

### 5.5 Ticket Resolution Actions

> **Status: DEFINED BUT NOT PROCESSED** — The `TicketResolution` enum and `close_chance()` method exist in code, but no system processes player resolution actions. Players cannot currently act on tickets. Only SLA breach → escalation is active.

| Action | Effect | Close Chance | Cost |
|--------|--------|--------------|------|
| Acknowledge | Pauses SLA timer for 60s | 0% | Free |
| Apologize | May close ticket | 50% | Free |
| Partial Refund | Usually closes ticket | 80% | 25% of session |
| Full Refund | Always closes ticket | 100% | 100% of session |
| Dispatch Technician | Resolves underlying issue | 100% | Tech time |
| Ignore | Ticket escalates | 0% | Reputation damage |

```rust
pub enum TicketResolution {
    Acknowledge,
    Apologize,
    PartialRefund,
    FullRefund,
    DispatchTechnician,
    Ignore,
}

impl TicketResolution {
    /// Chance to close the ticket (0.0 - 1.0)
    pub fn close_chance(&self) -> f32 {
        match self {
            TicketResolution::Acknowledge => 0.0,
            TicketResolution::Apologize => 0.50,
            TicketResolution::PartialRefund => 0.80,
            TicketResolution::FullRefund => 1.0,
            TicketResolution::DispatchTechnician => 1.0,
            TicketResolution::Ignore => 0.0,
        }
    }
}
```

### 5.6 Procedural Driver Messages

Messages are generated from templates based on ticket type:

```rust
fn generate_driver_message(ticket_type: TicketType) -> String {
    let variations: &[&str] = match ticket_type {
        TicketType::BillingDispute => &[
            "I was charged more than expected. Can you check my session?",
            "The price on my receipt doesn't match the charger. Why?",
            "I think I was double-charged for my last session.",
        ],
        TicketType::SessionDidntStart => &[
            "I've been waiting and the charger won't start. Help!",
            "I tapped my card but nothing is happening. Is it on?",
            "The app says it's charging but the car says otherwise.",
        ],
        TicketType::ConnectorStuck => &[
            "The cable is stuck and I can't unplug my car!",
            "I finished charging but the lock won't release. I'm trapped!",
            "The connector is jammed in the port. Please help!",
        ],
        TicketType::SlowCharging => &[
            "Charging is way slower than advertised. What's going on?",
            "I'm only getting 30kW on a 350kW charger. This is slow!",
            "It says 'Ultra-fast' but it's charging like a wall outlet.",
        ],
        TicketType::AppError => &[
            "The app keeps giving me errors. Can't authenticate.",
            "Your app is crashing every time I try to start a session.",
            "Login failed. I can't even get past the splash screen.",
        ],
    };

    let idx = (rand::random::<f32>() * variations.len() as f32) as usize;
    variations[idx % variations.len()].to_string()
}
```

---

## 6. Priority System

### 6.1 Problem: Everything Happens at Once

In a realistic sim, multiple issues occur simultaneously. The player must triage.

### 6.2 Dynamic Priority Calculation

Priority score (0-100+) is computed dynamically:

```rust
impl Ticket {
    pub fn update_priority(&mut self, game_time: f32, driver_waiting: bool) {
        let elapsed_minutes = (game_time - self.created_at) / 60.0;
        let time_bonus = (elapsed_minutes * 10.0) as i32;
        let waiting_bonus = if driver_waiting { 20 } else { 0 };
        let sla_urgency = if self.time_until_breach(game_time) < 60.0 { 30 } else { 0 };

        self.priority = self.ticket_type.base_priority() 
            + time_bonus 
            + waiting_bonus 
            + sla_urgency;
    }
}
```

| Factor | Bonus |
|--------|-------|
| Base (from ticket type) | 30-80 |
| +10 per minute elapsed | Variable |
| Driver physically waiting | +20 |
| SLA < 60 seconds | +30 |

---

## 7. Consequences of Neglect

| Neglect Duration | Consequence |
|------------------|-------------|
| SLA breach | Reputation −5, ticket escalated (BillingDispute: 2x session value chargeback; others: $25 penalty) |
| 5+ minutes offline | Drivers leave, no revenue |
| Repeated failures | Site reputation decline |
| Ignored tickets | Potential chargebacks |

---

## 8. Event Flow

### 8.1 Charger Fault → Ticket Creation

```
Charger develops fault
    ↓
Observer: on_charger_fault_global
    ├── Updates charger state
    ├── Creates Ticket entity
    └── Fires ChargerFaultEvent
    ↓
UI: Toast notification appears
UI: Ticket shows in sidebar
```

### 8.2 Remote Action Flow

```
Player clicks charger → Radial menu
Player selects remote action
    ↓
System: RemoteActionRequestEvent fired
    ↓
System: action_system processes
    ├── Validates cooldown
    ├── Rolls for success
    ├── Executes action
    └── Fires RemoteActionResultEvent
    ↓
UI: Toast shows success/failure
```

### 8.3 Technician Dispatch Flow

```
Player clicks "Dispatch Technician"
    ↓
System: TechnicianDispatchEvent fired
    ↓
System: dispatch_technician_system
    ├── Queues dispatch if busy
    └── Starts travel if idle
    ↓
System: technician_travel_system
    └── Updates travel_remaining each frame
    ↓
System: Spawns Technician entity on arrival
    ↓
System: technician_movement_system
    └── Pathfind to charger
    ↓
System: technician_repair_system
    └── Timer counts down
    ↓
System: RepairCompleteEvent fired
    ├── Clears charger fault
    ├── Deducts repair cost
    └── Technician walks to exit
```

---

## 9. Summary

This spec defines three layers of operational interaction:

1. **Remote Ops**: Fast, limited, accessed via radial menu
2. **Physical Tasks**: Slower, requires technician dispatch with travel time
3. **Customer Support**: Ticket-based, SLA-driven, reputation-affecting

Together, these systems create the "plate-spinning" feel of running a charging network.

---

*Document version: 2.1*
*Last updated: February 2026*
