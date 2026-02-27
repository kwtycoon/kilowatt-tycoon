//! Power/Utility details panel - toggleable panel showing detailed energy metrics
//!
//! Toggle with KeyP. Shows:
//! - Tariff details (TOU rates, demand charge)
//! - Metering (grid import, cumulative kWh, peak demand)
//! - Solar stats (installed, current generation)
//! - BESS stats (capacity, SOC, charge/discharge)

use bevy::prelude::*;

use crate::resources::GameClock;
use crate::ui::PowerPanelToggleButton;

// ============ Marker Components ============

#[derive(Component)]
pub struct PowerPanelRoot;

#[derive(Component)]
pub struct ClosePowerPanelButton;

// Section labels
#[derive(Component)]
pub struct TariffDetailsLabel;

#[derive(Component)]
pub struct MeteringDetailsLabel;

#[derive(Component)]
pub struct SolarDetailsLabel;

#[derive(Component)]
pub struct BessDetailsLabel;

/// Resource to track panel visibility
#[derive(Resource, Default)]
pub struct PowerPanelState {
    pub visible: bool,
}

// ============ Colors ============

mod colors {
    use bevy::prelude::Color;

    pub const PANEL_BG: Color = Color::srgba(0.08, 0.12, 0.16, 0.95);
    pub const SECTION_HEADER: Color = Color::srgb(0.5, 0.7, 0.9);
    pub const TEXT_PRIMARY: Color = Color::srgb(0.9, 0.9, 0.9);
    pub const TEXT_VALUE: Color = Color::srgb(0.4, 0.9, 0.6);
    pub const BUTTON_CLOSE: Color = Color::srgb(0.6, 0.3, 0.3);
}

// ============ Setup ============

pub fn setup_power_panel(mut commands: Commands, existing: Query<Entity, With<PowerPanelRoot>>) {
    if !existing.is_empty() {
        return;
    }

    // Insert visibility state resource
    commands.insert_resource(PowerPanelState { visible: false });

    // Right-side panel (initially hidden, offset to avoid sidebar)
    commands
        .spawn((
            Node {
                position_type: PositionType::Absolute,
                right: Val::Px(65.0),
                top: Val::Px(82.0),
                width: Val::Px(260.0),
                height: Val::Auto,
                flex_direction: FlexDirection::Column,
                padding: UiRect::all(Val::Px(12.0)),
                row_gap: Val::Px(8.0),
                ..default()
            },
            BackgroundColor(colors::PANEL_BG),
            Visibility::Hidden,
            PowerPanelRoot,
        ))
        .with_children(|panel| {
            // Header row
            panel
                .spawn(Node {
                    width: Val::Percent(100.0),
                    justify_content: JustifyContent::SpaceBetween,
                    align_items: AlignItems::Center,
                    ..default()
                })
                .with_children(|header| {
                    header.spawn((
                        Text::new("POWER & UTILITY"),
                        TextFont {
                            font_size: 16.0,
                            ..default()
                        },
                        TextColor(colors::TEXT_PRIMARY),
                    ));

                    // Close button
                    header
                        .spawn((
                            Button,
                            Node {
                                width: Val::Px(24.0),
                                height: Val::Px(24.0),
                                justify_content: JustifyContent::Center,
                                align_items: AlignItems::Center,
                                ..default()
                            },
                            BackgroundColor(colors::BUTTON_CLOSE),
                            ClosePowerPanelButton,
                        ))
                        .with_child((
                            Text::new("X"),
                            TextFont {
                                font_size: 14.0,
                                ..default()
                            },
                            TextColor(colors::TEXT_PRIMARY),
                        ));
                });

            // Separator
            panel.spawn((
                Node {
                    width: Val::Percent(100.0),
                    height: Val::Px(1.0),
                    margin: UiRect::vertical(Val::Px(4.0)),
                    ..default()
                },
                BackgroundColor(Color::srgba(1.0, 1.0, 1.0, 0.2)),
            ));

            // Tariff section
            panel.spawn((
                Text::new("TARIFF"),
                TextFont {
                    font_size: 14.0,
                    ..default()
                },
                TextColor(colors::SECTION_HEADER),
            ));
            panel.spawn((
                Text::new(
                    "Off-Peak: $0.12/kWh\nOn-Peak: $0.28/kWh\nDemand: $15/kW\nPeriod: Off-Peak",
                ),
                TextFont {
                    font_size: 12.0,
                    ..default()
                },
                TextColor(colors::TEXT_VALUE),
                TariffDetailsLabel,
            ));

            panel.spawn((
                Node {
                    width: Val::Percent(100.0),
                    height: Val::Px(1.0),
                    margin: UiRect::vertical(Val::Px(4.0)),
                    ..default()
                },
                BackgroundColor(Color::srgba(1.0, 1.0, 1.0, 0.2)),
            ));

            // Metering section
            panel.spawn((
                Text::new("METERING"),
                TextFont {
                    font_size: 14.0,
                    ..default()
                },
                TextColor(colors::SECTION_HEADER),
            ));
            panel.spawn((
                Text::new("Grid: 0 kW\nImported: 0 kWh\n15-min Avg: 0 kW\nPeak: 0 kW"),
                TextFont {
                    font_size: 12.0,
                    ..default()
                },
                TextColor(colors::TEXT_VALUE),
                MeteringDetailsLabel,
            ));

            panel.spawn((
                Node {
                    width: Val::Percent(100.0),
                    height: Val::Px(1.0),
                    margin: UiRect::vertical(Val::Px(4.0)),
                    ..default()
                },
                BackgroundColor(Color::srgba(1.0, 1.0, 1.0, 0.2)),
            ));

            // Solar section
            panel.spawn((
                Text::new("SOLAR"),
                TextFont {
                    font_size: 14.0,
                    ..default()
                },
                TextColor(colors::SECTION_HEADER),
            ));
            panel.spawn((
                Text::new("Installed: 0 kW\nGenerating: 0 kW\nTotal: 0 kWh"),
                TextFont {
                    font_size: 12.0,
                    ..default()
                },
                TextColor(colors::TEXT_VALUE),
                SolarDetailsLabel,
            ));

            panel.spawn((
                Node {
                    width: Val::Percent(100.0),
                    height: Val::Px(1.0),
                    margin: UiRect::vertical(Val::Px(4.0)),
                    ..default()
                },
                BackgroundColor(Color::srgba(1.0, 1.0, 1.0, 0.2)),
            ));

            // BESS section
            panel.spawn((
                Text::new("BATTERY"),
                TextFont {
                    font_size: 14.0,
                    ..default()
                },
                TextColor(colors::SECTION_HEADER),
            ));
            panel.spawn((
                Text::new("Capacity: 0 kWh / 0 kW\nSOC: 0 kWh (0%)\nPower: 0 kW"),
                TextFont {
                    font_size: 12.0,
                    ..default()
                },
                TextColor(colors::TEXT_VALUE),
                BessDetailsLabel,
            ));
        });
}

// ============ Update Systems ============

/// Update power panel visibility
pub fn update_power_panel_visibility(
    panel_state: Res<PowerPanelState>,
    mut panel_q: Query<&mut Visibility, With<PowerPanelRoot>>,
) {
    for mut vis in &mut panel_q {
        *vis = if panel_state.visible {
            Visibility::Inherited
        } else {
            Visibility::Hidden
        };
    }
}

/// Update power panel content
pub fn update_power_panel(
    panel_state: Res<PowerPanelState>,
    multi_site: Res<crate::resources::MultiSiteManager>,
    game_clock: Res<GameClock>,
    mut tariff_q: Query<&mut Text, With<TariffDetailsLabel>>,
    mut metering_q: Query<&mut Text, (With<MeteringDetailsLabel>, Without<TariffDetailsLabel>)>,
    mut solar_q: Query<
        &mut Text,
        (
            With<SolarDetailsLabel>,
            Without<MeteringDetailsLabel>,
            Without<TariffDetailsLabel>,
        ),
    >,
    mut bess_q: Query<
        &mut Text,
        (
            With<BessDetailsLabel>,
            Without<SolarDetailsLabel>,
            Without<MeteringDetailsLabel>,
            Without<TariffDetailsLabel>,
        ),
    >,
) {
    // Only update if visible
    if !panel_state.visible {
        return;
    }

    // Get active site or return early
    let Some(site) = multi_site.active_site() else {
        return;
    };

    let energy_config = &site.site_energy_config;
    let utility_meter = &site.utility_meter;
    let grid_import = &site.grid_import;
    let solar_state = &site.solar_state;
    let bess_state = &site.bess_state;

    let tou_period = energy_config.current_tou_period(game_clock.game_time);

    // Tariff
    for mut text in &mut tariff_q {
        **text = format!(
            "Off-Peak: ${:.2}/kWh\nOn-Peak: ${:.2}/kWh\nDemand: ${:.0}/kW\nPeriod: {}",
            energy_config.off_peak_rate,
            energy_config.on_peak_rate,
            energy_config.demand_rate_per_kw,
            tou_period.display_name()
        );
    }

    // Metering
    for mut text in &mut metering_q {
        **text = format!(
            "Grid: {:.0} kW\nImported: {:.1} kWh\n15-min Avg: {:.0} kW\nPeak: {:.0} kW",
            grid_import.current_kw,
            utility_meter.total_imported_kwh(),
            utility_meter.current_avg_kw,
            utility_meter.peak_demand_kw
        );
    }

    // Solar
    for mut text in &mut solar_q {
        **text = format!(
            "Installed: {:.0} kW\nGenerating: {:.1} kW\nTotal: {:.1} kWh",
            solar_state.installed_kw_peak,
            solar_state.current_generation_kw,
            solar_state.total_generated_kwh
        );
    }

    // BESS
    let power_direction = if bess_state.current_power_kw > 0.1 {
        "Discharging"
    } else if bess_state.current_power_kw < -0.1 {
        "Charging"
    } else {
        "Idle"
    };

    for mut text in &mut bess_q {
        **text = format!(
            "Capacity: {:.0} kWh / {:.0} kW\nSOC: {:.0} kWh ({:.0}%)\n{}: {:.1} kW",
            bess_state.capacity_kwh,
            bess_state.max_discharge_kw,
            bess_state.soc_kwh,
            bess_state.soc_percent(),
            power_direction,
            bess_state.current_power_kw.abs()
        );
    }
}

/// Handle close button
pub fn handle_close_power_panel(
    mut panel_state: ResMut<PowerPanelState>,
    close_btns: Query<&Interaction, (Changed<Interaction>, With<ClosePowerPanelButton>)>,
) {
    for interaction in &close_btns {
        if *interaction == Interaction::Pressed {
            panel_state.visible = false;
        }
    }
}

/// Toggle power panel with KeyP or button click
pub fn toggle_power_panel(
    keyboard: Res<ButtonInput<KeyCode>>,
    mut panel_state: ResMut<PowerPanelState>,
    toggle_btns: Query<&Interaction, (Changed<Interaction>, With<PowerPanelToggleButton>)>,
) {
    let mut toggled = keyboard.just_pressed(KeyCode::KeyP);

    for interaction in &toggle_btns {
        if *interaction == Interaction::Pressed {
            toggled = true;
        }
    }

    if toggled {
        panel_state.visible = !panel_state.visible;
        info!("Power panel toggled: {}", panel_state.visible);
    }
}
