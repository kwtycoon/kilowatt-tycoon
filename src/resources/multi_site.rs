//! Multi-site management - enables renting and operating multiple charging locations

use bevy::{math::Vec2, prelude::*};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

use super::site_grid::SiteGrid;
use super::{
    BessState, ChargerQueue, DemandState, DriverSchedule, GridEventState, GridImport,
    ServiceStrategy, SiteEnergyConfig, SiteUpgrades, SolarState, UtilityMeter,
};
use crate::components::power::{PhaseLoads, VoltageState};

/// Unique identifier for a site instance
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct SiteId(pub u32);

/// Spacing between sites in world coordinates (pixels)
pub const SITE_SPACING: f32 = 2000.0;

/// Site archetype definition
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum SiteArchetype {
    ParkingLot,
    GasStation,
    FleetDepot,
    ScooterHub,
}

impl SiteArchetype {
    /// Get display name for this archetype
    pub fn display_name(&self) -> &'static str {
        match self {
            SiteArchetype::ParkingLot => "Parking Lot",
            SiteArchetype::GasStation => "Gas Station",
            SiteArchetype::FleetDepot => "Fleet Depot",
            SiteArchetype::ScooterHub => "Scooter Alley",
        }
    }

    /// Get description for this archetype
    pub fn description(&self) -> &'static str {
        match self {
            SiteArchetype::ParkingLot => {
                "Retail parking lot with high visibility. Mild climate, ideal for beginners."
            }
            SiteArchetype::GasStation => {
                "Converted fuel station in the Northeast. Cold winters may slow charging during cold snaps."
            }
            SiteArchetype::FleetDepot => {
                "Commercial fleet depot. Endgame scale with 50+ bays, multiple vehicle types, 3 MW power."
            }
            SiteArchetype::ScooterHub => {
                "Ho Chi Minh City-inspired scooter charging alley. Dense L2 traffic and rapid turnover."
            }
        }
    }

    /// Get all archetype variants for iteration
    pub fn all_variants() -> &'static [SiteArchetype] {
        &[
            SiteArchetype::ParkingLot,
            SiteArchetype::GasStation,
            SiteArchetype::FleetDepot,
            SiteArchetype::ScooterHub,
        ]
    }

    /// Get the base ambient temperature offset in Fahrenheit for this site's climate.
    /// Sites have varying climates - some may trigger cold weather penalties.
    /// Positive = hotter climate, negative = colder climate.
    pub fn temperature_offset_f(&self) -> f32 {
        match self {
            SiteArchetype::ParkingLot => 5.0, // Mild climate, beginner-friendly
            SiteArchetype::GasStation => -35.0, // NJ winter: can dip below 0F during cold snaps
            SiteArchetype::FleetDepot => 15.0, // Industrial area, hot
            SiteArchetype::ScooterHub => 15.0, // Ho Chi Minh City tropical climate
        }
    }

    /// Get the base ambient temperature in Celsius for transformer calculations.
    /// Based on the site's climate characteristics.
    pub fn ambient_temp_c(&self) -> f32 {
        // Convert offset from Fahrenheit to Celsius and add to base 25°C
        let offset_c = self.temperature_offset_f() * 5.0 / 9.0;
        25.0 + offset_c
    }

    /// Get charging speed multiplier based on site temperature.
    /// Cold temperatures slow battery charging due to lithium-ion chemistry.
    /// Returns 1.0 for temps >= 32F (0C), scales down to 0.5 at -4F (-20C).
    pub fn cold_charging_multiplier(&self, base_temp_f: f32) -> f32 {
        let effective_temp = base_temp_f + self.temperature_offset_f();
        if effective_temp >= 32.0 {
            1.0 // No penalty at or above freezing (32F / 0C)
        } else if effective_temp <= -4.0 {
            0.5 // Max 50% penalty at -4F (-20C) or colder
        } else {
            // Linear interpolation: 32F -> 1.0, -4F -> 0.5
            // Range is 36 degrees (from 32 to -4)
            let ratio = (effective_temp + 4.0) / 36.0; // 0.0 at -4F, 1.0 at 32F
            0.5 + ratio * 0.5
        }
    }

    /// Whether this archetype is a fleet/commercial depot
    pub fn is_fleet(&self) -> bool {
        matches!(self, SiteArchetype::FleetDepot)
    }

    /// Get a climate warning if this site has extreme temperatures.
    /// Returns a warning message for cold sites that affect charging speed.
    pub fn climate_warning(&self) -> Option<String> {
        let offset = self.temperature_offset_f();
        if offset <= -40.0 {
            Some("Extreme Cold - Charging speeds reduced up to 50%".to_string())
        } else if offset <= -25.0 {
            Some("Cold Winters - Charging reduced during cold snaps".to_string())
        } else if offset <= -10.0 {
            Some("Cool Climate - Mild charging impact in cold weather".to_string())
        } else if offset >= 15.0 {
            Some("Hot & Humid Climate - Elevated thermal stress on equipment".to_string())
        } else {
            None
        }
    }
}

/// Complete state for a single site
///
/// Contains all per-site simulation state to enable concurrent operation.
/// With concurrent architecture, entities persist and are just hidden/shown.
#[derive(Debug, Clone)]
pub struct SiteState {
    pub id: SiteId,
    pub archetype: SiteArchetype,
    pub name: String,
    pub grid: SiteGrid,
    pub grid_capacity_kva: f32, // Hard limit from utility connection
    pub popularity: f32,
    pub challenge_level: u8,

    // Per-site statistics (cumulative)
    pub total_revenue: f32,
    pub total_sessions: i32,

    // Entity tracking (for stats/debugging - entities persist in ECS)
    pub charger_count: usize,
    pub driver_count: usize,

    // Site root entity - all site entities are children of this for transform hierarchy
    pub root_entity: Option<Entity>,

    /// Tiled map entity for visual rendering (from bevy_ecs_tiled)
    pub tiled_map_entity: Option<Entity>,

    // === Per-Site Simulation State (Phase 1) ===
    // Power system state
    pub phase_loads: PhaseLoads,
    pub voltage_state: VoltageState,

    // Driver/Queue management
    pub charger_queue: ChargerQueue,
    pub driver_schedule: DriverSchedule,
    pub demand_state: DemandState,

    // Energy infrastructure
    pub solar_state: SolarState,
    pub bess_state: BessState,
    pub grid_import: GridImport,
    pub utility_meter: UtilityMeter,
    pub site_energy_config: SiteEnergyConfig,
    pub grid_events: GridEventState,

    // Site configuration
    pub service_strategy: ServiceStrategy,
    pub site_upgrades: SiteUpgrades,

    /// Fraction of enabled chargers currently charging (0.0 - 1.0), updated by power dispatch.
    pub charger_utilization: f32,

    /// Maximum vehicles allowed in the lot at once (prevents entrance gridlock)
    pub max_vehicles: usize,

    /// Energy delivered to EVs today (kWh) - reset at end of day for carbon credits
    pub energy_delivered_kwh_today: f32,

    /// Charging sessions completed today (reset at end of day)
    pub sessions_today: i32,

    /// Accumulated opex cost this day, flushed to ledger at day-end
    pub pending_opex: f32,
    /// Accumulated warranty cost this day, flushed to ledger at day-end
    pub pending_warranty: f32,

    /// Pending video ad charger positions - positions where newly placed chargers should have video ads enabled
    pub pending_video_ad_chargers: std::collections::HashSet<(i32, i32)>,

    /// Thermal throttle factor applied to available kVA in dispatch (1.0 = no throttle, 0.25 = severe)
    /// Written by power_system based on hottest transformer temperature, read by power_dispatch_system.
    pub thermal_throttle_factor: f32,

    /// Remaining game-seconds of hacker overload attack (0 = inactive).
    /// While active, load shedding is bypassed and chargers draw max power.
    pub hacker_overload_remaining_secs: f32,
}

impl SiteState {
    /// Create a new site from template info
    pub fn new(
        id: SiteId,
        archetype: SiteArchetype,
        name: String,
        grid_capacity_kva: f32,
        popularity: f32,
        challenge_level: u8,
        _grid_size: (i32, i32),
    ) -> Self {
        // Create a default grid with the specified size
        // TODO: Load actual grid layout from template
        let grid = SiteGrid::default();

        Self {
            id,
            archetype,
            name,
            grid,
            grid_capacity_kva,
            popularity,
            challenge_level,
            total_revenue: 0.0,
            total_sessions: 0,
            charger_count: 0,
            driver_count: 0,
            root_entity: None,
            tiled_map_entity: None,
            // Initialize per-site simulation state
            phase_loads: PhaseLoads::default(),
            voltage_state: VoltageState::default(),
            charger_queue: ChargerQueue::default(),
            driver_schedule: DriverSchedule::default(),
            demand_state: DemandState {
                base_customers_per_hour: crate::resources::demand::base_demand_for_archetype(
                    archetype,
                ),
                time_until_next_spawn: crate::resources::demand::initial_spawn_delay_for_archetype(
                    archetype,
                ),
                ..DemandState::default()
            },
            solar_state: SolarState::default(),
            bess_state: BessState::default(),
            grid_import: GridImport::default(),
            utility_meter: UtilityMeter::default(),
            site_energy_config: SiteEnergyConfig::default(),
            grid_events: GridEventState::default(),
            service_strategy: ServiceStrategy::default(),
            site_upgrades: SiteUpgrades::default(),
            charger_utilization: 0.0,
            max_vehicles: match archetype {
                SiteArchetype::ScooterHub => 50,
                _ => 20,
            },
            energy_delivered_kwh_today: 0.0,
            sessions_today: 0,
            pending_opex: 0.0,
            pending_warranty: 0.0,
            pending_video_ad_chargers: std::collections::HashSet::new(),
            thermal_throttle_factor: 1.0,
            hacker_overload_remaining_secs: 0.0,
        }
    }

    /// Get the effective power capacity for the site.
    ///
    /// - L2 chargers use split-phase 240V directly from the grid (no transformer needed)
    /// - DCFC chargers require a transformer for 480V three-phase
    ///
    /// If there's no transformer and only L2 chargers, returns the grid capacity.
    /// If there are DCFC chargers, returns the minimum of grid and transformer capacity.
    pub fn effective_capacity_kva(&self) -> f32 {
        let transformer_kva = self.grid.total_transformer_capacity();
        let has_dcfc = self
            .grid
            .get_charger_bays()
            .iter()
            .any(|(_, _, ct)| ct.is_dcfc());

        if transformer_kva == 0.0 {
            if has_dcfc {
                // DCFC without transformer = no power for DCFC
                // But L2 chargers can still work from grid capacity
                // Calculate L2-only capacity
                let l2_load_kw: f32 = self
                    .grid
                    .get_charger_bays()
                    .iter()
                    .filter(|(_, _, ct)| !ct.is_dcfc())
                    .map(|(_, _, ct)| ct.power_kw())
                    .sum();
                // L2 can use grid capacity up to their total load
                l2_load_kw.min(self.grid_capacity_kva)
            } else {
                // L2-only site with no transformer: use grid capacity directly
                self.grid_capacity_kva
            }
        } else {
            // With transformer, use minimum of grid and transformer capacity
            self.grid_capacity_kva.min(transformer_kva)
        }
    }

    /// Hard limit for power dispatch: the maximum kVA the site can draw from
    /// the utility grid. Unlike `effective_capacity_kva` (which also caps at
    /// transformer rating), this only enforces the grid connection contract.
    /// Transformer overload is handled thermally — power still flows through an
    /// overloaded transformer, it just heats up faster.
    ///
    /// DCFC chargers still require at least one transformer to be present
    /// (for the 480V step-up); without one, only L2 load is allowed.
    pub fn dispatch_limit_kva(&self) -> f32 {
        let has_transformer = self.grid.has_transformer();
        let has_dcfc = self
            .grid
            .get_charger_bays()
            .iter()
            .any(|(_, _, ct)| ct.is_dcfc());

        if has_dcfc && !has_transformer {
            let l2_load_kw: f32 = self
                .grid
                .get_charger_bays()
                .iter()
                .filter(|(_, _, ct)| !ct.is_dcfc())
                .map(|(_, _, ct)| ct.power_kw())
                .sum();
            l2_load_kw.min(self.grid_capacity_kva)
        } else {
            self.grid_capacity_kva
        }
    }

    /// Get the world offset for this site (for positioning entities)
    /// Each site is positioned at a different location in world space
    /// First site (ID 1) is at (2000, 0), second site (ID 2) at (4000, 0), etc.
    pub fn world_offset(&self) -> Vec2 {
        Vec2::new(self.id.0 as f32 * SITE_SPACING, 0.0)
    }
}

/// Site listing for rental carousel
#[derive(Debug, Clone)]
pub struct SiteListingInfo {
    pub archetype: SiteArchetype,
    pub name: String,
    pub description: String,
    pub rent_cost: f32,         // One-time setup cost
    pub grid_capacity_kva: f32, // HARD LIMIT from utility connection
    pub popularity: f32,        // 0-100, affects demand
    pub challenge_level: u8,    // 1-5 stars (legacy, kept for compatibility)
    pub grid_size: (i32, i32),
}

impl SiteListingInfo {
    /// Check if this is a power-constrained site
    pub fn is_power_constrained(&self) -> bool {
        self.grid_capacity_kva < 100.0
    }

    /// Get a warning message if power is constrained
    pub fn power_warning(&self) -> Option<String> {
        if self.is_power_constrained() {
            Some(format!(
                "Limited Power ({} kVA) - Requires solar/battery for DCFC",
                self.grid_capacity_kva
            ))
        } else {
            None
        }
    }
}

/// Resource managing all sites
#[derive(Resource)]
pub struct MultiSiteManager {
    /// The site currently being viewed/displayed (all sites run concurrently)
    pub viewed_site_id: Option<SiteId>,
    pub owned_sites: HashMap<SiteId, SiteState>,
    pub available_sites: Vec<SiteListingInfo>,
    next_site_id: u32,
}

impl Default for MultiSiteManager {
    fn default() -> Self {
        Self {
            viewed_site_id: None,
            owned_sites: HashMap::new(),
            // Start empty - populated from SiteTemplateCache after assets load
            available_sites: Vec::new(),
            next_site_id: 1,
        }
    }
}

impl MultiSiteManager {
    /// Populate available sites from the loaded template cache
    ///
    /// This should be called after SiteTemplateCache is fully loaded.
    pub fn populate_from_cache(&mut self, cache: &super::SiteTemplateCache) {
        if !cache.loaded {
            warn!("populate_from_cache called before cache is fully loaded");
            return;
        }

        self.available_sites = SiteArchetype::all_variants()
            .iter()
            .filter_map(|&archetype| {
                cache.get(archetype).map(|template| SiteListingInfo {
                    archetype,
                    name: template.name.clone(),
                    description: template.description.clone(),
                    rent_cost: template.rent_cost,
                    grid_capacity_kva: template.grid_capacity_kva,
                    popularity: template.popularity,
                    challenge_level: template.challenge_level,
                    grid_size: (template.grid_size[0], template.grid_size[1]),
                })
            })
            .collect();

        info!(
            "Populated {} available sites from template cache",
            self.available_sites.len()
        );
    }

    /// Check if available sites have been populated
    pub fn is_populated(&self) -> bool {
        !self.available_sites.is_empty()
    }

    /// Rent a site and add it to owned sites
    ///
    /// The `tiled_assets` provides direct TMX data for building the grid (preferred).
    /// The `template_cache` provides fallback layout data if TMX is not available.
    /// The `game_data` provides handles to Tiled maps.
    pub fn rent_site(
        &mut self,
        listing: &SiteListingInfo,
        _template_cache: &super::SiteTemplateCache,
        tiled_assets: &bevy::asset::Assets<bevy_ecs_tiled::prelude::TiledMapAsset>,
        game_data: &super::GameDataAssets,
    ) -> Result<SiteId, String> {
        let site_id = SiteId(self.next_site_id);
        self.next_site_id += 1;

        let mut site_state = SiteState::new(
            site_id,
            listing.archetype,
            listing.name.clone(),
            listing.grid_capacity_kva,
            listing.popularity,
            listing.challenge_level,
            listing.grid_size,
        );

        // Load grid directly from TMX (single source of truth)
        if let Some(tiled_handle) = game_data.tiled_maps.get(&listing.archetype) {
            if let Some(tiled_asset) = tiled_assets.get(tiled_handle) {
                // Extract initial layout to get zone-based entry/exit positions
                let template =
                    crate::data::tiled_loader::extract_template_from_map(&tiled_asset.map);
                if let Some(ref template) = template
                    && let Some(ref layout) = template.initial_layout
                    && let Some(entry_pos) = layout.entry_pos
                    && let Some(exit_pos) = layout.exit_pos
                {
                    site_state.grid = crate::data::tiled_loader::build_site_grid_from_map(
                        &tiled_asset.map,
                        entry_pos,
                        exit_pos,
                    );
                    info!("Loaded grid from TMX for {}", listing.name);
                } else {
                    error!(
                        "TMX for {} missing entry/exit zones - map will not work properly",
                        listing.name
                    );
                }
            } else {
                error!("TMX asset not loaded for {:?}", listing.archetype);
            }
        } else {
            error!("No TMX handle for {:?}", listing.archetype);
        }

        self.owned_sites.insert(site_id, site_state);

        // Set as viewed site if first site
        if self.viewed_site_id.is_none() {
            self.viewed_site_id = Some(site_id);
        }

        Ok(site_id)
    }

    /// Switch to a different owned site
    pub fn switch_to_site(&mut self, site_id: SiteId) -> Result<(), String> {
        if !self.owned_sites.contains_key(&site_id) {
            return Err("Site not owned".to_string());
        }

        self.viewed_site_id = Some(site_id);
        Ok(())
    }

    /// Get the currently viewed site (the one being displayed)
    pub fn active_site(&self) -> Option<&SiteState> {
        self.viewed_site_id.and_then(|id| self.owned_sites.get(&id))
    }

    /// Get mutable reference to viewed site
    pub fn active_site_mut(&mut self) -> Option<&mut SiteState> {
        self.viewed_site_id
            .and_then(|id| self.owned_sites.get_mut(&id))
    }

    /// Get a specific site by ID
    pub fn get_site(&self, site_id: SiteId) -> Option<&SiteState> {
        self.owned_sites.get(&site_id)
    }

    /// Get mutable reference to a specific site
    pub fn get_site_mut(&mut self, site_id: SiteId) -> Option<&mut SiteState> {
        self.owned_sites.get_mut(&site_id)
    }

    /// Get all owned sites, sorted by ID (order of purchase)
    pub fn owned_sites_list(&self) -> Vec<(SiteId, &SiteState)> {
        let mut sites: Vec<(SiteId, &SiteState)> = self
            .owned_sites
            .iter()
            .map(|(id, site)| (*id, site))
            .collect();
        sites.sort_by_key(|(id, _)| *id);
        sites
    }

    /// Get available sites (not yet rented)
    pub fn available_sites_list(&self) -> &[SiteListingInfo] {
        &self.available_sites
    }

    /// Check if a site archetype is already owned
    pub fn owns_archetype(&self, archetype: SiteArchetype) -> bool {
        self.owned_sites
            .values()
            .any(|site| site.archetype == archetype)
    }

    /// Get total revenue across all sites (cumulative)
    pub fn total_revenue_all_sites(&self) -> f32 {
        self.owned_sites
            .values()
            .map(|site| site.total_revenue)
            .sum()
    }

    /// Get total sessions across all sites (cumulative)
    pub fn total_sessions_all_sites(&self) -> i32 {
        self.owned_sites
            .values()
            .map(|site| site.total_sessions)
            .sum()
    }

    /// Calculate the sell value of a site (50% of equipment cost + 20% of total revenue)
    pub fn calculate_site_value(&self, site_id: SiteId) -> Result<f32, String> {
        let site = self
            .get_site(site_id)
            .ok_or_else(|| "Site not owned".to_string())?;

        let grid = &site.grid;
        let mut equipment_value = 0.0;

        // Transformer value (50% depreciation) - count all transformers
        equipment_value += grid.transformer_count() as f32 * 50000.0; // 100k purchase, 50k resale each

        // Charger values (50% depreciation)
        for (_, _, charger_type) in grid.get_charger_bays() {
            equipment_value += match charger_type {
                crate::resources::ChargerPadType::L2 => 1500.0, // 3k → 1.5k
                crate::resources::ChargerPadType::DCFC50 => 20000.0, // 40k → 20k
                crate::resources::ChargerPadType::DCFC100 => 30000.0, // 60k → 30k
                crate::resources::ChargerPadType::DCFC150 => 40000.0, // 80k → 40k
                crate::resources::ChargerPadType::DCFC350 => 75000.0, // 150k → 75k
            };
        }

        // Solar array values (50% depreciation)
        equipment_value += grid.solar_positions.len() as f32 * 12000.0; // 24k → 12k each

        // Battery storage values (50% depreciation)
        equipment_value += grid.battery_positions.len() as f32 * 25000.0; // 50k → 25k each

        // Amenity values (50% depreciation each)
        for (_, _, amenity_type) in &grid.amenities {
            equipment_value += match amenity_type {
                crate::resources::AmenityType::WifiRestrooms => 7500.0, // 15k → 7.5k
                crate::resources::AmenityType::LoungeSnacks => 25000.0, // 50k → 25k
                crate::resources::AmenityType::Restaurant => 75000.0,   // 150k → 75k
            };
        }

        // Add 20% of total revenue earned at this site
        let revenue_bonus = site.total_revenue * 0.2;

        Ok(equipment_value + revenue_bonus)
    }

    /// Sell a site and return the refund amount
    pub fn sell_site(&mut self, site_id: SiteId) -> Result<f32, String> {
        // Validate not the last site
        if self.owned_sites.len() <= 1 {
            return Err("Cannot sell your last site".to_string());
        }

        // Calculate sell value before removing
        let sell_value = self.calculate_site_value(site_id)?;

        // Remove from owned sites
        let _site = self
            .owned_sites
            .remove(&site_id)
            .ok_or_else(|| "Site not owned".to_string())?;

        // If selling the viewed site, switch to another site
        if self.viewed_site_id == Some(site_id) {
            // Switch to the first remaining site
            if let Some((&first_id, _)) = self.owned_sites.iter().next() {
                self.viewed_site_id = Some(first_id);
            } else {
                self.viewed_site_id = None;
            }
        }

        Ok(sell_value)
    }
}
