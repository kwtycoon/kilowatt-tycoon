//! Asset handles for preloaded PNG sprites and audio
//!
//! Note: The original SVG files are the source assets. PNGs are generated
//! using `python3 tools/convert_svgs_to_pngs.py` (requires Inkscape).

use bevy::prelude::*;

/// Resource holding all preloaded image asset handles
#[derive(Resource, Default)]
pub struct ImageAssets {
    // Tiles
    pub tile_grass: Handle<Image>,
    pub tile_asphalt: Handle<Image>,
    pub tile_asphalt_lines: Handle<Image>,
    pub tile_concrete: Handle<Image>,
    pub tile_curb_grass: Handle<Image>,
    pub tile_curb_concrete: Handle<Image>,
    // Gas station tiles
    pub tile_store_wall: Handle<Image>,
    pub tile_store_entrance: Handle<Image>,
    pub tile_storefront: Handle<Image>,
    pub tile_pump_island: Handle<Image>,
    pub tile_canopy_floor: Handle<Image>,
    pub tile_canopy_shadow: Handle<Image>,
    pub tile_fuel_cap_covered: Handle<Image>,
    // Worn asphalt variations
    pub tile_asphalt_worn: Handle<Image>,
    pub tile_asphalt_skid: Handle<Image>,
    // Mall/Garage tiles
    pub tile_garage_floor: Handle<Image>,
    pub tile_garage_pillar: Handle<Image>,
    pub tile_mall_facade: Handle<Image>,
    // Workplace tiles
    pub tile_reserved_spot: Handle<Image>,
    pub tile_office_backdrop: Handle<Image>,
    // Transit tiles
    pub tile_loading_zone: Handle<Image>,
    // Other tiles
    pub tile_planter: Handle<Image>,

    // Vehicles
    pub vehicle_compact: Handle<Image>,
    pub vehicle_sedan: Handle<Image>,
    pub vehicle_suv: Handle<Image>,
    pub vehicle_crossover: Handle<Image>,
    pub vehicle_pickup: Handle<Image>,
    pub vehicle_bus: Handle<Image>,
    pub vehicle_semi: Handle<Image>,
    pub vehicle_tractor: Handle<Image>,
    pub vehicle_scooter: Handle<Image>,
    pub vehicle_motorcycle: Handle<Image>,

    // Chargers - DCFC 50kW (compact, budget)
    pub charger_dcfc50_available: Handle<Image>,
    pub charger_dcfc50_charging: Handle<Image>,
    pub charger_dcfc50_offline: Handle<Image>,
    pub charger_dcfc50_warning: Handle<Image>,
    pub charger_dcfc50_cable_stuck: Handle<Image>,

    // Chargers - DCFC 100kW (standard, built-in ad screen)
    pub charger_dcfc100_available: Handle<Image>,
    pub charger_dcfc100_charging: Handle<Image>,
    pub charger_dcfc100_offline: Handle<Image>,
    pub charger_dcfc100_warning: Handle<Image>,
    pub charger_dcfc100_cable_stuck: Handle<Image>,
    pub charger_dcfc100_screen: Handle<Image>,

    // Chargers - DCFC 150kW (standard)
    pub charger_dcfc150_available: Handle<Image>,
    pub charger_dcfc150_charging: Handle<Image>,
    pub charger_dcfc150_offline: Handle<Image>,
    pub charger_dcfc150_warning: Handle<Image>,
    pub charger_dcfc150_cable_stuck: Handle<Image>,

    // Chargers - DCFC 350kW (premium, flagship)
    pub charger_dcfc350_available: Handle<Image>,
    pub charger_dcfc350_charging: Handle<Image>,
    pub charger_dcfc350_offline: Handle<Image>,
    pub charger_dcfc350_warning: Handle<Image>,
    pub charger_dcfc350_cable_stuck: Handle<Image>,

    // Chargers - L2
    pub charger_l2_available: Handle<Image>,
    pub charger_l2_charging: Handle<Image>,
    pub charger_l2_offline: Handle<Image>,
    pub charger_l2_warning: Handle<Image>,
    pub charger_l2_cable_stuck: Handle<Image>,

    // Props
    pub prop_transformer: Handle<Image>,
    pub prop_transformer_hot: Handle<Image>,
    pub prop_transformer_critical: Handle<Image>,
    pub prop_solar_array_ground: Handle<Image>,
    pub prop_battery_container: Handle<Image>,
    pub prop_security_system: Handle<Image>,
    pub prop_security_pole: Handle<Image>,
    pub prop_security_camera_head: Handle<Image>,

    // Amenity Buildings
    pub prop_amenity_wifi_restrooms: Handle<Image>,
    pub prop_amenity_lounge_snacks: Handle<Image>,
    pub prop_amenity_restaurant_premium: Handle<Image>,

    // Mood Icons (displayed on vehicles)
    pub icon_mood_neutral: Handle<Image>,
    pub icon_mood_happy: Handle<Image>,
    pub icon_mood_impatient: Handle<Image>,
    pub icon_mood_angry: Handle<Image>,

    // Characters - Technician (male)
    pub character_technician_idle: Handle<Image>,
    pub character_technician_working: Handle<Image>,
    // Characters - Technician (female)
    pub character_technician_female_idle: Handle<Image>,
    pub character_technician_female_working: Handle<Image>,

    // Characters - Main (player-selectable)
    pub character_main_ant: Handle<Image>,
    pub character_main_mallard: Handle<Image>,
    pub character_main_raccoon: Handle<Image>,

    // Characters - Robber (black outfit)
    pub character_robber_walking: Handle<Image>,
    pub character_robber_stealing: Handle<Image>,
    // Characters - Robber (pink outfit)
    pub character_robber_walking_pink: Handle<Image>,
    pub character_robber_stealing_pink: Handle<Image>,
    // Robber loot
    pub stolen_cable: Handle<Image>,

    // Decals
    pub decal_ev_parking: Handle<Image>,
    pub decal_arrow: Handle<Image>,
    pub decal_stall_lines: Handle<Image>,

    // VFX
    pub vfx_selection: Handle<Image>,
    pub vfx_placement_cursor: Handle<Image>,
    pub vfx_light_pulse_green: Handle<Image>,
    pub vfx_light_pulse_blue: Handle<Image>,
    pub vfx_light_pulse_yellow: Handle<Image>,
    pub vfx_light_pulse_red: Handle<Image>,
    pub vfx_urgent_pulse: Handle<Image>,
    pub vfx_float_money: Handle<Image>,
    pub vfx_float_wrench: Handle<Image>,

    // UI Icons
    pub icon_fault: Handle<Image>,
    pub icon_warning: Handle<Image>,
    pub icon_cash: Handle<Image>,
    pub icon_power: Handle<Image>,
    pub icon_reputation: Handle<Image>,
    pub icon_weather_sunny: Handle<Image>,
    pub icon_weather_cloudy: Handle<Image>,
    pub icon_weather_rainy: Handle<Image>,
    pub icon_weather_heatwave: Handle<Image>,
    pub icon_weather_cold: Handle<Image>,
    pub icon_pause: Handle<Image>,
    pub icon_speed_1x: Handle<Image>,
    pub icon_arrow_left: Handle<Image>,
    pub icon_arrow_right: Handle<Image>,
    pub icon_plus: Handle<Image>,
    pub icon_minus: Handle<Image>,
    pub icon_star_filled: Handle<Image>,
    pub icon_star_empty: Handle<Image>,
    pub icon_medal_bronze: Handle<Image>,
    pub icon_medal_silver: Handle<Image>,
    pub icon_medal_gold: Handle<Image>,
    pub icon_dashboard: Handle<Image>,
    pub icon_briefcase: Handle<Image>,
    pub icon_plug: Handle<Image>,
    pub icon_infrastructure: Handle<Image>,
    pub icon_coffee: Handle<Image>,
    pub icon_upgrade: Handle<Image>,
    pub icon_success: Handle<Image>,
    pub icon_technician: Handle<Image>,
    pub icon_marketing: Handle<Image>,
    pub icon_ledger: Handle<Image>,

    // Action icons for radial menu
    pub icon_action_soft_reboot: Handle<Image>,
    pub icon_action_dispatch: Handle<Image>,
    pub icon_action_refund: Handle<Image>,
    pub icon_action_anti_theft: Handle<Image>,

    // Advertisement assets
    pub ad_dancing_banana: Handle<Image>,

    // In-world indicator icons
    pub icon_shield_indicator: Handle<Image>,

    // Title/Platform icons
    pub icon_bolt: Handle<Image>,
    pub icon_platform_windows: Handle<Image>,
    pub icon_platform_macos: Handle<Image>,
    pub icon_platform_linux: Handle<Image>,

    // Toggle button icons
    pub icon_ruler: Handle<Image>,
    pub icon_sound_on: Handle<Image>,
    pub icon_sound_off: Handle<Image>,

    // Splash screen
    pub splash_background: Handle<Image>,
}

impl ImageAssets {
    /// Create an ImageAssets instance with default (empty) handles.
    /// Useful for tests that don't need actual images loaded.
    #[cfg(test)]
    pub fn test() -> Self {
        Self::default()
    }
}

/// Resource holding all preloaded audio asset handles
#[derive(Resource, Default)]
pub struct AudioAssets {
    pub alarm_theft: Handle<AudioSource>,
}

/// System to load all audio assets on startup
pub fn load_audio_assets(mut commands: Commands, asset_server: Res<AssetServer>) {
    let assets = AudioAssets {
        alarm_theft: asset_server.load("sounds/alarm_theft.wav"),
    };
    commands.insert_resource(assets);
}

/// System to load all image assets on startup
pub fn load_image_assets(mut commands: Commands, asset_server: Res<AssetServer>) {
    let assets = ImageAssets {
        // Tiles
        tile_grass: asset_server.load("world/tiles/tile_grass.png"),
        tile_asphalt: asset_server.load("world/tiles/tile_asphalt_clean.png"),
        tile_asphalt_lines: asset_server.load("world/tiles/tile_asphalt_lines.png"),
        tile_concrete: asset_server.load("world/tiles/tile_concrete.png"),
        tile_curb_grass: asset_server.load("world/tiles/tile_curb_asphalt_grass.png"),
        tile_curb_concrete: asset_server.load("world/tiles/tile_curb_asphalt_concrete.png"),
        // Gas station tiles
        tile_store_wall: asset_server.load("world/tiles/tile_store_wall.png"),
        tile_store_entrance: asset_server.load("world/tiles/tile_store_entrance.png"),
        tile_storefront: asset_server.load("world/tiles/tile_storefront.png"),
        tile_pump_island: asset_server.load("world/tiles/tile_pump_island.png"),
        tile_canopy_floor: asset_server.load("world/tiles/tile_canopy_floor.png"),
        tile_canopy_shadow: asset_server.load("world/tiles/tile_canopy_shadow.png"),
        tile_fuel_cap_covered: asset_server.load("world/tiles/tile_fuel_cap_covered.png"),
        // Worn asphalt variations
        tile_asphalt_worn: asset_server.load("world/tiles/tile_asphalt_worn.png"),
        tile_asphalt_skid: asset_server.load("world/tiles/tile_asphalt_skid.png"),
        // Mall/Garage tiles
        tile_garage_floor: asset_server.load("world/tiles/tile_garage_floor.png"),
        tile_garage_pillar: asset_server.load("world/tiles/tile_garage_pillar.png"),
        tile_mall_facade: asset_server.load("world/tiles/tile_mall_facade.png"),
        // Workplace tiles
        tile_reserved_spot: asset_server.load("world/tiles/tile_reserved_spot.png"),
        tile_office_backdrop: asset_server.load("world/tiles/tile_office_backdrop.png"),
        // Transit tiles
        tile_loading_zone: asset_server.load("world/tiles/tile_loading_zone.png"),
        // Other tiles
        tile_planter: asset_server.load("world/tiles/tile_planter.png"),

        // Gas station props
        // Vehicles
        vehicle_compact: asset_server.load("vehicles/vehicle_compact.png"),
        vehicle_sedan: asset_server.load("vehicles/vehicle_sedan.png"),
        vehicle_suv: asset_server.load("vehicles/vehicle_suv.png"),
        vehicle_crossover: asset_server.load("vehicles/vehicle_crossover.png"),
        vehicle_pickup: asset_server.load("vehicles/vehicle_pickup.png"),
        vehicle_bus: asset_server.load("vehicles/vehicle_bus.png"),
        vehicle_semi: asset_server.load("vehicles/vehicle_semi.png"),
        vehicle_tractor: asset_server.load("vehicles/vehicle_tractor.png"),
        vehicle_scooter: asset_server.load("vehicles/vehicle_scooter.png"),
        vehicle_motorcycle: asset_server.load("vehicles/vehicle_motorcycle.png"),

        // Chargers - DCFC 50kW (compact, budget)
        charger_dcfc50_available: asset_server.load("chargers/dcfc50/charger_dcfc50_available.png"),
        charger_dcfc50_charging: asset_server.load("chargers/dcfc50/charger_dcfc50_charging.png"),
        charger_dcfc50_offline: asset_server.load("chargers/dcfc50/charger_dcfc50_offline.png"),
        charger_dcfc50_warning: asset_server.load("chargers/dcfc50/charger_dcfc50_warning.png"),
        charger_dcfc50_cable_stuck: asset_server
            .load("chargers/dcfc50/charger_dcfc50_cable_stuck.png"),

        // Chargers - DCFC 100kW (standard, built-in ad screen)
        charger_dcfc100_available: asset_server
            .load("chargers/dcfc100/charger_dcfc100_available.png"),
        charger_dcfc100_charging: asset_server
            .load("chargers/dcfc100/charger_dcfc100_charging.png"),
        charger_dcfc100_offline: asset_server.load("chargers/dcfc100/charger_dcfc100_offline.png"),
        charger_dcfc100_warning: asset_server.load("chargers/dcfc100/charger_dcfc100_warning.png"),
        charger_dcfc100_cable_stuck: asset_server
            .load("chargers/dcfc100/charger_dcfc100_cable_stuck.png"),
        charger_dcfc100_screen: asset_server.load("chargers/dcfc100/charger_dcfc100_screen.png"),

        // Chargers - DCFC 150kW (standard)
        charger_dcfc150_available: asset_server
            .load("chargers/dcfc150/charger_dcfc150_available.png"),
        charger_dcfc150_charging: asset_server
            .load("chargers/dcfc150/charger_dcfc150_charging.png"),
        charger_dcfc150_offline: asset_server.load("chargers/dcfc150/charger_dcfc150_offline.png"),
        charger_dcfc150_warning: asset_server.load("chargers/dcfc150/charger_dcfc150_warning.png"),
        charger_dcfc150_cable_stuck: asset_server
            .load("chargers/dcfc150/charger_dcfc150_cable_stuck.png"),

        // Chargers - DCFC 350kW (premium, flagship)
        charger_dcfc350_available: asset_server
            .load("chargers/dcfc350/charger_dcfc350_available.png"),
        charger_dcfc350_charging: asset_server
            .load("chargers/dcfc350/charger_dcfc350_charging.png"),
        charger_dcfc350_offline: asset_server.load("chargers/dcfc350/charger_dcfc350_offline.png"),
        charger_dcfc350_warning: asset_server.load("chargers/dcfc350/charger_dcfc350_warning.png"),
        charger_dcfc350_cable_stuck: asset_server
            .load("chargers/dcfc350/charger_dcfc350_cable_stuck.png"),

        // Chargers - L2
        charger_l2_available: asset_server.load("chargers/l2/charger_l2_available.png"),
        charger_l2_charging: asset_server.load("chargers/l2/charger_l2_charging.png"),
        charger_l2_offline: asset_server.load("chargers/l2/charger_l2_offline.png"),
        charger_l2_warning: asset_server.load("chargers/l2/charger_l2_warning.png"),
        charger_l2_cable_stuck: asset_server.load("chargers/l2/charger_l2_cable_stuck.png"),

        // Props
        prop_transformer: asset_server.load("props/prop_transformer.png"),
        prop_transformer_hot: asset_server.load("props/prop_transformer_hot.png"),
        prop_transformer_critical: asset_server.load("props/prop_transformer_critical.png"),
        prop_solar_array_ground: asset_server.load("props/prop_solar_array_ground.png"),
        prop_battery_container: asset_server.load("props/prop_battery_container.png"),
        prop_security_system: asset_server.load("props/prop_security_system.png"),
        prop_security_pole: asset_server.load("props/prop_security_pole.png"),
        prop_security_camera_head: asset_server.load("props/prop_security_camera_head.png"),

        // Amenity Buildings
        prop_amenity_wifi_restrooms: asset_server.load("props/prop_amenity_wifi_restrooms.png"),
        prop_amenity_lounge_snacks: asset_server.load("props/prop_amenity_lounge_snacks.png"),
        prop_amenity_restaurant_premium: asset_server
            .load("props/prop_amenity_restaurant_premium.png"),

        // Mood Icons (displayed on vehicles)
        icon_mood_neutral: asset_server.load("ui/icons/icon_mood_neutral.png"),
        icon_mood_happy: asset_server.load("ui/icons/icon_mood_happy.png"),
        icon_mood_impatient: asset_server.load("ui/icons/icon_mood_impatient.png"),
        icon_mood_angry: asset_server.load("ui/icons/icon_mood_angry.png"),

        // Characters - Technician (male)
        character_technician_idle: asset_server.load("characters/character_technician_idle.png"),
        character_technician_working: asset_server
            .load("characters/character_technician_working.png"),
        // Characters - Technician (female)
        character_technician_female_idle: asset_server
            .load("characters/character_technician_female_idle.png"),
        character_technician_female_working: asset_server
            .load("characters/character_technician_female_working.png"),

        // Characters - Main (player-selectable)
        character_main_ant: asset_server.load("characters/Main/Ant.png"),
        character_main_mallard: asset_server.load("characters/Main/Mallard.png"),
        character_main_raccoon: asset_server.load("characters/Main/Raccoon.png"),

        // Characters - Robber (black outfit)
        character_robber_walking: asset_server.load("characters/character_robber_walking.png"),
        character_robber_stealing: asset_server.load("characters/character_robber_stealing.png"),
        // Characters - Robber (pink outfit)
        character_robber_walking_pink: asset_server
            .load("characters/character_robber_walking_pink.png"),
        character_robber_stealing_pink: asset_server
            .load("characters/character_robber_stealing_pink.png"),
        stolen_cable: asset_server.load("characters/stolen_cable.png"),

        // Decals
        decal_ev_parking: asset_server.load("world/decals/decal_ev_parking.png"),
        decal_arrow: asset_server.load("world/decals/decal_arrow.png"),
        decal_stall_lines: asset_server.load("world/decals/decal_stall_lines.png"),

        // VFX
        vfx_selection: asset_server.load("vfx/vfx_selection.png"),
        vfx_placement_cursor: asset_server.load("vfx/vfx_placement_cursor.png"),
        vfx_light_pulse_green: asset_server.load("vfx/vfx_light_pulse_green.png"),
        vfx_light_pulse_blue: asset_server.load("vfx/vfx_light_pulse_blue.png"),
        vfx_light_pulse_yellow: asset_server.load("vfx/vfx_light_pulse_yellow.png"),
        vfx_light_pulse_red: asset_server.load("vfx/vfx_light_pulse_red.png"),
        vfx_urgent_pulse: asset_server.load("vfx/vfx_urgent_pulse.png"),
        vfx_float_money: asset_server.load("vfx/vfx_float_money.png"),
        vfx_float_wrench: asset_server.load("vfx/vfx_float_wrench.png"),

        // UI Icons
        icon_fault: asset_server.load("ui/icons/icon_fault.png"),
        icon_warning: asset_server.load("ui/icons/icon_warning.png"),
        icon_cash: asset_server.load("ui/icons/icon_cash.png"),
        icon_power: asset_server.load("ui/icons/icon_power.png"),
        icon_reputation: asset_server.load("ui/icons/icon_reputation.png"),
        icon_weather_sunny: asset_server.load("ui/icons/icon_weather_sunny.png"),
        icon_weather_cloudy: asset_server.load("ui/icons/icon_weather_cloudy.png"),
        icon_weather_rainy: asset_server.load("ui/icons/icon_weather_rainy.png"),
        icon_weather_heatwave: asset_server.load("ui/icons/icon_weather_heatwave.png"),
        icon_weather_cold: asset_server.load("ui/icons/icon_weather_cold.png"),
        icon_pause: asset_server.load("ui/icons/icon_pause.png"),
        icon_speed_1x: asset_server.load("ui/icons/icon_speed_1x.png"),
        icon_arrow_left: asset_server.load("ui/icons/icon_arrow_left.png"),
        icon_arrow_right: asset_server.load("ui/icons/icon_arrow_right.png"),
        icon_plus: asset_server.load("ui/icons/icon_plus.png"),
        icon_minus: asset_server.load("ui/icons/icon_minus.png"),
        icon_star_filled: asset_server.load("ui/icons/icon_star_filled.png"),
        icon_star_empty: asset_server.load("ui/icons/icon_star_empty.png"),
        icon_medal_bronze: asset_server.load("ui/icons/icon_medal_bronze.png"),
        icon_medal_silver: asset_server.load("ui/icons/icon_medal_silver.png"),
        icon_medal_gold: asset_server.load("ui/icons/icon_medal_gold.png"),
        icon_dashboard: asset_server.load("ui/icons/icon_dashboard.png"),
        icon_briefcase: asset_server.load("ui/icons/icon_briefcase.png"),
        icon_plug: asset_server.load("ui/icons/icon_plug.png"),
        icon_infrastructure: asset_server.load("ui/icons/icon_infrastructure.png"),
        icon_coffee: asset_server.load("ui/icons/icon_coffee.png"),
        icon_upgrade: asset_server.load("ui/icons/icon_upgrade.png"),
        icon_success: asset_server.load("ui/icons/icon_success.png"),
        icon_technician: asset_server.load("ui/icons/icon_technician.png"),
        icon_marketing: asset_server.load("ui/icons/icon_reputation.png"),
        icon_ledger: asset_server.load("ui/icons/icon_ledger.png"),

        // Action icons for radial menu
        icon_action_soft_reboot: asset_server.load("ui/icons/icon_action_soft_reboot.png"),
        icon_action_dispatch: asset_server.load("ui/icons/icon_action_dispatch.png"),
        icon_action_refund: asset_server.load("ui/icons/icon_action_refund.png"),
        icon_action_anti_theft: asset_server.load("ui/icons/icon_action_anti_theft.png"),

        // Advertisement assets
        ad_dancing_banana: asset_server.load("ads/dancing-banana.gif"),

        // In-world indicator icons
        icon_shield_indicator: asset_server.load("ui/icons/icon_shield_indicator.png"),

        // Title/Platform icons
        icon_bolt: asset_server.load("ui/icons/icon_bolt.png"),
        icon_platform_windows: asset_server.load("ui/icons/icon_platform_windows.png"),
        icon_platform_macos: asset_server.load("ui/icons/icon_platform_macos.png"),
        icon_platform_linux: asset_server.load("ui/icons/icon_platform_linux.png"),

        // Toggle button icons
        icon_ruler: asset_server.load("ui/icons/icon_ruler.png"),
        icon_sound_on: asset_server.load("ui/icons/icon_sound_on.png"),
        icon_sound_off: asset_server.load("ui/icons/icon_sound_off.png"),

        // Splash screen
        splash_background: asset_server.load("ui/splash_background.png"),
    };

    commands.insert_resource(assets);
}

impl ImageAssets {
    pub fn technician_idle(&self, gender: crate::resources::TechnicianGender) -> Handle<Image> {
        match gender {
            crate::resources::TechnicianGender::Male => self.character_technician_idle.clone(),
            crate::resources::TechnicianGender::Female => {
                self.character_technician_female_idle.clone()
            }
        }
    }

    pub fn technician_working(&self, gender: crate::resources::TechnicianGender) -> Handle<Image> {
        match gender {
            crate::resources::TechnicianGender::Male => self.character_technician_working.clone(),
            crate::resources::TechnicianGender::Female => {
                self.character_technician_female_working.clone()
            }
        }
    }
}
