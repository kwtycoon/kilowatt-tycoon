//! Site upgrades resource - purchasable efficiency improvements

use bevy::prelude::*;

/// Costs for each upgrade in dollars
pub mod upgrade_costs {
    pub const SMART_LOAD_SHEDDING: f32 = 8000.0;
    pub const ADVANCED_POWER_MANAGEMENT: f32 = 25000.0;
    pub const MARKETING_CAMPAIGN: f32 = 3000.0;
    pub const DYNAMIC_PRICING: f32 = 15000.0;
    // O&M platform tiers
    pub const OEM_DETECT: f32 = 5000.0;
    pub const OEM_OPTIMIZE: f32 = 20000.0;
    // Repeatable demand boost
    pub const DEMAND_BOOST: f32 = 500.0;
    // Infosec tiers
    pub const CYBER_FIREWALL: f32 = 12_000.0;
    pub const AGENTIC_SOC: f32 = 35_000.0;
}

/// Demand boost: 2x demand for 4 game hours
pub const DEMAND_BOOST_MULTIPLIER: f32 = 2.0;
pub const DEMAND_BOOST_DURATION_SECS: f32 = 14_400.0;

/// O&M platform tiers - determines fault detection, auto-remediation, and repair capabilities
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum OemTier {
    /// No O&M platform - faults detected only when a driver tries to use the charger
    #[default]
    None,
    /// Tier 1 "Detect" - immediate fault detection, auto-remediation of software faults
    Detect,
    /// Tier 2 "Optimize" - adds auto-dispatch, faster repairs, better reliability recovery
    Optimize,
}

impl OemTier {
    /// Fault detection delay in game seconds.
    /// Without O&M, faults are detected only when a driver tries to use the charger.
    /// With O&M, detection is near-instant.
    pub fn detection_delay_secs(&self) -> Option<f32> {
        match self {
            OemTier::None => None, // No automatic detection - waits for driver
            OemTier::Detect | OemTier::Optimize => Some(10.0), // Near-instant detection
        }
    }

    /// Repair time multiplier (lower = faster repairs due to better diagnostics/dispatch)
    pub fn repair_time_multiplier(&self) -> f32 {
        match self {
            OemTier::None | OemTier::Detect => 1.0,
            OemTier::Optimize => 0.75, // 25% faster
        }
    }

    /// How fast charger reliability recovers after faults are resolved
    pub fn reliability_recovery_multiplier(&self) -> f32 {
        match self {
            OemTier::None => 1.0,
            OemTier::Detect => 1.5,
            OemTier::Optimize => 2.0,
        }
    }

    /// Whether auto-remediation is available (auto-fix software faults without player action)
    pub fn has_auto_remediation(&self) -> bool {
        !matches!(self, OemTier::None)
    }

    /// Whether auto-dispatch is available (auto-dispatch technician on fault, re-dispatch on failure)
    pub fn has_auto_dispatch(&self) -> bool {
        matches!(self, OemTier::Optimize)
    }

    pub fn display_name(&self) -> &'static str {
        match self {
            OemTier::None => "No O&M Platform",
            OemTier::Detect => "O&M: Detect",
            OemTier::Optimize => "O&M: Optimize",
        }
    }

    pub fn description(&self) -> &'static str {
        match self {
            OemTier::None => {
                "No monitoring platform — faults detected only when a driver tries to charge"
            }
            OemTier::Detect => {
                "Immediate fault detection, auto-remediation of software faults, 1.5x reliability recovery, OPEX controls"
            }
            OemTier::Optimize => {
                "Auto-dispatch technicians, 25% faster repairs, 2x reliability recovery"
            }
        }
    }

    /// Cost to upgrade TO this tier (incremental from previous tier)
    pub fn upgrade_cost(&self) -> f32 {
        match self {
            OemTier::None => 0.0,
            OemTier::Detect => upgrade_costs::OEM_DETECT,
            OemTier::Optimize => upgrade_costs::OEM_OPTIMIZE,
        }
    }

    /// Next tier (if any)
    pub fn next_tier(&self) -> Option<OemTier> {
        match self {
            OemTier::None => Some(OemTier::Detect),
            OemTier::Detect => Some(OemTier::Optimize),
            OemTier::Optimize => None,
        }
    }

    /// Whether this tier is at least the given tier
    pub fn at_least(&self, other: OemTier) -> bool {
        self.tier_level() >= other.tier_level()
    }

    fn tier_level(&self) -> u8 {
        match self {
            OemTier::None => 0,
            OemTier::Detect => 1,
            OemTier::Optimize => 2,
        }
    }
}

/// Tracks purchased site upgrades
#[derive(Resource, Debug, Clone, Default)]
pub struct SiteUpgrades {
    /// Smart Load Shedding - auto-throttles charger power when transformer overheats
    pub has_smart_load_shedding: bool,
    /// Advanced power management - unlocks power density and battery controls
    pub has_advanced_power_management: bool,
    /// Marketing campaign - +10% demand (temporary or permanent depending on design)
    pub has_marketing: bool,
    /// Dynamic pricing engine - unlocks TOU, cost-plus, and surge pricing modes
    pub has_dynamic_pricing: bool,
    /// O&M platform tier
    pub oem_tier: OemTier,
    /// Remaining game-seconds on the active demand boost (0 = inactive)
    pub demand_boost_remaining_secs: f32,
    /// Cyber Firewall — reduces hacker spawn chance and attack success
    pub has_cyber_firewall: bool,
    /// Agentic SOC — advanced AI-driven infosec (requires Firewall)
    pub has_agentic_soc: bool,
}

impl SiteUpgrades {
    /// Get the repair time multiplier (lower = faster repairs)
    /// Delegated to OEM tier
    pub fn repair_time_multiplier(&self) -> f32 {
        self.oem_tier.repair_time_multiplier()
    }

    /// Combined demand multiplier from marketing + active demand boost.
    pub fn demand_multiplier(&self) -> f32 {
        let marketing = if self.has_marketing { 1.1 } else { 1.0 };
        let boost = if self.is_demand_boost_active() {
            DEMAND_BOOST_MULTIPLIER
        } else {
            1.0
        };
        marketing * boost
    }

    pub fn is_demand_boost_active(&self) -> bool {
        self.demand_boost_remaining_secs > 0.0
    }

    pub fn activate_demand_boost(&mut self) {
        self.demand_boost_remaining_secs = DEMAND_BOOST_DURATION_SECS;
    }

    pub fn tick_demand_boost(&mut self, delta_game_secs: f32) {
        if self.demand_boost_remaining_secs > 0.0 {
            self.demand_boost_remaining_secs =
                (self.demand_boost_remaining_secs - delta_game_secs).max(0.0);
        }
    }

    /// Human-readable time remaining for the demand boost (e.g. "3h 15m").
    pub fn demand_boost_time_remaining_display(&self) -> String {
        let total_secs = self.demand_boost_remaining_secs.ceil() as u32;
        let hours = total_secs / 3600;
        let minutes = (total_secs % 3600) / 60;
        if hours > 0 {
            format!("{hours}h {minutes:02}m")
        } else {
            format!("{minutes}m")
        }
    }

    /// Whether smart load shedding is active.
    /// When purchased, the site auto-throttles charger power delivery as the
    /// transformer temperature climbs, preventing runaway overload fires.
    pub fn has_smart_load_shedding(&self) -> bool {
        self.has_smart_load_shedding
    }

    /// Check if advanced power management is active (unlocks power density + battery controls)
    pub fn has_power_management(&self) -> bool {
        self.has_advanced_power_management
    }

    /// Check if dynamic pricing engine is active (unlocks TOU, cost-plus, surge modes)
    pub fn has_dynamic_pricing(&self) -> bool {
        self.has_dynamic_pricing
    }

    /// Whether any O&M tier is active
    pub fn has_om_software(&self) -> bool {
        self.oem_tier != OemTier::None
    }

    /// Calculate average charger uptime percentage based on upgrades
    pub fn estimated_uptime_percent(&self) -> f32 {
        let base_uptime: f32 = 85.0;
        let oem_bonus: f32 = match self.oem_tier {
            OemTier::None => 0.0,
            OemTier::Detect => 5.0,
            OemTier::Optimize => 10.0,
        };
        let shedding_bonus: f32 = if self.has_smart_load_shedding {
            3.0
        } else {
            0.0
        };
        (base_uptime + oem_bonus + shedding_bonus).min(99.0)
    }

    /// Get upgrade info for display
    pub fn upgrade_info() -> Vec<UpgradeInfo> {
        vec![
            UpgradeInfo {
                id: UpgradeId::DemandBoost,
                name: UpgradeId::DemandBoost.display_name(),
                description: "2x customer demand for 4 hours",
                cost: upgrade_costs::DEMAND_BOOST,
            },
            UpgradeInfo {
                id: UpgradeId::Marketing,
                name: UpgradeId::Marketing.display_name(),
                description: "+10% customer demand",
                cost: upgrade_costs::MARKETING_CAMPAIGN,
            },
            UpgradeInfo {
                id: UpgradeId::SmartLoadShedding,
                name: UpgradeId::SmartLoadShedding.display_name(),
                description: "Auto-throttles chargers when transformer overheats",
                cost: upgrade_costs::SMART_LOAD_SHEDDING,
            },
            UpgradeInfo {
                id: UpgradeId::DynamicPricing,
                name: UpgradeId::DynamicPricing.display_name(),
                description: "Unlocks TOU, cost-plus, and surge pricing modes",
                cost: upgrade_costs::DYNAMIC_PRICING,
            },
            UpgradeInfo {
                id: UpgradeId::AdvancedPowerManagement,
                name: UpgradeId::AdvancedPowerManagement.display_name(),
                description: "Unlocks power density + battery controls",
                cost: upgrade_costs::ADVANCED_POWER_MANAGEMENT,
            },
            UpgradeInfo {
                id: UpgradeId::OemDetect,
                name: UpgradeId::OemDetect.display_name(),
                description: "Immediate fault detection, auto-remediation",
                cost: upgrade_costs::OEM_DETECT,
            },
            UpgradeInfo {
                id: UpgradeId::OemOptimize,
                name: UpgradeId::OemOptimize.display_name(),
                description: "Auto-dispatch technicians, 25% faster repairs",
                cost: upgrade_costs::OEM_OPTIMIZE,
            },
            UpgradeInfo {
                id: UpgradeId::CyberFirewall,
                name: UpgradeId::CyberFirewall.display_name(),
                description: "Reduces hacker attacks by 60%, 50% block rate",
                cost: upgrade_costs::CYBER_FIREWALL,
            },
            UpgradeInfo {
                id: UpgradeId::AgenticSoc,
                name: UpgradeId::AgenticSoc.display_name(),
                description: "AI infosec: 90% fewer hackers, 95% block, auto-terminate",
                cost: upgrade_costs::AGENTIC_SOC,
            },
        ]
    }

    /// Check if an upgrade is purchased (DemandBoost is never "purchased" — it's repeatable).
    pub fn is_purchased(&self, id: UpgradeId) -> bool {
        match id {
            UpgradeId::SmartLoadShedding => self.has_smart_load_shedding,
            UpgradeId::AdvancedPowerManagement => self.has_advanced_power_management,
            UpgradeId::Marketing => self.has_marketing,
            UpgradeId::DynamicPricing => self.has_dynamic_pricing,
            UpgradeId::OemDetect => self.oem_tier.at_least(OemTier::Detect),
            UpgradeId::OemOptimize => self.oem_tier.at_least(OemTier::Optimize),
            UpgradeId::DemandBoost => false,
            UpgradeId::CyberFirewall => self.has_cyber_firewall,
            UpgradeId::AgenticSoc => self.has_agentic_soc,
        }
    }

    /// Purchase an upgrade (sets the flag to true, or activates the boost timer).
    pub fn purchase(&mut self, id: UpgradeId) {
        match id {
            UpgradeId::SmartLoadShedding => self.has_smart_load_shedding = true,
            UpgradeId::AdvancedPowerManagement => self.has_advanced_power_management = true,
            UpgradeId::Marketing => self.has_marketing = true,
            UpgradeId::DynamicPricing => self.has_dynamic_pricing = true,
            UpgradeId::OemDetect => {
                if self.oem_tier == OemTier::None {
                    self.oem_tier = OemTier::Detect;
                }
            }
            UpgradeId::OemOptimize => {
                if self.oem_tier.at_least(OemTier::Detect) {
                    self.oem_tier = OemTier::Optimize;
                }
            }
            UpgradeId::DemandBoost => self.activate_demand_boost(),
            UpgradeId::CyberFirewall => self.has_cyber_firewall = true,
            UpgradeId::AgenticSoc => {
                if self.has_cyber_firewall {
                    self.has_agentic_soc = true;
                }
            }
        }
    }

    /// Get the cost of an upgrade
    pub fn get_cost(id: UpgradeId) -> f32 {
        match id {
            UpgradeId::SmartLoadShedding => upgrade_costs::SMART_LOAD_SHEDDING,
            UpgradeId::AdvancedPowerManagement => upgrade_costs::ADVANCED_POWER_MANAGEMENT,
            UpgradeId::Marketing => upgrade_costs::MARKETING_CAMPAIGN,
            UpgradeId::DynamicPricing => upgrade_costs::DYNAMIC_PRICING,
            UpgradeId::OemDetect => upgrade_costs::OEM_DETECT,
            UpgradeId::OemOptimize => upgrade_costs::OEM_OPTIMIZE,
            UpgradeId::DemandBoost => upgrade_costs::DEMAND_BOOST,
            UpgradeId::CyberFirewall => upgrade_costs::CYBER_FIREWALL,
            UpgradeId::AgenticSoc => upgrade_costs::AGENTIC_SOC,
        }
    }

    /// Check if an OEM tier upgrade can be purchased (requires previous tier)
    pub fn can_purchase_oem(&self, id: UpgradeId) -> bool {
        match id {
            UpgradeId::OemDetect => self.oem_tier == OemTier::None,
            UpgradeId::OemOptimize => self.oem_tier == OemTier::Detect,
            UpgradeId::AgenticSoc => self.has_cyber_firewall && !self.has_agentic_soc,
            UpgradeId::CyberFirewall => !self.has_cyber_firewall,
            _ => true,
        }
    }
}

/// Upgrade identifier
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum UpgradeId {
    SmartLoadShedding,
    AdvancedPowerManagement,
    Marketing,
    DynamicPricing,
    OemDetect,
    OemOptimize,
    /// Repeatable temporary demand boost
    DemandBoost,
    /// Tier 1 infosec — firewall reduces hacker spawn/success
    CyberFirewall,
    /// Tier 2 infosec — agentic SOC auto-blocks attacks (requires Firewall)
    AgenticSoc,
}

impl UpgradeId {
    pub fn display_name(&self) -> &'static str {
        match self {
            UpgradeId::SmartLoadShedding => "Smart Load Shedding",
            UpgradeId::AdvancedPowerManagement => "Advanced Power Management",
            UpgradeId::Marketing => "Marketing Campaign",
            UpgradeId::DynamicPricing => "Dynamic Pricing Engine",
            UpgradeId::OemDetect => "O&M: Detect",
            UpgradeId::OemOptimize => "O&M: Optimize",
            UpgradeId::DemandBoost => "Demand Blitz",
            UpgradeId::CyberFirewall => "Cyber Firewall",
            UpgradeId::AgenticSoc => "Agentic SOC",
        }
    }
}

/// Info about an upgrade for UI display
#[derive(Debug, Clone)]
pub struct UpgradeInfo {
    pub id: UpgradeId,
    pub name: &'static str,
    pub description: &'static str,
    pub cost: f32,
}
