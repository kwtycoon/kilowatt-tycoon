//! Power system

use bevy::prelude::*;

use crate::components::BelongsToSite;
use crate::components::charger::Charger;
use crate::components::power::Transformer;
use crate::events::TransformerWarningEvent;
use crate::resources::{GameClock, MultiSiteManager, SiteConfig};

/// Update power system state
/// Load is distributed proportionally across transformers based on their kVA rating.
/// Transformer cooling upgrade increases effective thermal headroom.
pub fn power_system(
    mut multi_site: ResMut<MultiSiteManager>,
    chargers: Query<(&Charger, &BelongsToSite)>,
    mut transformers: Query<&mut Transformer>,
    game_clock: Res<GameClock>,
    time: Res<Time>,
    _site_config: Res<SiteConfig>,
    mut warning_events: MessageWriter<TransformerWarningEvent>,
) {
    if game_clock.is_paused() {
        return;
    }

    // Process each site independently
    for (site_id, site_state) in multi_site.owned_sites.iter_mut() {
        // Reset phase loads for this site
        site_state.phase_loads.reset();

        // Sum up apparent power (kVA) from all chargers at this site
        // kVA = kW_output / (efficiency * power_factor)
        for (charger, belongs) in &chargers {
            if belongs.site_id == *site_id {
                // Calculate apparent power drawn from grid for this charger's output
                let apparent_power_kva = charger.input_kva(charger.current_power_kw);
                site_state
                    .phase_loads
                    .add_load(charger.phase, apparent_power_kva);
            }
        }

        let total_load_kva = site_state.phase_loads.total_load();

        // Update transformer temperature
        let delta_game = time.delta_secs() * game_clock.speed.multiplier();

        // Transformer cooling upgrade increases effective thermal limit
        let thermal_headroom_mult = site_state.site_upgrades.thermal_headroom_multiplier();

        // Calculate total transformer capacity for this site (for proportional distribution)
        let total_site_kva = site_state.grid.total_transformer_capacity();

        // Track hottest transformer for warning events (only fire one warning per site)
        let mut hottest_temp: f32 = 0.0;
        let mut any_warning = false;
        let mut any_critical = false;

        // Get site-specific ambient temperature based on archetype/climate
        let site_ambient_temp_c = site_state.archetype.ambient_temp_c();

        // Update each transformer at this site with its proportional load (in kVA)
        for mut transformer in &mut transformers {
            // Only process transformers belonging to this site
            if transformer.site_id != *site_id {
                continue;
            }

            // Update ambient temperature based on site climate
            transformer.ambient_temp_c = site_ambient_temp_c;

            // Proportional load distribution: load = total_load * (this_kva / total_kva)
            let load_share = if total_site_kva > 0.0 {
                total_load_kva * (transformer.rating_kva / total_site_kva)
            } else {
                0.0
            };
            transformer.current_load_kva = load_share;

            // With cooling upgrade, transformer runs cooler (effectively higher thermal limit)
            // We simulate this by applying the headroom multiplier to the thermal limit
            let effective_thermal_limit = transformer.thermal_limit_c * thermal_headroom_mult;
            let base_thermal = transformer.thermal_limit_c;

            // Temporarily boost thermal limit for temperature calculation
            transformer.thermal_limit_c = effective_thermal_limit;
            transformer.update_temperature(delta_game);
            transformer.thermal_limit_c = base_thermal; // Restore for warning checks

            // Track hottest transformer
            if transformer.current_temp_c > hottest_temp {
                hottest_temp = transformer.current_temp_c;
            }

            // Check for warnings on this transformer
            if transformer.is_critical() {
                any_critical = true;
            } else if transformer.is_warning() {
                any_warning = true;
            }
        }

        // Compute thermal throttle factor from hottest transformer temperature.
        // Below warning: no throttle. Warning zone: linear 1.0 → 0.5. Critical: 0.25.
        site_state.thermal_throttle_factor = if hottest_temp >= 90.0 {
            0.25
        } else if hottest_temp >= 75.0 {
            1.0 - 0.5 * ((hottest_temp - 75.0) / 15.0)
        } else {
            1.0
        };

        // Fire a single warning event for the site based on hottest transformer
        if any_critical {
            warning_events.write(TransformerWarningEvent {
                temperature: hottest_temp,
                is_critical: true,
            });
        } else if any_warning {
            warning_events.write(TransformerWarningEvent {
                temperature: hottest_temp,
                is_critical: false,
            });
        }

        // Calculate voltage sag based on load vs capacity (using kVA)
        let capacity = site_state.effective_capacity_kva();
        if capacity > 0.0 {
            let load_ratio = total_load_kva / capacity;
            // Simple voltage sag model: voltage drops under heavy load
            let voltage_pct = (100.0 - load_ratio * 10.0).clamp(75.0, 100.0);
            site_state.voltage_state.current_voltage_pct = voltage_pct;
        }
    }
}
