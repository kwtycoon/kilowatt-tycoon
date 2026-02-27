//! Site upgrades resource - purchasable efficiency improvements

use bevy::prelude::*;

/// Costs for each upgrade in dollars
pub mod upgrade_costs {
    pub const TRANSFORMER_COOLING: f32 = 8000.0;
    pub const ADVANCED_POWER_MANAGEMENT: f32 = 25000.0;
    pub const MARKETING_CAMPAIGN: f32 = 3000.0;
    // O&M platform tiers
    pub const OEM_DETECT: f32 = 5000.0;
    pub const OEM_OPTIMIZE: f32 = 20000.0;
}

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
    /// Transformer cooling - +15% thermal headroom
    pub has_transformer_cooling: bool,
    /// Advanced power management - unlocks power density and battery controls
    pub has_advanced_power_management: bool,
    /// Marketing campaign - +10% demand (temporary or permanent depending on design)
    pub has_marketing: bool,
    /// O&M platform tier
    pub oem_tier: OemTier,
}

impl SiteUpgrades {
    /// Get the repair time multiplier (lower = faster repairs)
    /// Delegated to OEM tier
    pub fn repair_time_multiplier(&self) -> f32 {
        self.oem_tier.repair_time_multiplier()
    }

    /// Get the demand bonus from marketing
    /// Marketing campaign adds 10% demand
    pub fn demand_multiplier(&self) -> f32 {
        if self.has_marketing {
            1.1 // +10%
        } else {
            1.0
        }
    }

    /// Get the thermal headroom bonus
    /// Transformer cooling adds 15% headroom (returns as multiplier on max temp)
    pub fn thermal_headroom_multiplier(&self) -> f32 {
        if self.has_transformer_cooling {
            1.15 // 15% more headroom
        } else {
            1.0
        }
    }

    /// Check if advanced power management is active (unlocks power density + battery controls)
    pub fn has_power_management(&self) -> bool {
        self.has_advanced_power_management
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
        let cooling_bonus: f32 = if self.has_transformer_cooling {
            3.0
        } else {
            0.0
        };
        (base_uptime + oem_bonus + cooling_bonus).min(99.0)
    }

    /// Get upgrade info for display
    pub fn upgrade_info() -> Vec<UpgradeInfo> {
        vec![
            UpgradeInfo {
                id: UpgradeId::TransformerCooling,
                name: "Transformer Cooling",
                description: "+15% thermal headroom",
                cost: upgrade_costs::TRANSFORMER_COOLING,
            },
            UpgradeInfo {
                id: UpgradeId::AdvancedPowerManagement,
                name: "Advanced Power Management",
                description: "Unlocks power density + battery controls",
                cost: upgrade_costs::ADVANCED_POWER_MANAGEMENT,
            },
            UpgradeInfo {
                id: UpgradeId::Marketing,
                name: "Marketing Campaign",
                description: "+10% customer demand",
                cost: upgrade_costs::MARKETING_CAMPAIGN,
            },
            UpgradeInfo {
                id: UpgradeId::OemDetect,
                name: "O&M: Detect",
                description: "Immediate fault detection, auto-remediation",
                cost: upgrade_costs::OEM_DETECT,
            },
            UpgradeInfo {
                id: UpgradeId::OemOptimize,
                name: "O&M: Optimize",
                description: "Auto-dispatch technicians, 25% faster repairs",
                cost: upgrade_costs::OEM_OPTIMIZE,
            },
        ]
    }

    /// Check if an upgrade is purchased
    pub fn is_purchased(&self, id: UpgradeId) -> bool {
        match id {
            UpgradeId::TransformerCooling => self.has_transformer_cooling,
            UpgradeId::AdvancedPowerManagement => self.has_advanced_power_management,
            UpgradeId::Marketing => self.has_marketing,
            UpgradeId::OemDetect => self.oem_tier.at_least(OemTier::Detect),
            UpgradeId::OemOptimize => self.oem_tier.at_least(OemTier::Optimize),
        }
    }

    /// Purchase an upgrade (sets the flag to true)
    pub fn purchase(&mut self, id: UpgradeId) {
        match id {
            UpgradeId::TransformerCooling => self.has_transformer_cooling = true,
            UpgradeId::AdvancedPowerManagement => self.has_advanced_power_management = true,
            UpgradeId::Marketing => self.has_marketing = true,
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
        }
    }

    /// Get the cost of an upgrade
    pub fn get_cost(id: UpgradeId) -> f32 {
        match id {
            UpgradeId::TransformerCooling => upgrade_costs::TRANSFORMER_COOLING,
            UpgradeId::AdvancedPowerManagement => upgrade_costs::ADVANCED_POWER_MANAGEMENT,
            UpgradeId::Marketing => upgrade_costs::MARKETING_CAMPAIGN,
            UpgradeId::OemDetect => upgrade_costs::OEM_DETECT,
            UpgradeId::OemOptimize => upgrade_costs::OEM_OPTIMIZE,
        }
    }

    /// Check if an OEM tier upgrade can be purchased (requires previous tier)
    pub fn can_purchase_oem(&self, id: UpgradeId) -> bool {
        match id {
            UpgradeId::OemDetect => self.oem_tier == OemTier::None,
            UpgradeId::OemOptimize => self.oem_tier == OemTier::Detect,
            _ => true, // Non-OEM upgrades have no tier prereqs
        }
    }
}

/// Upgrade identifier
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum UpgradeId {
    TransformerCooling,
    AdvancedPowerManagement,
    Marketing,
    OemDetect,
    OemOptimize,
}

/// Info about an upgrade for UI display
#[derive(Debug, Clone)]
pub struct UpgradeInfo {
    pub id: UpgradeId,
    pub name: &'static str,
    pub description: &'static str,
    pub cost: f32,
}
