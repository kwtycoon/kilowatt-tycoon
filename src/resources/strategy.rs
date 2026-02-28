//! Service strategy resource - the player's "recipe" for running their charging station

use bevy::math::ops;
use bevy::prelude::*;

use super::{AmenityType, MultiSiteManager, SiteGrid};

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

/// Service strategy - the player's control panel for balancing price, quality, and customer satisfaction
#[derive(Resource, Debug, Clone)]
pub struct ServiceStrategy {
    // === Pricing (The "Price") ===
    /// Energy price per kWh charged to customers
    pub energy_price_kwh: f32,
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
}

impl Default for ServiceStrategy {
    fn default() -> Self {
        Self {
            energy_price_kwh: 0.45,
            idle_fee_min: 0.50,
            ad_space_price_per_hour: 2.0, // $2/hour default - balanced price/probability
            target_power_density: 1.0,    // 100% of rated power
            maintenance_investment: 10.0, // $10/hour
            amenity_counts: [0; 3],
            solar_export_policy: SolarExportPolicy::Never,
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

    /// Validate and clamp strategy values
    pub fn clamp(&mut self) {
        self.energy_price_kwh = self.energy_price_kwh.clamp(0.10, 2.00);
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
