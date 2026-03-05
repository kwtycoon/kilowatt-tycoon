//! Dynamic demand system for procedural driver generation
//!
//! This module manages customer arrival rates based on time-of-day,
//! weather, reputation, and other factors.

use bevy::prelude::*;
use rand::Rng;

use crate::components::driver::{PatienceLevel, VehicleType};
use crate::resources::site_config::DriverData;

/// Base demand configuration (customers per game hour before multipliers)
pub const DEFAULT_CUSTOMERS_PER_HOUR: f32 = 3.0;

/// Per-archetype base demand rate.
///
/// ScooterHub is intentionally extreme: HCMC-style corridors with a relentless
/// stream of two-wheelers that never lets up.
pub fn base_demand_for_archetype(archetype: crate::resources::SiteArchetype) -> f32 {
    use crate::resources::SiteArchetype;
    match archetype {
        SiteArchetype::ParkingLot => 3.0,
        SiteArchetype::GasStation => 6.0,
        SiteArchetype::FleetDepot => 4.0,
        SiteArchetype::ScooterHub => 18.0,
    }
}

/// Demand multiplier based on installed charger capacity.
///
/// Larger stations attract more drivers: apps show availability, word spreads,
/// and the station becomes a known destination. Uses `sqrt(count)` so demand
/// grows sub-linearly — doubling chargers doesn't double demand.
pub fn capacity_demand_multiplier(active_charger_count: u32) -> f32 {
    (active_charger_count.max(1) as f32).sqrt()
}

/// Initial procedural spawn delay (game seconds) per archetype.
///
/// ScooterHub floods the player almost immediately.
pub fn initial_spawn_delay_for_archetype(archetype: crate::resources::SiteArchetype) -> f32 {
    use crate::resources::SiteArchetype;
    match archetype {
        SiteArchetype::ScooterHub => 60.0,
        _ => 600.0,
    }
}

/// Demand state resource - tracks procedural driver generation
#[derive(Resource, Debug, Clone)]
pub struct DemandState {
    /// Base customer arrival rate (customers per game hour)
    pub base_customers_per_hour: f32,
    /// Timer tracking time until next spawn attempt (in game seconds)
    pub time_until_next_spawn: f32,
    /// Counter for generating unique procedural driver IDs
    pub procedural_counter: u32,
    /// Whether procedural generation is enabled
    pub enabled: bool,
}

impl Default for DemandState {
    fn default() -> Self {
        Self {
            base_customers_per_hour: DEFAULT_CUSTOMERS_PER_HOUR,
            time_until_next_spawn: 600.0, // Start with 10 game minutes
            procedural_counter: 0,
            enabled: true,
        }
    }
}

impl DemandState {
    /// Calculate effective demand rate combining all multipliers
    ///
    /// Returns customers per game hour.
    /// Uses the default time-of-day curve; prefer
    /// [`calculate_effective_demand_for_archetype`](Self::calculate_effective_demand_for_archetype)
    /// when the site archetype is known.
    pub fn calculate_effective_demand(
        &self,
        reputation: i32,
        weather_multiplier: f32,
        news_multiplier: f32,
        marketing_multiplier: f32,
        hour: u32,
        price_multiplier: f32,
    ) -> f32 {
        self.base_customers_per_hour
            * reputation_factor(reputation)
            * weather_multiplier
            * news_multiplier
            * marketing_multiplier
            * time_of_day_multiplier(hour)
            * price_multiplier
    }

    /// Archetype-aware variant that picks the correct time-of-day curve.
    pub fn calculate_effective_demand_for_archetype(
        &self,
        reputation: i32,
        weather_multiplier: f32,
        news_multiplier: f32,
        marketing_multiplier: f32,
        hour: u32,
        price_multiplier: f32,
        archetype: crate::resources::SiteArchetype,
    ) -> f32 {
        self.base_customers_per_hour
            * reputation_factor(reputation)
            * weather_multiplier
            * news_multiplier
            * marketing_multiplier
            * time_of_day_multiplier_for_archetype(hour, archetype)
            * price_multiplier
    }

    /// Calculate spawn interval in game seconds based on effective demand
    ///
    /// Higher demand = shorter intervals
    pub fn calculate_spawn_interval(&self, effective_demand_per_hour: f32) -> f32 {
        if effective_demand_per_hour <= 0.0 {
            return 3600.0; // Default to 1 hour if demand is zero
        }

        // Convert customers/hour to seconds between customers
        let base_interval = 3600.0 / effective_demand_per_hour;

        // Add some randomness (±20%)
        let mut rng = rand::rng();
        let randomness = rng.random_range(-0.2..0.2);
        base_interval * (1.0 + randomness)
    }

    /// Update the spawn timer
    pub fn tick(&mut self, delta_game_seconds: f32) {
        self.time_until_next_spawn -= delta_game_seconds;
    }

    /// Check if it's time to spawn a new driver
    pub fn should_spawn(&self) -> bool {
        self.enabled && self.time_until_next_spawn <= 0.0
    }

    /// Reset spawn timer with a new interval
    pub fn reset_timer(&mut self, interval: f32) {
        self.time_until_next_spawn = interval;
    }

    /// Increment the procedural counter and return the new value
    pub fn next_id(&mut self) -> u32 {
        self.procedural_counter += 1;
        self.procedural_counter
    }
}

/// Convert reputation (0-100) to a demand multiplier
///
/// - rep 0 = 0.5x (half demand - bad reputation)
/// - rep 50 = 1.0x (baseline)
/// - rep 100 = 1.5x (excellent reputation)
pub fn reputation_factor(rep: i32) -> f32 {
    let clamped_rep = rep.clamp(0, 100);
    0.5 + (clamped_rep as f32 / 100.0)
}

/// Reference price for demand elasticity (the default flat price).
const REFERENCE_PRICE: f32 = 0.45;
/// Elasticity coefficient — how strongly demand reacts to price deviations.
const PRICE_ELASTICITY: f32 = 0.5;

/// Convert a customer-facing price into a demand multiplier.
///
/// Prices below the reference attract more customers; prices above repel.
/// Clamped to `0.3 .. 1.8` to prevent extreme swings.
pub fn price_elasticity_factor(current_price: f32) -> f32 {
    let factor = 1.0 + PRICE_ELASTICITY * (REFERENCE_PRICE - current_price) / REFERENCE_PRICE;
    factor.clamp(0.3, 1.8)
}

/// Get time-of-day demand multiplier based on hour (0-23)
///
/// Models realistic EV charging patterns with peak hours
pub fn time_of_day_multiplier(hour: u32) -> f32 {
    match hour {
        0..=5 => 0.2,   // Night: Minimal traffic
        6 => 0.6,       // Early morning: Starting to pick up
        7..=9 => 1.3,   // Morning rush: Peak commute
        10..=11 => 0.8, // Mid-day: Normal activity
        12..=13 => 1.0, // Lunch: Lunch errands
        14..=16 => 0.9, // Afternoon: Normal activity
        17..=19 => 1.5, // Evening rush: Peak demand
        20..=21 => 1.0, // Evening: Evening activities
        22..=23 => 0.4, // Late night: Winding down
        _ => 1.0,       // Fallback
    }
}

/// Archetype-aware time-of-day multiplier.
///
/// ScooterHub uses a nearly-flat curve because HCMC never sleeps — delivery
/// riders, commuters, and GrabFood couriers keep the station busy 24/7.
pub fn time_of_day_multiplier_for_archetype(
    hour: u32,
    archetype: crate::resources::SiteArchetype,
) -> f32 {
    use crate::resources::SiteArchetype;

    if matches!(archetype, SiteArchetype::ScooterHub) {
        return match hour {
            0..=5 => 0.7,   // HCMC never truly sleeps
            6 => 0.9,       // Early morning ramp
            7..=9 => 1.3,   // Morning commute
            10..=11 => 1.0, // Mid-day steady
            12..=13 => 1.1, // Lunch delivery rush
            14..=16 => 1.0, // Afternoon steady
            17..=19 => 1.4, // Evening peak
            20..=21 => 1.1, // Evening delivery wave
            22..=23 => 0.8, // Late night — still busy
            _ => 1.0,
        };
    }

    time_of_day_multiplier(hour)
}

/// Generate a 12-character lowercase hex string resembling a MAC address (e.g. `0a324bef89cd`).
pub fn generate_evcc_mac(rng: &mut impl Rng) -> String {
    let bytes: [u8; 6] = rng.random();
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

/// Generate a procedural driver with randomized attributes
pub fn generate_procedural_driver(rng: &mut impl Rng, id_counter: u32) -> DriverData {
    let vehicle = random_vehicle_type(rng);

    DriverData {
        id: format!("proc_{id_counter}"),
        evcc_id: Some(generate_evcc_mac(rng)),
        vehicle,
        vehicle_name: random_vehicle_name(rng, vehicle),
        arrival_time: 0.0, // Spawn immediately
        target_charger: None,
        patience: random_patience(rng),
        charge_needed_kwh: charge_needed_for_vehicle(rng, vehicle),
        notes: String::new(),
    }
}

/// Generate a procedural driver with site-aware traffic mix.
///
/// ScooterHub is intentionally dominated by two-wheel vehicles to emulate
/// dense urban scooter corridors.
pub fn generate_procedural_driver_for_site(
    rng: &mut impl Rng,
    id_counter: u32,
    archetype: crate::resources::SiteArchetype,
) -> DriverData {
    let vehicle = random_vehicle_type_for_site(rng, archetype);

    DriverData {
        id: format!("proc_{id_counter}"),
        evcc_id: Some(generate_evcc_mac(rng)),
        vehicle,
        vehicle_name: random_vehicle_name(rng, vehicle),
        arrival_time: 0.0, // Spawn immediately
        target_charger: None,
        patience: random_patience(rng),
        charge_needed_kwh: charge_needed_for_vehicle(rng, vehicle),
        notes: String::new(),
    }
}

/// Generate realistic charge needs based on vehicle type (2-900 kWh range)
pub fn charge_needed_for_vehicle(rng: &mut impl Rng, vehicle: VehicleType) -> f32 {
    match vehicle {
        VehicleType::Compact => rng.random_range(20.0..50.0), // Small battery (Bolt, Leaf)
        VehicleType::Sedan => rng.random_range(30.0..80.0),   // Mid-size (Model 3, Ioniq 6)
        VehicleType::Crossover => rng.random_range(40.0..100.0), // Crossover (Ioniq 5, EV6)
        VehicleType::Suv => rng.random_range(60.0..150.0),    // Large SUV (iX, EQS SUV)
        VehicleType::Pickup => rng.random_range(80.0..250.0), // Trucks (F-150 Lightning, Cybertruck)
        VehicleType::Scooter => rng.random_range(2.0..6.0),   // Small battery (Vespa, NIU)
        VehicleType::Motorcycle => rng.random_range(10.0..20.0), // Mid-size (Zero SR/F, LiveWire)
        VehicleType::Bus => rng.random_range(200.0..500.0),   // Transit bus (BYD K9, Proterra)
        VehicleType::Semi => rng.random_range(400.0..900.0),  // Semi truck (Tesla Semi, eCascadia)
        VehicleType::Tractor => rng.random_range(100.0..300.0), // Farm tractor (Monarch, John Deere)
        VehicleType::Firetruck => rng.random_range(132.0..327.0), // Electric Firetrucks (Rosenbauer RTX, E-ONE Vector)
    }
}

/// Randomly select a vehicle type with realistic distribution
fn random_vehicle_type(rng: &mut impl Rng) -> VehicleType {
    let roll = rng.random::<f32>();
    match roll {
        x if x < 0.12 => VehicleType::Compact,    // 12%
        x if x < 0.38 => VehicleType::Sedan,      // 26%
        x if x < 0.58 => VehicleType::Crossover,  // 20%
        x if x < 0.75 => VehicleType::Suv,        // 17%
        x if x < 0.85 => VehicleType::Pickup,     // 10%
        x if x < 0.89 => VehicleType::Scooter,    // 4%
        x if x < 0.93 => VehicleType::Motorcycle, // 4%
        x if x < 0.96 => VehicleType::Bus,        // 3%
        x if x < 0.97 => VehicleType::Semi,       // 1%
        x if x < 0.98 => VehicleType::Firetruck,  // 1%
        _ => VehicleType::Tractor,                // 2%
    }
}

/// Site-aware vehicle selection used by procedural demand.
fn random_vehicle_type_for_site(
    rng: &mut impl Rng,
    archetype: crate::resources::SiteArchetype,
) -> VehicleType {
    use crate::resources::SiteArchetype;

    let roll = rng.random::<f32>();

    match archetype {
        SiteArchetype::ScooterHub => match roll {
            x if x < 0.78 => VehicleType::Scooter,    // 78%
            x if x < 0.97 => VehicleType::Motorcycle, // 19%
            x if x < 0.985 => VehicleType::Compact,   // 1.5%
            x if x < 0.995 => VehicleType::Sedan,     // 1.0%
            _ => VehicleType::Crossover,              // 0.5%
        },
        SiteArchetype::FleetDepot => match roll {
            x if x < 0.25 => VehicleType::Semi,      // 25%
            x if x < 0.47 => VehicleType::Bus,       // 22%
            x if x < 0.65 => VehicleType::Pickup,    // 18%
            x if x < 0.75 => VehicleType::Tractor,   // 10%
            x if x < 0.80 => VehicleType::Firetruck, // 5%
            x if x < 0.88 => VehicleType::Suv,       // 8%
            x if x < 0.93 => VehicleType::Sedan,     // 5%
            x if x < 0.97 => VehicleType::Crossover, // 4%
            _ => VehicleType::Compact,               // 3%
        },
        _ => random_vehicle_type(rng),
    }
}

/// Public wrapper for fleet systems that need a random vehicle name.
pub fn random_vehicle_name_for_type(rng: &mut impl Rng, vehicle: VehicleType) -> String {
    random_vehicle_name(rng, vehicle)
}

fn random_vehicle_name(rng: &mut impl Rng, vehicle: VehicleType) -> String {
    let names = match vehicle {
        VehicleType::Compact => vec![
            "Chevy Bolt",
            "Nissan Leaf",
            "Mini Cooper SE",
            "Fiat 500e",
            "Honda e",
        ],
        VehicleType::Sedan => vec![
            "Tesla Model 3",
            "BMW i4",
            "Hyundai Ioniq 6",
            "Polestar 2",
            "Mercedes EQE",
        ],
        VehicleType::Crossover => vec![
            "Hyundai Ioniq 5",
            "Kia EV6",
            "Volkswagen ID.4",
            "Ford Mustang Mach-E",
            "Nissan Ariya",
        ],
        VehicleType::Suv => vec![
            "Tesla Model X",
            "BMW iX",
            "Mercedes EQS SUV",
            "Audi e-tron",
            "Cadillac Lyriq",
        ],
        VehicleType::Pickup => vec![
            "Ford F-150 Lightning",
            "Rivian R1T",
            "Tesla Cybertruck",
            "Chevy Silverado EV",
            "GMC Hummer EV",
        ],
        VehicleType::Scooter => vec![
            "Vespa Elettrica",
            "NIU NQi GT",
            "Gogoro Viva",
            "Super Soco CPx",
            "Ather 450X",
        ],
        VehicleType::Motorcycle => vec![
            "Zero SR/F",
            "Energica Ego",
            "Harley LiveWire",
            "Lightning Strike",
            "Damon Hypersport",
        ],
        VehicleType::Bus => vec![
            "BYD K9",
            "Proterra ZX5",
            "New Flyer Xcelsior",
            "Gillig Electric",
            "Lion Electric LionC",
        ],
        VehicleType::Semi => vec![
            "Tesla Semi",
            "Freightliner eCascadia",
            "Volvo VNR Electric",
            "Nikola Tre",
            "Peterbilt 579EV",
        ],
        VehicleType::Tractor => vec![
            "Monarch MK-V",
            "John Deere SESAM",
            "Fendt e100 Vario",
            "Solectrac e70N",
            "Rigitrac SKE 40",
        ],
        VehicleType::Firetruck => vec![
            "Rosenbauer RTX",
            "Pierce Volterra",
            "E-ONE Vector",
            "Magirus M-Case",
        ],
    };

    let idx = (rng.random::<f32>() * names.len() as f32) as usize % names.len();
    names[idx].to_string()
}

/// Randomly select a patience level with realistic distribution
fn random_patience(rng: &mut impl Rng) -> PatienceLevel {
    let roll = rng.random::<f32>();
    match roll {
        x if x < 0.10 => PatienceLevel::VeryLow, // 10%
        x if x < 0.30 => PatienceLevel::Low,     // 20%
        x if x < 0.80 => PatienceLevel::Medium,  // 50%
        _ => PatienceLevel::High,                // 20%
    }
}
