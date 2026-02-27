//! Inline power stats panel (Stats → Power)

use super::{ActivePanel, colors, panel::*};
use crate::components::charger::Charger;
use crate::resources::{ImageAssets, SiteConfig};
use bevy::ecs::hierarchy::ChildSpawnerCommands;
use bevy::prelude::*;

// ============ Panel Marker ============

#[derive(Component)]
pub struct PowerPanel;

// ============ Label Components ============

#[derive(Component)]
pub struct PowerGridDrawLabel;

#[derive(Component)]
pub struct PowerPeakDemandLabel;

#[derive(Component)]
pub struct PowerTransformerLabel;

#[derive(Component)]
pub struct PowerTariffLabel;

#[derive(Component)]
pub struct PowerSiteCapacityLabel;

#[derive(Component)]
pub struct PowerChargerCapacityLabel;

#[derive(Component)]
pub struct PowerThresholdBar;

#[derive(Component)]
pub struct PowerThresholdBarFill;

#[derive(Component)]
pub struct PowerSolarLabel;

#[derive(Component)]
pub struct PowerBatteryLabel;

#[derive(Component)]
pub struct SolarBar;

#[derive(Component)]
pub struct SolarBarFill;

#[derive(Component)]
pub struct BatteryBar;

#[derive(Component)]
pub struct BatteryBarFill;

#[derive(Component)]
pub struct PowerOffPeakRateLabel;

#[derive(Component)]
pub struct PowerOnPeakRateLabel;

#[derive(Component)]
pub struct PowerDemandChargeLabel;

// ============ Spawn Functions ============

/// Spawn the power stats panel
pub fn spawn_power_panel(parent: &mut ChildSpawnerCommands, image_assets: &ImageAssets) {
    spawn_panel_container(parent, ActivePanel::Power, PowerPanel, false).with_children(|panel| {
        // Header with icon
        panel
            .spawn(Node {
                flex_direction: FlexDirection::Row,
                column_gap: Val::Px(6.0),
                align_items: AlignItems::Center,
                ..default()
            })
            .with_children(|header| {
                header.spawn((
                    ImageNode::new(image_assets.icon_power.clone()),
                    Node {
                        width: Val::Px(20.0),
                        height: Val::Px(20.0),
                        ..default()
                    },
                ));
                header.spawn((
                    Text::new("POWER STATS"),
                    TextFont {
                        font_size: 16.0,
                        ..default()
                    },
                    TextColor(colors::TEXT_PRIMARY),
                ));
            });

        spawn_labeled_row(panel, "Grid Draw:", "0 kVA", PowerGridDrawLabel);
        spawn_labeled_row(panel, "Peak Demand:", "0 kW → $0", PowerPeakDemandLabel);

        // Progress bar for current load vs peak threshold
        panel
            .spawn(Node {
                width: Val::Percent(100.0),
                height: Val::Px(20.0),
                flex_direction: FlexDirection::Column,
                row_gap: Val::Px(2.0),
                ..default()
            })
            .with_children(|bar_container| {
                // Bar background
                bar_container
                    .spawn((
                        Node {
                            width: Val::Percent(100.0),
                            height: Val::Px(8.0),
                            ..default()
                        },
                        BackgroundColor(Color::srgba(1.0, 1.0, 1.0, 0.1)),
                        BorderRadius::all(Val::Px(4.0)),
                        PowerThresholdBar,
                    ))
                    .with_child((
                        Node {
                            width: Val::Percent(0.0), // Will be updated dynamically
                            height: Val::Percent(100.0),
                            ..default()
                        },
                        BackgroundColor(Color::srgb(0.3, 0.9, 0.3)), // Green by default
                        BorderRadius::all(Val::Px(4.0)),
                        PowerThresholdBarFill,
                    ));

                // Label below bar
                bar_container.spawn((
                    Text::new("0/0 kW - Safe"),
                    TextFont {
                        font_size: 10.0,
                        ..default()
                    },
                    TextColor(colors::TEXT_SECONDARY),
                ));
            });

        spawn_labeled_row(panel, "Transformer:", "25°C", PowerTransformerLabel);

        spawn_separator(panel);

        // Capacity section
        panel.spawn((
            Text::new("Capacity:"),
            TextFont {
                font_size: 12.0,
                ..default()
            },
            TextColor(colors::TEXT_SECONDARY),
        ));

        spawn_labeled_row(panel, "Site Limit:", "0 kVA", PowerSiteCapacityLabel);
        spawn_labeled_row(panel, "Chargers Max:", "0 kW", PowerChargerCapacityLabel);

        spawn_separator(panel);

        // Resources section
        panel.spawn((
            Text::new("Resources:"),
            TextFont {
                font_size: 12.0,
                ..default()
            },
            TextColor(colors::TEXT_SECONDARY),
        ));

        // Solar bar with generation indicator
        panel.spawn((
            Text::new("Solar:"),
            TextFont {
                font_size: 11.0,
                ..default()
            },
            TextColor(colors::TEXT_SECONDARY),
        ));
        panel
            .spawn(Node {
                width: Val::Percent(100.0),
                height: Val::Px(20.0),
                flex_direction: FlexDirection::Column,
                row_gap: Val::Px(2.0),
                ..default()
            })
            .with_children(|bar_container| {
                // Bar background
                bar_container
                    .spawn((
                        Node {
                            width: Val::Percent(100.0),
                            height: Val::Px(8.0),
                            ..default()
                        },
                        BackgroundColor(Color::srgba(1.0, 1.0, 1.0, 0.1)),
                        BorderRadius::all(Val::Px(4.0)),
                        SolarBar,
                    ))
                    .with_child((
                        Node {
                            width: Val::Percent(0.0), // Will be updated dynamically
                            height: Val::Percent(100.0),
                            ..default()
                        },
                        BackgroundColor(Color::srgb(1.0, 0.8, 0.2)), // Yellow/orange for solar
                        BorderRadius::all(Val::Px(4.0)),
                        SolarBarFill,
                    ));

                // Label below bar
                bar_container.spawn((
                    Text::new("0/0 kW"),
                    TextFont {
                        font_size: 10.0,
                        ..default()
                    },
                    TextColor(colors::TEXT_SECONDARY),
                    PowerSolarLabel,
                ));
            });

        // Battery bar with SOC and rate indicator
        panel.spawn((
            Text::new("Battery:"),
            TextFont {
                font_size: 11.0,
                ..default()
            },
            TextColor(colors::TEXT_SECONDARY),
        ));
        panel
            .spawn(Node {
                width: Val::Percent(100.0),
                height: Val::Px(20.0),
                flex_direction: FlexDirection::Column,
                row_gap: Val::Px(2.0),
                ..default()
            })
            .with_children(|bar_container| {
                // Bar background
                bar_container
                    .spawn((
                        Node {
                            width: Val::Percent(100.0),
                            height: Val::Px(8.0),
                            ..default()
                        },
                        BackgroundColor(Color::srgba(1.0, 1.0, 1.0, 0.1)),
                        BorderRadius::all(Val::Px(4.0)),
                        BatteryBar,
                    ))
                    .with_child((
                        Node {
                            width: Val::Percent(50.0), // Start at 50% SOC
                            height: Val::Percent(100.0),
                            ..default()
                        },
                        BackgroundColor(Color::srgb(0.3, 0.7, 0.9)), // Blue for battery
                        BorderRadius::all(Val::Px(4.0)),
                        BatteryBarFill,
                    ));

                // Label below bar
                bar_container.spawn((
                    Text::new("0/0 kWh (0%) | idle"),
                    TextFont {
                        font_size: 10.0,
                        ..default()
                    },
                    TextColor(colors::TEXT_SECONDARY),
                    PowerBatteryLabel,
                ));
            });

        spawn_separator(panel);

        // Utility rates section
        panel.spawn((
            Text::new("Utility Rates:"),
            TextFont {
                font_size: 12.0,
                ..default()
            },
            TextColor(colors::TEXT_SECONDARY),
        ));

        spawn_labeled_row(panel, "Off-Peak:", "$0.12/kWh", PowerOffPeakRateLabel);
        spawn_labeled_row(panel, "On-Peak:", "$0.28/kWh", PowerOnPeakRateLabel);
        spawn_labeled_row(panel, "Demand:", "$15/kW", PowerDemandChargeLabel);

        spawn_separator(panel);

        panel.spawn((
            Text::new("Monitor your site's power consumption and grid connection."),
            TextFont {
                font_size: 11.0,
                ..default()
            },
            TextColor(colors::TEXT_SECONDARY),
        ));
    });
}

// ============ Update Systems ============

/// Update the threshold progress bar (fill width and color)
#[allow(clippy::type_complexity)]
pub fn update_power_threshold_bar(
    multi_site: Res<crate::resources::MultiSiteManager>,
    mut fill_query: Query<(&mut Node, &mut BackgroundColor), With<PowerThresholdBarFill>>,
    mut label_query: Query<
        &mut Text,
        (
            Without<PowerGridDrawLabel>,
            Without<PowerPeakDemandLabel>,
            Without<PowerTransformerLabel>,
            Without<PowerSiteCapacityLabel>,
            Without<PowerChargerCapacityLabel>,
            Without<PowerSolarLabel>,
            Without<PowerBatteryLabel>,
            Without<PowerOffPeakRateLabel>,
            Without<PowerOnPeakRateLabel>,
            Without<PowerDemandChargeLabel>,
        ),
    >,
    mut last_percentage: Local<f32>,
    mut last_status: Local<u8>,
) {
    // Get active site data
    let Some(site_state) = multi_site.active_site() else {
        return;
    };

    let current_load = site_state.grid_import.current_kw;
    let peak_kw = site_state.utility_meter.peak_demand_kw;

    // Calculate percentage and color zone
    let percentage = if peak_kw > 0.0 {
        (current_load / peak_kw).min(1.5) // Cap at 150% for display
    } else {
        0.0
    };

    let (color, status_text, status_id) = if percentage < 0.70 {
        (Color::srgb(0.3, 0.9, 0.3), "Safe", 0)
    } else if percentage < 0.85 {
        (Color::srgb(1.0, 0.9, 0.2), "Caution", 1)
    } else if percentage < 1.0 {
        (Color::srgb(1.0, 0.6, 0.2), "Risk", 2)
    } else {
        (Color::srgb(1.0, 0.2, 0.2), "NEW PEAK!", 3)
    };

    // Only update if changed significantly (prevents flicker at boundaries)
    let percentage_changed = (percentage - *last_percentage).abs() > 0.02; // 2% hysteresis
    let status_changed = status_id != *last_status;

    if percentage_changed || status_changed {
        // Update fill bar
        for (mut node, mut bg_color) in &mut fill_query {
            node.width = Val::Percent((percentage * 100.0).min(100.0));
            *bg_color = BackgroundColor(color);
        }

        // Update label
        for mut text in &mut label_query {
            if text.0.contains(" kW -") || text.0.contains("kW - ") {
                **text = format!("{current_load:.0}/{peak_kw:.0} kW - {status_text}");
            }
        }

        *last_percentage = percentage;
        *last_status = status_id;
    }
}

/// Update basic power stats (grid draw, peak, transformer)
#[allow(clippy::type_complexity)]
pub fn update_power_panel_basic(
    multi_site: Res<crate::resources::MultiSiteManager>,
    _site_config: Res<SiteConfig>,
    transformers: Query<&crate::components::power::Transformer>,
    mut grid_draw: Query<&mut Text, With<PowerGridDrawLabel>>,
    mut peak_demand: Query<&mut Text, (With<PowerPeakDemandLabel>, Without<PowerGridDrawLabel>)>,
    mut transformer_label: Query<
        (&mut Text, &mut TextColor),
        (
            With<PowerTransformerLabel>,
            Without<PowerGridDrawLabel>,
            Without<PowerPeakDemandLabel>,
        ),
    >,
) {
    // Get active site data
    let Some(site_state) = multi_site.active_site() else {
        return;
    };

    // Grid draw (apparent power - kVA for infrastructure monitoring)
    for mut text in &mut grid_draw {
        **text = format!("{:.0} kVA", site_state.grid_import.current_kva);
    }

    // Peak demand with cost
    for mut text in &mut peak_demand {
        let peak_kw = site_state.utility_meter.peak_demand_kw;
        let demand_rate = site_state.site_energy_config.demand_rate_per_kw;
        let cost = peak_kw * demand_rate;
        **text = format!("{peak_kw:.0} kW → ${cost:.0}");
    }

    // Transformer temp - find hottest transformer at this site
    let transformer_count = site_state.grid.transformer_count();
    let hottest_temp = transformers
        .iter()
        .filter(|t| t.site_id == site_state.id)
        .map(|t| t.current_temp_c)
        .fold(25.0_f32, |max, temp| max.max(temp));

    // Fallback: if no transformer entities exist yet, estimate temperature
    let temp = if transformers.iter().any(|t| t.site_id == site_state.id) {
        hottest_temp
    } else {
        // Estimate based on load (fallback for when station not yet open)
        let total_load = site_state.phase_loads.total_load();
        let capacity = site_state.effective_capacity_kva();
        let load_pct = if capacity > 0.0 {
            total_load / capacity
        } else {
            0.0
        };
        let heat_multiplier = site_state.service_strategy.target_power_density.powf(1.3);
        25.0 + (85.0 * load_pct * load_pct * heat_multiplier)
    };

    for (mut text, mut color) in &mut transformer_label {
        // Show count if multiple transformers
        if transformer_count > 1 {
            **text = format!("{transformer_count}x @ {temp:.0}°C");
        } else {
            **text = format!("{temp:.0}°C");
        }

        if temp >= 90.0 {
            *color = TextColor(Color::srgb(1.0, 0.2, 0.2));
        } else if temp >= 75.0 {
            *color = TextColor(Color::srgb(1.0, 0.6, 0.2));
        } else {
            *color = TextColor(colors::TEXT_PRIMARY);
        }
    }
}

/// Update capacity information (site limit, charger capacity)
pub fn update_power_panel_capacity(
    multi_site: Res<crate::resources::MultiSiteManager>,
    chargers: Query<&Charger>,
    mut site_capacity: Query<&mut Text, With<PowerSiteCapacityLabel>>,
    mut charger_capacity: Query<
        &mut Text,
        (
            With<PowerChargerCapacityLabel>,
            Without<PowerSiteCapacityLabel>,
        ),
    >,
) {
    // Site capacity (effective limit from placed transformer and contracted capacity - in kVA)
    let site_limit_kva = multi_site
        .active_site()
        .map(|s| s.effective_capacity_kva())
        .unwrap_or(0.0);
    for mut text in &mut site_capacity {
        if site_limit_kva == 0.0 {
            **text = "No Transformer".to_string();
        } else {
            **text = format!("{site_limit_kva:.0} kVA");
        }
    }

    // Charger capacity (sum of all charger max output power - stays in kW)
    let total_charger_capacity: f32 = chargers.iter().map(|c| c.max_power_kw).sum();
    for mut text in &mut charger_capacity {
        if site_limit_kva > 0.0 && total_charger_capacity > site_limit_kva {
            **text = format!("{total_charger_capacity:.0} kW (limited)");
        } else {
            **text = format!("{total_charger_capacity:.0} kW");
        }
    }
}

/// Update resources information (solar, battery, utility rates)
#[allow(clippy::type_complexity)]
pub fn update_power_panel_resources(
    multi_site: Res<crate::resources::MultiSiteManager>,
    mut solar: Query<&mut Text, With<PowerSolarLabel>>,
    mut battery: Query<&mut Text, (With<PowerBatteryLabel>, Without<PowerSolarLabel>)>,
    mut off_peak_rate: Query<
        &mut Text,
        (
            With<PowerOffPeakRateLabel>,
            Without<PowerSolarLabel>,
            Without<PowerBatteryLabel>,
        ),
    >,
    mut on_peak_rate: Query<
        &mut Text,
        (
            With<PowerOnPeakRateLabel>,
            Without<PowerSolarLabel>,
            Without<PowerBatteryLabel>,
            Without<PowerOffPeakRateLabel>,
        ),
    >,
    mut demand_charge: Query<
        &mut Text,
        (
            With<PowerDemandChargeLabel>,
            Without<PowerSolarLabel>,
            Without<PowerBatteryLabel>,
            Without<PowerOffPeakRateLabel>,
            Without<PowerOnPeakRateLabel>,
        ),
    >,
) {
    // Get active site data
    let Some(site_state) = multi_site.active_site() else {
        return;
    };

    // Solar label - use grid totals for installed check (available during build phase)
    for mut text in &mut solar {
        if site_state.grid.total_solar_kw > 0.0 {
            let current = site_state.solar_state.current_generation_kw;
            let peak = site_state.grid.total_solar_kw;
            **text = format!("{current:.0}/{peak:.0} kW");
        } else {
            **text = "Not installed".to_string();
        }
    }

    // Battery label with SOC and rate - use grid totals for installed check (available during build phase)
    for mut text in &mut battery {
        if site_state.grid.total_battery_kwh > 0.0 {
            let soc_kwh = site_state.bess_state.soc_kwh;
            let capacity = site_state.bess_state.capacity_kwh;
            let soc_pct = site_state.bess_state.soc_percent();
            let power = site_state.bess_state.current_power_kw;

            // Determine rate indicator
            let rate_str = if power > 0.1 {
                format!("-{power:.0} kW") // Discharging
            } else if power < -0.1 {
                let abs_power = power.abs();
                format!("+{abs_power:.0} kW") // Charging
            } else {
                "idle".to_string()
            };

            **text = format!("{soc_kwh:.0}/{capacity:.0} kWh ({soc_pct:.0}%) | {rate_str}");
        } else {
            **text = "Not installed".to_string();
        }
    }

    // Utility rates
    for mut text in &mut off_peak_rate {
        **text = format!("${:.2}/kWh", site_state.site_energy_config.off_peak_rate);
    }

    for mut text in &mut on_peak_rate {
        **text = format!("${:.2}/kWh", site_state.site_energy_config.on_peak_rate);
    }

    for mut text in &mut demand_charge {
        **text = format!(
            "${:.2}/kW",
            site_state.site_energy_config.demand_rate_per_kw
        );
    }
}

/// Update the solar bar fill width and color
pub fn update_solar_bar(
    multi_site: Res<crate::resources::MultiSiteManager>,
    mut fill_query: Query<(&mut Node, &mut BackgroundColor), With<SolarBarFill>>,
) {
    let Some(site_state) = multi_site.active_site() else {
        return;
    };

    // Check if solar is installed
    let peak_kw = site_state.grid.total_solar_kw;
    if peak_kw <= 0.0 {
        // No solar - hide bar
        for (mut node, _) in &mut fill_query {
            node.width = Val::Percent(0.0);
        }
        return;
    }

    let current_kw = site_state.solar_state.current_generation_kw;
    let percentage = (current_kw / peak_kw * 100.0).clamp(0.0, 100.0);

    // Color based on generation level - brighter yellow when generating more
    let color = if percentage > 50.0 {
        Color::srgb(1.0, 0.85, 0.1) // Bright yellow
    } else if percentage > 10.0 {
        Color::srgb(1.0, 0.7, 0.2) // Orange-yellow
    } else {
        Color::srgb(0.6, 0.5, 0.3) // Dim (night/low light)
    };

    for (mut node, mut bg_color) in &mut fill_query {
        node.width = Val::Percent(percentage);
        *bg_color = BackgroundColor(color);
    }
}

/// Update the battery bar fill width and color
pub fn update_battery_bar(
    multi_site: Res<crate::resources::MultiSiteManager>,
    mut fill_query: Query<(&mut Node, &mut BackgroundColor), With<BatteryBarFill>>,
) {
    let Some(site_state) = multi_site.active_site() else {
        return;
    };

    // Check if battery is installed
    let capacity_kwh = site_state.bess_state.capacity_kwh;
    if capacity_kwh <= 0.0 {
        // No battery - hide bar
        for (mut node, _) in &mut fill_query {
            node.width = Val::Percent(0.0);
        }
        return;
    }

    let soc_percent = site_state.bess_state.soc_percent();
    let power = site_state.bess_state.current_power_kw;

    // Color based on state:
    // - Blue when charging
    // - Green when discharging/standby
    // - Red when low (< 20%)
    let color = if soc_percent < 20.0 {
        Color::srgb(0.9, 0.3, 0.3) // Red - low battery
    } else if power < -0.1 {
        Color::srgb(0.3, 0.6, 0.9) // Blue - charging
    } else if power > 0.1 {
        Color::srgb(0.3, 0.9, 0.5) // Green - discharging
    } else {
        Color::srgb(0.4, 0.7, 0.8) // Cyan - standby
    };

    for (mut node, mut bg_color) in &mut fill_query {
        node.width = Val::Percent(soc_percent);
        *bg_color = BackgroundColor(color);
    }
}

// ============ Update Systems ============
