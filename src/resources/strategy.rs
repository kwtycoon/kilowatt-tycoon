//! Service strategy resource - the player's "recipe" for running their charging station

use bevy::math::ops;
use bevy::prelude::*;

use crate::components::charger::{ChargerType, FaultType};

use super::{AmenityType, MultiSiteManager, SiteEnergyConfig, SiteGrid};

/// Policy controlling when excess solar generation is exported to the grid.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum SolarExportPolicy {
    /// No export; excess solar is curtailed (wasted).
    #[default]
    Never,
    /// Export only surplus after self-consumption and BESS charging.
    ExcessOnly,
    /// Prioritize grid export over BESS charging (skips storing excess solar in battery).
    MaxExport,
}

impl SolarExportPolicy {
    pub fn display_name(&self) -> &'static str {
        match self {
            SolarExportPolicy::Never => "Never",
            SolarExportPolicy::ExcessOnly => "Excess Only",
            SolarExportPolicy::MaxExport => "Max Export",
        }
    }

    pub fn next(&self) -> Self {
        match self {
            SolarExportPolicy::Never => SolarExportPolicy::ExcessOnly,
            SolarExportPolicy::ExcessOnly => SolarExportPolicy::MaxExport,
            SolarExportPolicy::MaxExport => SolarExportPolicy::Never,
        }
    }

    pub fn prev(&self) -> Self {
        match self {
            SolarExportPolicy::Never => SolarExportPolicy::MaxExport,
            SolarExportPolicy::ExcessOnly => SolarExportPolicy::Never,
            SolarExportPolicy::MaxExport => SolarExportPolicy::ExcessOnly,
        }
    }
}

/// Extended warranty tier — controls how much of the parts cost is covered on dispatch.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum WarrantyTier {
    /// No coverage, no premium.
    #[default]
    None,
    /// Covers GroundFault and CableDamage parts at 100%. Does NOT cover CableTheft.
    Standard,
    /// Covers ALL fault parts at 100%, including CableTheft cable replacement.
    Comprehensive,
}

impl WarrantyTier {
    pub fn display_name(&self) -> &'static str {
        match self {
            WarrantyTier::None => "None",
            WarrantyTier::Standard => "Standard",
            WarrantyTier::Comprehensive => "Full",
        }
    }

    pub fn description(&self) -> &'static str {
        match self {
            WarrantyTier::None => "No warranty coverage",
            WarrantyTier::Standard => "Covers: Ground Fault, Cable Damage parts",
            WarrantyTier::Comprehensive => "Covers: All fault parts incl. Cable Theft",
        }
    }

    pub fn next(&self) -> Self {
        match self {
            WarrantyTier::None => WarrantyTier::Standard,
            WarrantyTier::Standard => WarrantyTier::Comprehensive,
            WarrantyTier::Comprehensive => WarrantyTier::None,
        }
    }

    pub fn prev(&self) -> Self {
        match self {
            WarrantyTier::None => WarrantyTier::Comprehensive,
            WarrantyTier::Standard => WarrantyTier::None,
            WarrantyTier::Comprehensive => WarrantyTier::Standard,
        }
    }

    /// Multiplier applied to parts cost at dispatch. 0.0 = fully covered, 1.0 = no coverage.
    pub fn parts_cost_multiplier(&self, fault_type: FaultType) -> f32 {
        match self {
            WarrantyTier::None => 1.0,
            WarrantyTier::Standard => match fault_type {
                FaultType::GroundFault | FaultType::CableDamage => 0.0,
                _ => 1.0,
            },
            WarrantyTier::Comprehensive => match fault_type {
                FaultType::GroundFault | FaultType::CableDamage | FaultType::CableTheft => 0.0,
                _ => 1.0,
            },
        }
    }

    /// Monthly premium for a single charger at this warranty tier.
    pub fn charger_monthly_premium(&self, charger_type: ChargerType, rated_power_kw: f32) -> f32 {
        match self {
            WarrantyTier::None => 0.0,
            WarrantyTier::Standard => match charger_type {
                ChargerType::AcLevel2 => 40.0,
                ChargerType::DcFast => {
                    if rated_power_kw <= 50.0 {
                        60.0
                    } else if rated_power_kw <= 100.0 {
                        75.0
                    } else if rated_power_kw <= 150.0 {
                        90.0
                    } else {
                        110.0
                    }
                }
            },
            WarrantyTier::Comprehensive => match charger_type {
                ChargerType::AcLevel2 => 65.0,
                ChargerType::DcFast => {
                    if rated_power_kw <= 50.0 {
                        175.0
                    } else if rated_power_kw <= 100.0 {
                        250.0
                    } else if rated_power_kw <= 150.0 {
                        325.0
                    } else {
                        500.0
                    }
                }
            },
        }
    }
}

/// Customer-facing pricing mode — controls how the energy sell price is computed each tick.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum PricingMode {
    /// Single fixed price (the original behaviour).
    #[default]
    Flat,
    /// Two prices that auto-switch with the utility TOU schedule.
    TouLinked,
    /// Markup percentage over the current utility buy rate with floor/ceiling.
    CostPlus,
    /// Surge pricing based on charger utilization.
    DemandResponsive,
}

impl PricingMode {
    pub fn display_name(&self) -> &'static str {
        match self {
            PricingMode::Flat => "Flat",
            PricingMode::TouLinked => "TOU-Linked",
            PricingMode::CostPlus => "Cost-Plus",
            PricingMode::DemandResponsive => "Surge",
        }
    }

    pub fn description(&self) -> &'static str {
        match self {
            PricingMode::Flat => "Fixed price for all customers at all times.",
            PricingMode::TouLinked => {
                "Set separate off-peak and on-peak prices that follow the utility schedule."
            }
            PricingMode::CostPlus => {
                "Automatic markup over your utility buy rate with floor and ceiling."
            }
            PricingMode::DemandResponsive => {
                "Price rises when charger utilization exceeds a threshold."
            }
        }
    }

    pub fn next(&self) -> Self {
        match self {
            PricingMode::Flat => PricingMode::TouLinked,
            PricingMode::TouLinked => PricingMode::CostPlus,
            PricingMode::CostPlus => PricingMode::DemandResponsive,
            PricingMode::DemandResponsive => PricingMode::Flat,
        }
    }

    pub fn prev(&self) -> Self {
        match self {
            PricingMode::Flat => PricingMode::DemandResponsive,
            PricingMode::TouLinked => PricingMode::Flat,
            PricingMode::CostPlus => PricingMode::TouLinked,
            PricingMode::DemandResponsive => PricingMode::CostPlus,
        }
    }
}

// === Dynamic Pricing sub-structs ===

/// Flat pricing: a single fixed $/kWh.
#[derive(Debug, Clone)]
pub struct FlatPricing {
    pub price_kwh: f32,
}

/// TOU-linked pricing: separate prices that switch with the utility schedule.
#[derive(Debug, Clone)]
pub struct TouPricing {
    pub off_peak_price: f32,
    pub on_peak_price: f32,
}

/// Cost-plus pricing: automatic markup over the utility buy rate with floor/ceiling.
#[derive(Debug, Clone)]
pub struct CostPlusPricing {
    /// Markup percentage over utility buy rate (e.g. 200.0 = 200%)
    pub markup_pct: f32,
    pub floor: f32,
    pub ceiling: f32,
}

/// Demand-responsive (surge) pricing based on charger utilization.
#[derive(Debug, Clone)]
pub struct SurgePricing {
    pub base_price: f32,
    pub multiplier: f32,
    /// Utilization fraction (0.0-1.0) where surge begins
    pub threshold: f32,
}

/// All customer-facing sell-price configuration, grouped by mode.
///
/// Lives on `ServiceStrategy` as `.pricing`. Separated so future buy-side
/// TOU dynamics (grid import/export rates, event-driven tariffs) can live
/// on `SiteEnergyConfig` without polluting this struct.
#[derive(Debug, Clone)]
pub struct DynamicPricingConfig {
    pub mode: PricingMode,
    pub flat: FlatPricing,
    pub tou: TouPricing,
    pub cost_plus: CostPlusPricing,
    pub surge: SurgePricing,
}

impl Default for DynamicPricingConfig {
    fn default() -> Self {
        Self {
            mode: PricingMode::Flat,
            flat: FlatPricing { price_kwh: 0.45 },
            tou: TouPricing {
                off_peak_price: 0.30,
                on_peak_price: 0.55,
            },
            cost_plus: CostPlusPricing {
                markup_pct: 200.0,
                floor: 0.20,
                ceiling: 1.00,
            },
            surge: SurgePricing {
                base_price: 0.35,
                multiplier: 1.5,
                threshold: 0.75,
            },
        }
    }
}

impl DynamicPricingConfig {
    /// Validate and clamp all pricing parameters to their allowed ranges.
    pub fn clamp(&mut self) {
        self.flat.price_kwh = self.flat.price_kwh.clamp(0.10, 2.00);
        self.tou.off_peak_price = self.tou.off_peak_price.clamp(0.10, 2.00);
        self.tou.on_peak_price = self.tou.on_peak_price.clamp(0.10, 2.00);
        self.cost_plus.markup_pct = self.cost_plus.markup_pct.clamp(50.0, 2000.0);
        self.cost_plus.floor = self.cost_plus.floor.clamp(0.10, 1.00);
        self.cost_plus.ceiling = self.cost_plus.ceiling.clamp(0.30, 3.00);
        self.surge.base_price = self.surge.base_price.clamp(0.10, 1.50);
        self.surge.multiplier = self.surge.multiplier.clamp(1.0, 3.0);
        self.surge.threshold = self.surge.threshold.clamp(0.50, 0.90);
    }

    /// Compute the current customer-facing energy price based on the active pricing mode.
    pub fn effective_price(
        &self,
        game_time: f32,
        energy_config: &SiteEnergyConfig,
        charger_utilization: f32,
    ) -> f32 {
        match self.mode {
            PricingMode::Flat => self.flat.price_kwh,
            PricingMode::TouLinked => match energy_config.current_tou_period(game_time) {
                crate::resources::TouPeriod::OffPeak => self.tou.off_peak_price,
                crate::resources::TouPeriod::OnPeak => self.tou.on_peak_price,
            },
            PricingMode::CostPlus => {
                let utility_rate = energy_config.current_rate(game_time);
                let markup = utility_rate * (1.0 + self.cost_plus.markup_pct / 100.0);
                markup.clamp(self.cost_plus.floor, self.cost_plus.ceiling)
            }
            PricingMode::DemandResponsive => {
                let util = charger_utilization.clamp(0.0, 1.0);
                if util <= self.surge.threshold {
                    self.surge.base_price
                } else {
                    let ramp =
                        (util - self.surge.threshold) / (1.0 - self.surge.threshold).max(0.01);
                    self.surge.base_price * (1.0 + ramp * (self.surge.multiplier - 1.0))
                }
            }
        }
    }
}

// === ServiceStrategy ===

/// Service strategy - the player's control panel for balancing price, quality, and customer satisfaction
#[derive(Resource, Debug, Clone)]
pub struct ServiceStrategy {
    // === Pricing ===
    pub pricing: DynamicPricingConfig,
    /// Idle fee per minute after charging completes
    pub idle_fee_min: f32,
    /// Video ad space price per hour charged to advertisers
    /// Range: $0.50/hr to $10.00/hr
    /// Higher prices = lower probability of advertisers buying space
    pub ad_space_price_per_hour: f32,

    // === Quality (The "Lemons") ===
    /// Target power delivery multiplier (0.5 - 1.5)
    /// Higher = faster charging but more heat/stress on equipment
    pub target_power_density: f32,

    // === Reliability (The "Sugar") ===
    /// Maintenance investment per game hour
    /// Reduces charger failure rates and speeds up repairs
    pub maintenance_investment: f32,

    // === Comfort (The "Ice") ===
    /// Amenity counts: [WiFi+Restrooms, Lounge+Snacks, Restaurant]
    /// Multiple amenities stack - patience multipliers compound multiplicatively,
    /// OPEX costs sum additively.
    pub amenity_counts: [u32; 3],

    // === Solar Export ===
    /// Controls when excess solar generation is sold back to the grid.
    pub solar_export_policy: SolarExportPolicy,

    // === Extended Warranty ===
    /// Coverage tier for charger parts costs on dispatch.
    pub warranty_tier: WarrantyTier,
}

impl Default for ServiceStrategy {
    fn default() -> Self {
        Self {
            pricing: DynamicPricingConfig::default(),
            idle_fee_min: 0.50,
            ad_space_price_per_hour: 2.0,
            target_power_density: 1.0,
            maintenance_investment: 10.0,
            amenity_counts: [0; 3],
            solar_export_policy: SolarExportPolicy::Never,
            warranty_tier: WarrantyTier::None,
        }
    }
}

impl ServiceStrategy {
    /// Get the patience depletion multiplier based on placed amenities.
    /// Each amenity compounds multiplicatively: 0.85^wifi * 0.70^lounge * 0.50^restaurant
    pub fn patience_multiplier(&self) -> f32 {
        let [wifi, lounge, restaurant] = self.amenity_counts;
        ops::powf(0.85, wifi as f32)
            * ops::powf(0.70, lounge as f32)
            * ops::powf(0.50, restaurant as f32)
    }

    /// Get the failure rate multiplier based on maintenance investment
    /// Higher maintenance = lower failure rates
    pub fn failure_rate_multiplier(&self) -> f32 {
        // $0/hr = 2x failures, $10/hr = 1x, $30+/hr = 0.3x
        let normalized = (self.maintenance_investment / 10.0).clamp(0.0, 3.0);
        (2.0 - normalized * 0.57).max(0.3)
    }

    /// Get hourly OPEX from maintenance investment
    pub fn hourly_maintenance_cost(&self) -> f32 {
        self.maintenance_investment
    }

    /// Get a display string describing placed amenities
    pub fn amenity_name(&self) -> String {
        let [wifi, lounge, restaurant] = self.amenity_counts;
        let mut parts = Vec::new();
        if wifi > 0 {
            parts.push(format!("{}x WiFi", wifi));
        }
        if lounge > 0 {
            parts.push(format!("{}x Lounge", lounge));
        }
        if restaurant > 0 {
            parts.push(format!("{}x Restaurant", restaurant));
        }
        if parts.is_empty() {
            "None".to_string()
        } else {
            parts.join(", ")
        }
    }

    /// Get the total amenity cost per hour (additive across all placed amenities)
    pub fn amenity_cost_per_hour(&self) -> f32 {
        let [wifi, lounge, restaurant] = self.amenity_counts;
        wifi as f32 * 5.0 + lounge as f32 * 15.0 + restaurant as f32 * 35.0
    }

    /// Compute the hourly warranty cost for this site given its charger inventory.
    /// Converts each charger's monthly premium to an hourly rate (monthly / 720).
    pub fn hourly_warranty_cost_for_chargers(
        &self,
        charger_types: impl Iterator<Item = (ChargerType, f32)>,
    ) -> f32 {
        if self.warranty_tier == WarrantyTier::None {
            return 0.0;
        }
        let monthly: f32 = charger_types
            .map(|(ct, kw)| self.warranty_tier.charger_monthly_premium(ct, kw))
            .sum();
        monthly / 720.0
    }

    /// Validate and clamp strategy values
    pub fn clamp(&mut self) {
        self.pricing.clamp();
        self.idle_fee_min = self.idle_fee_min.clamp(0.0, 5.0);
        self.ad_space_price_per_hour = self.ad_space_price_per_hour.clamp(0.50, 10.0);
        self.target_power_density = self.target_power_density.clamp(0.5, 1.5);
        self.maintenance_investment = self.maintenance_investment.clamp(0.0, 50.0);
    }

    /// Calculate the probability of an advertiser buying ad space at the current price.
    /// $0.50/hr = 95% chance, $10.00/hr = 1% chance (linear interpolation)
    pub fn advertiser_interest_probability(&self) -> f32 {
        // Linear interpolation: y = y1 + (x - x1) * (y2 - y1) / (x2 - x1)
        // x1 = 0.5, y1 = 0.95 (95%)
        // x2 = 10.0, y2 = 0.01 (1%)
        let price = self.ad_space_price_per_hour.clamp(0.50, 10.0);
        let prob = 0.95 + (price - 0.50) * (0.01 - 0.95) / (10.0 - 0.50);
        prob.clamp(0.01, 0.95)
    }

    /// Sync amenity counts from placed amenity buildings on the grid
    pub fn sync_from_grid(&mut self, grid: &SiteGrid) {
        self.amenity_counts = Self::count_amenities(&grid.amenities);
    }

    /// Count amenities by type from a list of placed amenities
    fn count_amenities(amenities: &[(i32, i32, AmenityType)]) -> [u32; 3] {
        let mut counts = [0u32; 3];
        for (_, _, amenity_type) in amenities {
            match amenity_type {
                AmenityType::WifiRestrooms => counts[0] += 1,
                AmenityType::LoungeSnacks => counts[1] += 1,
                AmenityType::Restaurant => counts[2] += 1,
            }
        }
        counts
    }
}

/// Sync amenity counts from grid to the active site's per-site ServiceStrategy
pub fn sync_amenity_from_grid(mut multi_site: ResMut<MultiSiteManager>) {
    if !multi_site.is_changed() {
        return;
    }
    let Some(site) = multi_site.active_site_mut() else {
        return;
    };
    let counts = ServiceStrategy::count_amenities(&site.grid.amenities);
    site.service_strategy.amenity_counts = counts;
}

/// Weather type enumeration
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum WeatherType {
    #[default]
    Sunny,
    Overcast,
    Rainy,
    Heatwave,
    Cold,
}

impl WeatherType {
    /// Get display name for weather type
    pub fn display_name(&self) -> &'static str {
        match self {
            WeatherType::Sunny => "☀ Sunny",
            WeatherType::Overcast => "☁ Overcast",
            WeatherType::Rainy => "🌧 Rainy",
            WeatherType::Heatwave => "🔥 Heatwave",
            WeatherType::Cold => "❄ Cold",
        }
    }

    /// Get emoji icon for weather
    pub fn icon(&self) -> &'static str {
        match self {
            WeatherType::Sunny => "☀",
            WeatherType::Overcast => "☁",
            WeatherType::Rainy => "🌧",
            WeatherType::Heatwave => "🔥",
            WeatherType::Cold => "❄",
        }
    }

    /// Get solar generation multiplier for this weather
    pub fn solar_multiplier(&self) -> f32 {
        match self {
            WeatherType::Sunny => 1.0,
            WeatherType::Overcast => 0.6,
            WeatherType::Rainy => 0.3,
            WeatherType::Heatwave => 1.2, // Hot but very sunny
            WeatherType::Cold => 0.9,     // Clear but winter sun
        }
    }

    /// Get patience depletion multiplier for this weather
    pub fn patience_multiplier(&self) -> f32 {
        match self {
            WeatherType::Sunny => 1.0,
            WeatherType::Overcast => 1.0,
            WeatherType::Rainy => 0.9,    // Drivers more patient in rain
            WeatherType::Heatwave => 1.5, // Very impatient in heat
            WeatherType::Cold => 1.2,     // Somewhat impatient in cold
        }
    }

    /// Get charger health derating multiplier
    pub fn charger_health_multiplier(&self) -> f32 {
        match self {
            WeatherType::Sunny => 1.0,
            WeatherType::Overcast => 1.0,
            WeatherType::Rainy => 0.98,    // Slight moisture risk
            WeatherType::Heatwave => 0.85, // Significant heat stress
            WeatherType::Cold => 0.95,     // Cold affects efficiency
        }
    }

    /// Get demand multiplier (arrival rate modifier)
    pub fn demand_multiplier(&self) -> f32 {
        match self {
            WeatherType::Sunny => 1.0,
            WeatherType::Overcast => 1.0,
            WeatherType::Rainy => 0.8,    // Fewer trips
            WeatherType::Heatwave => 0.9, // Some avoid travel
            WeatherType::Cold => 0.85,    // Reduced travel
        }
    }

    /// Wholesale spot price multiplier driven by weather-induced grid stress.
    /// Heatwaves and cold snaps drive up wholesale electricity prices.
    pub fn spot_price_multiplier(&self) -> f32 {
        match self {
            WeatherType::Sunny => 1.0,
            WeatherType::Overcast => 0.9,
            WeatherType::Rainy => 0.7,
            WeatherType::Heatwave => 2.5, // Everyone running AC
            WeatherType::Cold => 1.8,     // Heating load spike
        }
    }
}

/// Environment state - weather and news events
#[derive(Resource, Debug, Clone)]
pub struct EnvironmentState {
    /// Current weather type
    pub current_weather: WeatherType,
    /// Ambient temperature (Fahrenheit)
    pub temperature_f: f32,
    /// Active news headline (if any)
    pub active_news: Option<String>,
    /// Demand multiplier from news events (1.0 = normal, >1.0 = increased demand)
    pub news_demand_multiplier: f32,
    /// Game time when weather last changed
    pub last_weather_change: f32,
    /// Game time when news last changed
    pub last_news_change: f32,
    /// Weather forecast (next 2 periods)
    pub forecast: [WeatherType; 2],
}

impl Default for EnvironmentState {
    fn default() -> Self {
        Self {
            current_weather: WeatherType::Sunny,
            temperature_f: 75.0,
            active_news: None,
            news_demand_multiplier: 1.0,
            last_weather_change: 0.0,
            last_news_change: 0.0,
            forecast: [WeatherType::Sunny, WeatherType::Overcast],
        }
    }
}

impl EnvironmentState {
    /// Get total demand multiplier (weather + news combined)
    pub fn total_demand_multiplier(&self) -> f32 {
        self.current_weather.demand_multiplier() * self.news_demand_multiplier
    }

    /// Get the base temperature for current weather (before site offset)
    fn base_temperature_f(&self) -> f32 {
        match self.current_weather {
            WeatherType::Sunny => 75.0 + (rand::random::<f32>() * 10.0),
            WeatherType::Overcast => 65.0 + (rand::random::<f32>() * 10.0),
            WeatherType::Rainy => 60.0 + (rand::random::<f32>() * 10.0),
            WeatherType::Heatwave => 95.0 + (rand::random::<f32>() * 10.0),
            WeatherType::Cold => 35.0 + (rand::random::<f32>() * 10.0),
        }
    }

    /// Get temperature for current weather
    pub fn update_temperature(&mut self) {
        self.temperature_f = self.base_temperature_f();
    }

    /// Get the effective temperature for a specific site, accounting for its climate offset.
    /// Sites in hotter regions will show higher temperatures, colder regions lower.
    pub fn temperature_for_site(&self, archetype: crate::resources::SiteArchetype) -> f32 {
        self.temperature_f + archetype.temperature_offset_f()
    }

    /// Roll for random weather change
    pub fn roll_weather_change(&mut self, game_time: f32) {
        // Change weather every ~8 game hours (28800 seconds)
        // This ensures weather doesn't change too fast at high game speeds
        if game_time - self.last_weather_change < 28800.0 {
            return;
        }

        // Shift forecast forward
        self.current_weather = self.forecast[0];
        self.forecast[0] = self.forecast[1];

        // Generate new forecast
        self.forecast[1] = match rand::random::<f32>() {
            x if x < 0.35 => WeatherType::Sunny,
            x if x < 0.55 => WeatherType::Overcast,
            x if x < 0.70 => WeatherType::Rainy,
            x if x < 0.85 => WeatherType::Cold,
            _ => WeatherType::Heatwave,
        };

        self.update_temperature();
        self.last_weather_change = game_time;
    }

    /// Roll for random news event
    pub fn roll_news_event(&mut self, game_time: f32) {
        // Change news every ~8 game hours (28800 seconds)
        if game_time - self.last_news_change < 28800.0 {
            return;
        }

        // Random chance of news event (30% chance)
        if rand::random::<f32>() < 0.3 {
            let events = [
                ("Local EV Rally Today!", 1.25),
                ("Gas Prices Soar!", 1.15),
                ("Highway Closed - Detours!", 0.7),
                ("EV Charger Fire Reported Across Town", 0.85),
                ("City Promotes Green Transportation", 1.1),
                ("Weekend Road Trip Season!", 1.3),
                ("Power Grid Alert Issued", 0.9),
            ];

            let idx = (rand::random::<f32>() * events.len() as f32) as usize % events.len();
            let (headline, multiplier) = &events[idx];
            self.active_news = Some(headline.to_string());
            self.news_demand_multiplier = *multiplier;
        } else {
            // Clear news
            self.active_news = None;
            self.news_demand_multiplier = 1.0;
        }

        self.last_news_change = game_time;
    }

    /// Reset environment timers for a new day
    pub fn reset_for_new_day(&mut self) {
        self.last_weather_change = 0.0;
        self.last_news_change = 0.0;
    }
}
