//! Unit system preference (Imperial / Metric)

use bevy::prelude::*;

/// Which measurement system the player has selected.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum UnitPreference {
    /// Fahrenheit temperatures (default)
    #[default]
    Imperial,
    /// Celsius temperatures
    Metric,
}

impl UnitPreference {
    /// Toggle between Imperial and Metric.
    pub fn toggle(&mut self) {
        *self = match self {
            Self::Imperial => Self::Metric,
            Self::Metric => Self::Imperial,
        };
    }

    /// Short label for display on the toggle button.
    pub fn label(self) -> &'static str {
        match self {
            Self::Imperial => "Imperial",
            Self::Metric => "Metric",
        }
    }

    /// Convert a temperature stored in Fahrenheit to Celsius.
    pub fn f_to_c(temp_f: f32) -> f32 {
        (temp_f - 32.0) * 5.0 / 9.0
    }

    /// Format a temperature that is stored in Fahrenheit for display.
    ///
    /// Uses plain ASCII (no degree symbol) so the default Bevy font renders
    /// it correctly.
    pub fn format_temp(self, temp_f: f32) -> String {
        match self {
            Self::Imperial => format!("{:.0} F", temp_f),
            Self::Metric => {
                let temp_c = Self::f_to_c(temp_f);
                format!("{:.0} C", temp_c)
            }
        }
    }
}

/// Global resource holding the player's unit preference.
#[derive(Resource, Debug, Clone, Deref, DerefMut)]
pub struct UnitSystem(pub UnitPreference);

impl Default for UnitSystem {
    fn default() -> Self {
        Self(UnitPreference::Imperial)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_is_imperial() {
        assert_eq!(UnitPreference::default(), UnitPreference::Imperial);
        let unit = UnitSystem::default();
        assert_eq!(*unit, UnitPreference::Imperial);
    }

    #[test]
    fn toggle_switches_between_imperial_and_metric() {
        let mut pref = UnitPreference::Imperial;
        pref.toggle();
        assert_eq!(pref, UnitPreference::Metric);
        pref.toggle();
        assert_eq!(pref, UnitPreference::Imperial);
    }

    #[test]
    fn label_matches_variant() {
        assert_eq!(UnitPreference::Imperial.label(), "Imperial");
        assert_eq!(UnitPreference::Metric.label(), "Metric");
    }

    // --- Temperature conversion ---

    #[test]
    fn freezing_point_conversion() {
        // 32 F == 0 C
        let c = UnitPreference::f_to_c(32.0);
        assert!((c - 0.0).abs() < 0.01, "expected 0 C, got {c}");
    }

    #[test]
    fn boiling_point_conversion() {
        // 212 F == 100 C
        let c = UnitPreference::f_to_c(212.0);
        assert!((c - 100.0).abs() < 0.01, "expected 100 C, got {c}");
    }

    #[test]
    fn body_temp_conversion() {
        // 98.6 F == 37 C
        let c = UnitPreference::f_to_c(98.6);
        assert!((c - 37.0).abs() < 0.1, "expected ~37 C, got {c}");
    }

    #[test]
    fn negative_temp_conversion() {
        // -40 F == -40 C (the crossover point)
        let c = UnitPreference::f_to_c(-40.0);
        assert!((c - (-40.0)).abs() < 0.01, "expected -40 C, got {c}");
    }

    // --- format_temp output ---

    #[test]
    fn format_temp_imperial() {
        let s = UnitPreference::Imperial.format_temp(75.0);
        assert_eq!(s, "75 F");
    }

    #[test]
    fn format_temp_metric_freezing() {
        // 32 F -> 0 C
        let s = UnitPreference::Metric.format_temp(32.0);
        assert_eq!(s, "0 C");
    }

    #[test]
    fn format_temp_metric_boiling() {
        // 212 F -> 100 C
        let s = UnitPreference::Metric.format_temp(212.0);
        assert_eq!(s, "100 C");
    }

    #[test]
    fn format_temp_metric_typical_weather() {
        // 77 F -> 25 C
        let s = UnitPreference::Metric.format_temp(77.0);
        assert_eq!(s, "25 C");
    }

    #[test]
    fn format_temp_rounds_correctly() {
        // 100 F -> 37.777... C -> rounds to "38 C"
        let s = UnitPreference::Metric.format_temp(100.0);
        assert_eq!(s, "38 C");
    }
}
