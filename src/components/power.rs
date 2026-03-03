//! Power system components

use bevy::prelude::*;

use super::charger::Phase;
use crate::resources::SiteId;

/// Hysteresis-stabilized visual tier for the transformer sprite.
/// Prevents flickering when temperature oscillates near threshold boundaries.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum TransformerVisualTier {
    #[default]
    Normal,
    Warning,
    Critical,
}

/// Site transformer component - one entity per physical transformer
#[derive(Component, Debug, Clone)]
pub struct Transformer {
    /// Which site this transformer belongs to
    pub site_id: SiteId,
    /// Grid position (anchor tile, bottom-left of 2x2)
    pub grid_pos: (i32, i32),
    /// Transformer rating in kVA
    pub rating_kva: f32,
    pub thermal_limit_c: f32,
    pub current_temp_c: f32,
    /// Current apparent power load in kVA (includes reactive power)
    pub current_load_kva: f32,
    pub ambient_temp_c: f32,
    /// Accumulated seconds spent above overload threshold.
    pub overload_seconds: f32,
    /// True while transformer is actively burning.
    pub on_fire: bool,
    /// True after a fire is extinguished; requires demolition/replacement.
    pub destroyed: bool,
    /// Set once a firetruck has been dispatched for the active incident.
    pub firetruck_dispatched: bool,
    /// Tracks which warning level has already been emitted (0=none, 1=warning, 2=critical).
    pub last_warning_level: u8,
    /// Hysteresis-stabilized visual tier for sprite selection.
    pub visual_tier: TransformerVisualTier,
}

impl Default for Transformer {
    fn default() -> Self {
        Self {
            site_id: SiteId(0),
            grid_pos: (0, 0),
            rating_kva: 75.0,
            thermal_limit_c: 110.0,
            current_temp_c: 25.0,
            current_load_kva: 0.0,
            ambient_temp_c: 25.0,
            overload_seconds: 0.0,
            on_fire: false,
            destroyed: false,
            firetruck_dispatched: false,
            last_warning_level: 0,
            visual_tier: TransformerVisualTier::default(),
        }
    }
}

impl Transformer {
    pub fn load_percentage(&self) -> f32 {
        (self.current_load_kva / self.rating_kva * 100.0).clamp(0.0, 150.0)
    }

    pub fn update_temperature(&mut self, delta_seconds: f32) {
        let load_ratio = self.current_load_kva / self.rating_kva;
        // Temperature rises quadratically with load
        let target_temp =
            self.ambient_temp_c + (self.thermal_limit_c - self.ambient_temp_c) * load_ratio.powi(2);

        // Exponential approach to target (thermal mass)
        let thermal_constant = 0.02;
        self.current_temp_c +=
            (target_temp - self.current_temp_c) * thermal_constant * delta_seconds;
    }

    pub fn is_warning(&self) -> bool {
        self.current_temp_c >= 75.0
    }

    pub fn is_critical(&self) -> bool {
        self.current_temp_c >= 90.0
    }

    pub fn is_overloaded(&self) -> bool {
        self.current_load_kva > self.rating_kva * 1.1
    }

    /// How much faster the fire countdown ticks based on excess load.
    /// At or below rated capacity the base rate is 1x; each additional 10%
    /// overload adds 0.3x (with the default multiplier of 3.0).
    pub fn excess_pull_factor(&self) -> f32 {
        const EXCESS_PULL_ACCELERATION: f32 = 3.0;
        let load_ratio = self.current_load_kva / self.rating_kva;
        1.0 + (load_ratio - 1.0).max(0.0) * EXCESS_PULL_ACCELERATION
    }

    /// Update the visual tier with 3C hysteresis margins to prevent sprite flickering.
    pub fn update_visual_tier(&mut self) {
        self.visual_tier = match self.visual_tier {
            TransformerVisualTier::Normal => {
                if self.current_temp_c >= 78.0 {
                    TransformerVisualTier::Warning
                } else {
                    TransformerVisualTier::Normal
                }
            }
            TransformerVisualTier::Warning => {
                if self.current_temp_c >= 93.0 {
                    TransformerVisualTier::Critical
                } else if self.current_temp_c < 72.0 {
                    TransformerVisualTier::Normal
                } else {
                    TransformerVisualTier::Warning
                }
            }
            TransformerVisualTier::Critical => {
                if self.current_temp_c < 87.0 {
                    TransformerVisualTier::Warning
                } else {
                    TransformerVisualTier::Critical
                }
            }
        };
    }
}

/// Tracks apparent power load (kVA) per electrical phase
#[derive(Resource, Debug, Clone, Default)]
pub struct PhaseLoads {
    pub phase_a_kva: f32,
    pub phase_b_kva: f32,
    pub phase_c_kva: f32,
}

impl PhaseLoads {
    pub fn get_load(&self, phase: Phase) -> f32 {
        match phase {
            Phase::A => self.phase_a_kva,
            Phase::B => self.phase_b_kva,
            Phase::C => self.phase_c_kva,
        }
    }

    /// Add apparent power load (kVA) to a phase
    pub fn add_load(&mut self, phase: Phase, kva: f32) {
        match phase {
            Phase::A => self.phase_a_kva += kva,
            Phase::B => self.phase_b_kva += kva,
            Phase::C => self.phase_c_kva += kva,
        }
    }

    /// Total apparent power load (kVA) across all phases
    pub fn total_load(&self) -> f32 {
        self.phase_a_kva + self.phase_b_kva + self.phase_c_kva
    }

    pub fn reset(&mut self) {
        self.phase_a_kva = 0.0;
        self.phase_b_kva = 0.0;
        self.phase_c_kva = 0.0;
    }

    /// Calculate imbalance percentage (0 = balanced, 100 = max imbalance)
    pub fn imbalance_percentage(&self) -> f32 {
        let total = self.total_load();
        if total < 0.1 {
            return 0.0;
        }

        let avg = total / 3.0;
        let max_deviation = (self.phase_a_kva - avg)
            .abs()
            .max((self.phase_b_kva - avg).abs())
            .max((self.phase_c_kva - avg).abs());

        (max_deviation / avg * 100.0).min(100.0)
    }

    pub fn phase_percentage(&self, phase: Phase, capacity_per_phase: f32) -> f32 {
        if capacity_per_phase < 0.1 {
            return 0.0;
        }
        (self.get_load(phase) / capacity_per_phase * 100.0).clamp(0.0, 150.0)
    }
}

/// Voltage state for derating calculations
#[derive(Resource, Debug, Clone)]
pub struct VoltageState {
    pub nominal_voltage: f32,
    pub current_voltage_pct: f32,
}

impl Default for VoltageState {
    fn default() -> Self {
        Self {
            nominal_voltage: 400.0,
            current_voltage_pct: 100.0,
        }
    }
}

impl VoltageState {
    /// Calculate derating factor based on voltage sag
    pub fn derating_factor(&self) -> f32 {
        match self.current_voltage_pct {
            v if v >= 95.0 => 1.0,
            v if v >= 90.0 => 0.9,
            v if v >= 85.0 => 0.75,
            v if v >= 80.0 => 0.5,
            _ => 0.0, // Charger trips
        }
    }

    pub fn is_warning(&self) -> bool {
        self.current_voltage_pct < 95.0
    }
}
