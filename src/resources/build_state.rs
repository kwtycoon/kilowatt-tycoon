//! Build mode state and resources

use super::site_grid::StructureSize;
use bevy::prelude::*;

/// Tool types available in build mode
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum BuildTool {
    #[default]
    Select,
    Road,
    ParkingBay,
    ChargerL2,
    ChargerDCFC50,      // 50kW budget DCFC
    ChargerDCFC100,     // 100kW DCFC with built-in video ads
    ChargerDCFC150,     // 150kW standard DCFC
    ChargerDCFC350,     // 350kW premium DCFC
    Transformer100kVA,  // 100 kVA transformer (small sites)
    Transformer500kVA,  // 500 kVA transformer
    Transformer1000kVA, // 1000 kVA transformer
    Transformer2500kVA, // 2500 kVA transformer
    SolarCanopy,
    BatteryStorage,
    SecuritySystem,          // Lot-wide security system (2x2)
    RfBooster,               // RF signal booster (1x1)
    AmenityWifiRestrooms,    // Level 1 amenity
    AmenityLoungeSnacks,     // Level 2 amenity
    AmenityRestaurant,       // Level 3 amenity
    AmenityDriverRestLounge, // Driver Rest Lounge (gig-worker dormitory)
    Sell,
}

impl BuildTool {
    pub fn display_name(&self) -> &'static str {
        match self {
            BuildTool::Select => "Select",
            BuildTool::Road => "Road",
            BuildTool::ParkingBay => "Parking Bay",
            BuildTool::ChargerL2 => "L2 (7kW)",
            BuildTool::ChargerDCFC50 => "DC 50kW",
            BuildTool::ChargerDCFC100 => "DC 100kW +Ads",
            BuildTool::ChargerDCFC150 => "DC 150kW",
            BuildTool::ChargerDCFC350 => "DC 350kW",
            BuildTool::Transformer100kVA => "Transformer 100kVA",
            BuildTool::Transformer500kVA => "Transformer 500kVA",
            BuildTool::Transformer1000kVA => "Transformer 1000kVA",
            BuildTool::Transformer2500kVA => "Transformer 2500kVA",
            BuildTool::SolarCanopy => "Solar 25kW",
            BuildTool::BatteryStorage => "Battery 200kWh",
            BuildTool::SecuritySystem => "Security System",
            BuildTool::RfBooster => "RF Booster",
            BuildTool::AmenityWifiRestrooms => "WiFi+Restrooms",
            BuildTool::AmenityLoungeSnacks => "Lounge+Snacks",
            BuildTool::AmenityRestaurant => "Restaurant",
            BuildTool::AmenityDriverRestLounge => "Driver Rest",
            BuildTool::Sell => "Sell",
        }
    }

    /// Cost to place this item (0 for select/sell)
    pub fn cost(&self) -> i32 {
        match self {
            BuildTool::Select => 0,
            BuildTool::Road => 50,
            BuildTool::ParkingBay => 100,
            BuildTool::ChargerL2 => 3000,
            BuildTool::ChargerDCFC50 => 40000,  // $40k budget DCFC
            BuildTool::ChargerDCFC100 => 60000, // $60k DCFC with built-in video ads
            BuildTool::ChargerDCFC150 => 80000, // $80k standard DCFC
            BuildTool::ChargerDCFC350 => 150000, // $150k premium DCFC
            BuildTool::Transformer100kVA => 25000, // $25k for 100 kVA
            BuildTool::Transformer500kVA => 50000, // $50k for 500 kVA
            BuildTool::Transformer1000kVA => 80000, // $80k for 1000 kVA
            BuildTool::Transformer2500kVA => 175000, // $175k for 2500 kVA
            BuildTool::SolarCanopy => 24000,    // $24k for 3x2 (25kW) solar array
            BuildTool::BatteryStorage => 50000, // $50k for 2x2 (200kWh) battery
            BuildTool::SecuritySystem => 80000, // $80k for 2x2 lot-wide security system
            BuildTool::RfBooster => 25000,      // $25k for 1x1 RF signal booster
            BuildTool::AmenityWifiRestrooms => 15000, // $15k for 3x3 WiFi+Restrooms
            BuildTool::AmenityLoungeSnacks => 50000, // $50k for 4x4 Lounge+Snacks
            BuildTool::AmenityRestaurant => 150000, // $150k for 5x4 Restaurant
            BuildTool::AmenityDriverRestLounge => 25000, // $25k for 3x3 Driver Rest Lounge
            BuildTool::Sell => 0,
        }
    }

    /// Capacity provided by this equipment
    pub fn capacity_info(&self) -> Option<(f32, &'static str)> {
        match self {
            BuildTool::ChargerDCFC50 => Some((50.0, "kW")),
            BuildTool::ChargerDCFC100 => Some((100.0, "kW")),
            BuildTool::ChargerDCFC150 => Some((150.0, "kW")),
            BuildTool::ChargerDCFC350 => Some((350.0, "kW")),
            BuildTool::Transformer100kVA => Some((100.0, "kVA")),
            BuildTool::Transformer500kVA => Some((500.0, "kVA")),
            BuildTool::Transformer1000kVA => Some((1000.0, "kVA")),
            BuildTool::Transformer2500kVA => Some((2500.0, "kVA")),
            BuildTool::SolarCanopy => Some((25.0, "kW")), // 3x2 solar = 25kW
            BuildTool::BatteryStorage => Some((200.0, "kWh")), // 2x2 battery = 200kWh
            BuildTool::SecuritySystem => None, // Security system doesn't have a capacity metric
            _ => None,
        }
    }

    /// Help text shown when the info toggle is active on amenity buttons.
    pub fn description(&self) -> Option<&'static str> {
        match self {
            BuildTool::AmenityWifiRestrooms => Some(
                "Free WiFi and clean restrooms. Slows driver patience drain by 15% per building. 3\u{d7}3 tiles. $5/hr operating cost.",
            ),
            BuildTool::AmenityLoungeSnacks => Some(
                "Comfortable seating, snacks, and coffee. Slows patience drain by 30% per building. 4\u{d7}4 tiles. $15/hr operating cost.",
            ),
            BuildTool::AmenityRestaurant => Some(
                "Full-service dining experience. Slows patience drain by 50% per building. 5\u{d7}4 tiles. $35/hr operating cost.",
            ),
            BuildTool::AmenityDriverRestLounge => Some(
                "Rest area for gig-economy drivers. Slows patience drain by 40% per building. 3\u{d7}3 tiles. $10/hr operating cost.",
            ),
            _ => None,
        }
    }

    /// Check if this is any transformer type
    pub fn is_transformer(&self) -> bool {
        matches!(
            self,
            BuildTool::Transformer100kVA
                | BuildTool::Transformer500kVA
                | BuildTool::Transformer1000kVA
                | BuildTool::Transformer2500kVA
        )
    }

    /// Get the kVA rating for transformer tools
    pub fn transformer_kva(&self) -> Option<f32> {
        match self {
            BuildTool::Transformer100kVA => Some(100.0),
            BuildTool::Transformer500kVA => Some(500.0),
            BuildTool::Transformer1000kVA => Some(1000.0),
            BuildTool::Transformer2500kVA => Some(2500.0),
            _ => None,
        }
    }

    /// Get all placeable tools (excluding select)
    /// Note: Road and ParkingBay are pre-built in templates, so only chargers + transformer are placeable by players
    pub fn all_placeables() -> &'static [BuildTool] {
        &[
            BuildTool::ChargerL2,
            BuildTool::ChargerDCFC50,
            BuildTool::ChargerDCFC100,
            BuildTool::ChargerDCFC150,
            BuildTool::ChargerDCFC350,
            BuildTool::Transformer100kVA,
            BuildTool::Transformer500kVA,
            BuildTool::Transformer1000kVA,
            BuildTool::Transformer2500kVA,
            BuildTool::SolarCanopy,
            BuildTool::BatteryStorage,
            BuildTool::SecuritySystem,
            BuildTool::RfBooster,
            BuildTool::AmenityWifiRestrooms,
            BuildTool::AmenityLoungeSnacks,
            BuildTool::AmenityRestaurant,
            BuildTool::AmenityDriverRestLounge,
        ]
    }

    /// Check if this is any DCFC charger type
    pub fn is_dcfc(&self) -> bool {
        matches!(
            self,
            BuildTool::ChargerDCFC50
                | BuildTool::ChargerDCFC100
                | BuildTool::ChargerDCFC150
                | BuildTool::ChargerDCFC350
        )
    }

    /// Check if this is an amenity building
    pub fn is_amenity(&self) -> bool {
        matches!(
            self,
            BuildTool::AmenityWifiRestrooms
                | BuildTool::AmenityLoungeSnacks
                | BuildTool::AmenityRestaurant
                | BuildTool::AmenityDriverRestLounge
        )
    }

    /// Get the structure size for multi-tile structures
    pub fn structure_size(&self) -> Option<StructureSize> {
        match self {
            BuildTool::Transformer100kVA
            | BuildTool::Transformer500kVA
            | BuildTool::Transformer1000kVA
            | BuildTool::Transformer2500kVA => Some(StructureSize::TwoByTwo),
            BuildTool::SolarCanopy => Some(StructureSize::ThreeByTwo),
            BuildTool::BatteryStorage => Some(StructureSize::TwoByTwo),
            BuildTool::SecuritySystem => Some(StructureSize::TwoByTwo),
            BuildTool::AmenityWifiRestrooms => Some(StructureSize::ThreeByThree),
            BuildTool::AmenityLoungeSnacks => Some(StructureSize::FourByFour),
            BuildTool::AmenityRestaurant => Some(StructureSize::FiveByFour),
            BuildTool::AmenityDriverRestLounge => Some(StructureSize::ThreeByThree),
            _ => None,
        }
    }

    /// Get the structure size for all tools (including single-tile items)
    pub fn footprint_size(&self) -> StructureSize {
        self.structure_size().unwrap_or(StructureSize::Single)
    }

    /// Check if this is any charger type
    pub fn is_charger(&self) -> bool {
        matches!(
            self,
            BuildTool::ChargerL2
                | BuildTool::ChargerDCFC50
                | BuildTool::ChargerDCFC100
                | BuildTool::ChargerDCFC150
                | BuildTool::ChargerDCFC350
        )
    }
}

/// Current build mode state
#[derive(Resource, Debug, Clone)]
pub struct BuildState {
    /// Whether the day has started (simulation/time is running).
    /// Building is allowed regardless of this flag.
    pub is_open: bool,
    /// Currently selected build tool
    pub selected_tool: BuildTool,
    /// Whether we're in drag-placement mode
    pub is_dragging: bool,
    /// Last placed tile (to avoid double-placing while dragging)
    pub last_placed_tile: Option<(i32, i32)>,
}

impl Default for BuildState {
    fn default() -> Self {
        Self {
            is_open: false,
            selected_tool: BuildTool::ChargerL2,
            is_dragging: false,
            last_placed_tile: None,
        }
    }
}

impl BuildState {
    /// Check if we can place items.
    /// Building is now allowed both before and during the day.
    pub fn can_build(&self) -> bool {
        true
    }

    /// Start the day (begin simulation/time progression)
    pub fn start_day(&mut self) {
        self.is_open = true;
    }

    /// Legacy alias for start_day (deprecated, use start_day instead)
    #[allow(dead_code)]
    pub fn open_station(&mut self) {
        self.start_day();
    }

    /// Check if a tool is currently selected
    pub fn is_tool_selected(&self, tool: BuildTool) -> bool {
        self.selected_tool == tool
    }
}

/// Validation result for opening the station
#[derive(Debug, Clone)]
pub struct OpenValidation {
    pub can_open: bool,
    pub issues: Vec<String>,
}

impl OpenValidation {
    pub fn valid() -> Self {
        Self {
            can_open: true,
            issues: Vec::new(),
        }
    }

    pub fn invalid(issues: Vec<String>) -> Self {
        Self {
            can_open: false,
            issues,
        }
    }
}
