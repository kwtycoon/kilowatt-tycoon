//! Operations panel - comprehensive technician and O&M statistics

use bevy::ecs::hierarchy::ChildSpawnerCommands;
use bevy::prelude::*;

use crate::components::BelongsToSite;
use crate::components::charger::{Charger, ChargerState};
use crate::resources::{
    GameClock, GameState, ImageAssets, MultiSiteManager, PHOTOVOLTAIC_CANOPY_FAULT_MULTIPLIER,
    SelectedChargerEntity, TechStatus, TechnicianState, WarrantyTier,
};
use crate::ui::sidebar::{ActivePanel, PanelContent, colors};

// ============ Panel Components ============

#[derive(Component)]
pub struct OperationsPanel;

#[derive(Component)]
pub struct OperationsContent;

#[derive(Component)]
pub struct ViewLedgerButton;

// ============ Spawn Functions ============

/// Spawn the operations panel
pub fn spawn_operations_panel(parent: &mut ChildSpawnerCommands, _image_assets: &ImageAssets) {
    parent
        .spawn((
            Node {
                width: Val::Percent(100.0),
                flex_direction: FlexDirection::Column,
                row_gap: Val::Px(8.0),
                display: Display::None, // Hidden by default
                ..default()
            },
            PanelContent(ActivePanel::Operations),
            OperationsPanel,
        ))
        .with_children(|panel| {
            // Content container that gets rebuilt each frame
            panel.spawn((
                Node {
                    width: Val::Percent(100.0),
                    flex_direction: FlexDirection::Column,
                    row_gap: Val::Px(8.0),
                    ..default()
                },
                OperationsContent,
            ));
        });
}

/// Update the operations panel with current data
pub fn update_operations_panel(
    mut commands: Commands,
    content_query: Query<Entity, With<OperationsContent>>,
    children_query: Query<&Children>,
    chargers: Query<(Entity, &Charger, &BelongsToSite)>,
    tech_state: Res<TechnicianState>,
    multi_site: Res<MultiSiteManager>,
    game_state: Res<GameState>,
    _game_clock: Res<GameClock>,
    image_assets: Res<ImageAssets>,
) {
    let Ok(content_entity) = content_query.single() else {
        return;
    };

    // Clear existing content
    if let Ok(children) = children_query.get(content_entity) {
        let to_despawn: Vec<Entity> = children.to_vec();
        for child in to_despawn {
            commands.entity(child).try_despawn();
        }
    }

    // Collect faulted chargers
    let mut faulted: Vec<(Entity, &Charger)> = chargers
        .iter()
        .filter(|(_, c, _)| c.current_fault.is_some())
        .map(|(e, c, _)| (e, c))
        .collect();

    // Sort by fault severity (offline first, then warning)
    faulted.sort_by_key(|(_, c)| match c.state() {
        ChargerState::Offline => 0,
        ChargerState::Warning => 1,
        _ => 2,
    });

    let fault_count = faulted.len();

    let (warranty_tier, warranty_monthly_premium) =
        if let Some(site_state) = multi_site.active_site() {
            let tier = site_state.service_strategy.warranty_tier;
            let premium: f32 = if tier != WarrantyTier::None {
                chargers
                    .iter()
                    .filter(|(_, _, b)| Some(b.site_id) == multi_site.viewed_site_id)
                    .map(|(_, c, _)| c.warranty_premium(tier))
                    .sum()
            } else {
                0.0
            };
            (tier, premium)
        } else {
            (WarrantyTier::None, 0.0)
        };

    // Build panel content
    commands.entity(content_entity).with_children(|parent| {
        // Section 1: Technician Status
        spawn_technician_section(parent, &tech_state, &multi_site, &image_assets);

        // Section 2: O&M Statistics
        spawn_om_stats_section(
            parent,
            &game_state,
            &multi_site,
            fault_count,
            warranty_tier,
            warranty_monthly_premium,
            &image_assets,
        );

        // Section 2.5: RF Environment
        spawn_rf_stats_section(parent, &multi_site);

        // Section 3: Active Faults (if any)
        spawn_faults_section(parent, &faulted, &tech_state, &image_assets);
    });
}

// ============ Section Builders ============

fn spawn_technician_section(
    parent: &mut ChildSpawnerCommands,
    tech_state: &TechnicianState,
    multi_site: &MultiSiteManager,
    image_assets: &ImageAssets,
) {
    let location_text = tech_state.current_location_name(multi_site);
    let status_text = tech_state.eta_string();
    let queue_count = tech_state.dispatch_queue.len();

    // Status color based on technician state
    let status_color = match tech_state.status {
        TechStatus::Idle => Color::srgb(0.3, 0.8, 0.3), // Green - available
        TechStatus::EnRoute => Color::srgb(0.5, 0.8, 1.0), // Blue - traveling
        TechStatus::WalkingOnSite => Color::srgb(0.5, 0.8, 1.0),
        TechStatus::Repairing => Color::srgb(1.0, 0.8, 0.2), // Yellow - working
        TechStatus::LeavingSite => Color::srgb(0.7, 0.7, 0.7),
    };

    parent
        .spawn((
            Node {
                width: Val::Percent(100.0),
                padding: UiRect::all(Val::Px(10.0)),
                flex_direction: FlexDirection::Column,
                row_gap: Val::Px(6.0),
                ..default()
            },
            BackgroundColor(Color::srgb(0.1, 0.15, 0.2)),
            BorderRadius::all(Val::Px(6.0)),
        ))
        .with_children(|section| {
            // Header: "TECHNICIAN" with icon
            section
                .spawn(Node {
                    flex_direction: FlexDirection::Row,
                    column_gap: Val::Px(8.0),
                    align_items: AlignItems::Center,
                    margin: UiRect::bottom(Val::Px(4.0)),
                    ..default()
                })
                .with_children(|header| {
                    header.spawn((
                        ImageNode::new(image_assets.icon_technician.clone()),
                        Node {
                            width: Val::Px(20.0),
                            height: Val::Px(20.0),
                            ..default()
                        },
                    ));
                    header.spawn((
                        Text::new("TECHNICIAN"),
                        TextFont {
                            font_size: 14.0,
                            ..default()
                        },
                        TextColor(colors::TEXT_PRIMARY),
                    ));
                });

            // Status row
            spawn_stat_row(section, "Status:", &status_text, status_color);

            // Location row
            spawn_stat_row(
                section,
                "Location:",
                &location_text,
                Color::srgb(0.8, 0.8, 0.8),
            );

            // Queue row (if any queued)
            if queue_count > 0 {
                spawn_stat_row(
                    section,
                    "Queue:",
                    &format!("{queue_count} pending"),
                    Color::srgb(1.0, 0.7, 0.3),
                );
            }

            // Progress bar if en route or repairing
            if tech_state.status == TechStatus::EnRoute {
                spawn_progress_bar(
                    section,
                    tech_state.travel_progress(),
                    Color::srgb(0.3, 0.8, 1.0),
                );
            } else if tech_state.status == TechStatus::Repairing
                && tech_state.repair_remaining > 0.0
            {
                // Calculate repair progress (we don't have total, so show time remaining)
                let repair_mins = (tech_state.repair_remaining / 60.0).ceil() as i32;
                spawn_stat_row(
                    section,
                    "Repair ETA:",
                    &format!("{repair_mins}m remaining"),
                    Color::srgb(1.0, 0.8, 0.2),
                );
            }
        });
}

fn spawn_om_stats_section(
    parent: &mut ChildSpawnerCommands,
    game_state: &GameState,
    multi_site: &MultiSiteManager,
    fault_count: usize,
    warranty_tier: WarrantyTier,
    warranty_monthly_premium: f32,
    image_assets: &ImageAssets,
) {
    // Get site-specific data
    let (uptime_pct, maintenance_cost, oem_tier, protected_charger_count) =
        if let Some(site_state) = multi_site.active_site() {
            (
                site_state.site_upgrades.estimated_uptime_percent(),
                site_state.service_strategy.hourly_maintenance_cost(),
                site_state.site_upgrades.oem_tier,
                site_state.grid.protected_charger_count(),
            )
        } else {
            (85.0, 10.0, crate::resources::OemTier::None, 0)
        };
    let canopy_fault_reduction_pct = (1.0 - PHOTOVOLTAIC_CANOPY_FAULT_MULTIPLIER) * 100.0;

    parent
        .spawn((
            Node {
                width: Val::Percent(100.0),
                padding: UiRect::all(Val::Px(10.0)),
                flex_direction: FlexDirection::Column,
                row_gap: Val::Px(6.0),
                ..default()
            },
            BackgroundColor(Color::srgb(0.12, 0.12, 0.15)),
            BorderRadius::all(Val::Px(6.0)),
        ))
        .with_children(|section| {
            // Header: "O&M STATISTICS"
            section
                .spawn(Node {
                    flex_direction: FlexDirection::Row,
                    column_gap: Val::Px(8.0),
                    align_items: AlignItems::Center,
                    margin: UiRect::bottom(Val::Px(4.0)),
                    ..default()
                })
                .with_children(|header| {
                    header.spawn((
                        ImageNode::new(image_assets.icon_briefcase.clone()),
                        Node {
                            width: Val::Px(20.0),
                            height: Val::Px(20.0),
                            ..default()
                        },
                    ));
                    header.spawn((
                        Text::new("O&M STATISTICS"),
                        TextFont {
                            font_size: 14.0,
                            ..default()
                        },
                        TextColor(colors::TEXT_PRIMARY),
                    ));
                });

            // Uptime
            let uptime_color = if uptime_pct >= 95.0 {
                Color::srgb(0.3, 0.9, 0.3)
            } else if uptime_pct >= 85.0 {
                Color::srgb(0.8, 0.8, 0.3)
            } else {
                Color::srgb(1.0, 0.4, 0.3)
            };
            spawn_stat_row(
                section,
                "Est. Uptime:",
                &format!("{uptime_pct:.0}%"),
                uptime_color,
            );

            // Maintenance spend
            spawn_stat_row(
                section,
                "Maintenance:",
                &format!("${maintenance_cost:.0}/hr"),
                colors::TEXT_SECONDARY,
            );

            // Active faults
            let fault_color = if fault_count == 0 {
                Color::srgb(0.3, 0.8, 0.3)
            } else {
                Color::srgb(1.0, 0.4, 0.3)
            };
            spawn_stat_row(
                section,
                "Active Faults:",
                &format!("{fault_count}"),
                fault_color,
            );

            let canopy_color = if protected_charger_count > 0 {
                Color::srgb(0.3, 0.8, 0.3)
            } else {
                Color::srgb(0.5, 0.5, 0.5)
            };
            spawn_stat_row(
                section,
                "Canopy Protected:",
                &format!("{protected_charger_count} chargers"),
                canopy_color,
            );
            let canopy_fault_text = if protected_charger_count > 0 {
                format!("-{canopy_fault_reduction_pct:.0}% covered")
            } else {
                "None".to_string()
            };
            spawn_stat_row(
                section,
                "Fault Reduction:",
                &canopy_fault_text,
                canopy_color,
            );

            // Warranty tier and premium
            spawn_stat_row(
                section,
                "Warranty:",
                warranty_tier.display_name(),
                colors::TEXT_SECONDARY,
            );
            if warranty_tier != WarrantyTier::None {
                spawn_stat_row(
                    section,
                    "Premium:",
                    &format!("${:.0}/mo", warranty_monthly_premium),
                    colors::TEXT_SECONDARY,
                );
            }

            // Total OPEX
            spawn_stat_row(
                section,
                "Total OPEX:",
                &format!("${:.0}", game_state.ledger.total_opex_f32()),
                Color::srgb(1.0, 0.6, 0.3),
            );

            // OEM Platform status
            if oem_tier != crate::resources::OemTier::None {
                section
                    .spawn(Node {
                        flex_direction: FlexDirection::Row,
                        column_gap: Val::Px(6.0),
                        align_items: AlignItems::Center,
                        margin: UiRect::top(Val::Px(4.0)),
                        ..default()
                    })
                    .with_children(|row| {
                        row.spawn((
                            ImageNode::new(image_assets.icon_success.clone()),
                            Node {
                                width: Val::Px(14.0),
                                height: Val::Px(14.0),
                                ..default()
                            },
                        ));
                        row.spawn((
                            Text::new(format!("{} Active", oem_tier.display_name())),
                            TextFont {
                                font_size: 11.0,
                                ..default()
                            },
                            TextColor(Color::srgb(0.3, 0.8, 0.3)),
                        ));
                    });
            }

            // View Ledger button
            section
                .spawn((
                    Button,
                    ViewLedgerButton,
                    Node {
                        width: Val::Percent(100.0),
                        padding: UiRect::axes(Val::Px(12.0), Val::Px(6.0)),
                        justify_content: JustifyContent::Center,
                        align_items: AlignItems::Center,
                        margin: UiRect::top(Val::Px(6.0)),
                        ..default()
                    },
                    BackgroundColor(Color::srgba(1.0, 1.0, 1.0, 0.08)),
                    BorderRadius::all(Val::Px(4.0)),
                ))
                .with_child((
                    Text::new("View Ledger (L)"),
                    TextFont {
                        font_size: 11.0,
                        ..default()
                    },
                    TextColor(Color::srgb(0.7, 0.8, 1.0)),
                ));
        });
}

fn spawn_faults_section(
    parent: &mut ChildSpawnerCommands,
    faulted: &[(Entity, &Charger)],
    tech_state: &TechnicianState,
    image_assets: &ImageAssets,
) {
    parent
        .spawn((
            Node {
                width: Val::Percent(100.0),
                padding: UiRect::all(Val::Px(10.0)),
                flex_direction: FlexDirection::Column,
                row_gap: Val::Px(6.0),
                ..default()
            },
            BackgroundColor(Color::srgb(0.15, 0.1, 0.1)),
            BorderRadius::all(Val::Px(6.0)),
        ))
        .with_children(|section| {
            // Header: "ACTIVE FAULTS"
            section
                .spawn(Node {
                    flex_direction: FlexDirection::Row,
                    column_gap: Val::Px(8.0),
                    align_items: AlignItems::Center,
                    margin: UiRect::bottom(Val::Px(4.0)),
                    ..default()
                })
                .with_children(|header| {
                    header.spawn((
                        ImageNode::new(image_assets.icon_fault.clone()),
                        Node {
                            width: Val::Px(20.0),
                            height: Val::Px(20.0),
                            ..default()
                        },
                    ));
                    header.spawn((
                        Text::new("ACTIVE FAULTS"),
                        TextFont {
                            font_size: 14.0,
                            ..default()
                        },
                        TextColor(colors::TEXT_PRIMARY),
                    ));
                });

            if faulted.is_empty() {
                // No faults - show success message
                section
                    .spawn(Node {
                        flex_direction: FlexDirection::Row,
                        column_gap: Val::Px(6.0),
                        align_items: AlignItems::Center,
                        ..default()
                    })
                    .with_children(|row| {
                        row.spawn((
                            ImageNode::new(image_assets.icon_success.clone()),
                            Node {
                                width: Val::Px(16.0),
                                height: Val::Px(16.0),
                                ..default()
                            },
                        ));
                        row.spawn((
                            Text::new("All chargers operational"),
                            TextFont {
                                font_size: 13.0,
                                ..default()
                            },
                            TextColor(Color::srgb(0.3, 0.8, 0.3)),
                        ));
                    });
            } else {
                // Show compact fault list
                for (charger_entity, charger) in faulted.iter() {
                    spawn_fault_row(section, *charger_entity, charger, tech_state, image_assets);
                }
            }
        });
}

// ============ Helper Functions ============

fn spawn_rf_stats_section(parent: &mut ChildSpawnerCommands, multi_site: &MultiSiteManager) {
    let rf = multi_site
        .active_site()
        .map(|s| &s.rf_environment)
        .cloned()
        .unwrap_or_default();

    let noise_dbm = -90.0 + rf.noise_floor * 40.0;
    let snr_db = rf.snr * 30.0;

    let noise_color = if noise_dbm < -75.0 {
        Color::srgb(0.3, 0.9, 0.3)
    } else if noise_dbm < -60.0 {
        Color::srgb(0.8, 0.8, 0.3)
    } else {
        Color::srgb(1.0, 0.4, 0.3)
    };

    let snr_color = if snr_db > 20.0 {
        Color::srgb(0.3, 0.9, 0.3)
    } else if snr_db > 10.0 {
        Color::srgb(0.8, 0.8, 0.3)
    } else {
        Color::srgb(1.0, 0.4, 0.3)
    };

    let (risk_label, risk_color) = if rf.comm_fault_multiplier <= 0.3 {
        ("Low", Color::srgb(0.3, 0.9, 0.3))
    } else if rf.comm_fault_multiplier <= 0.7 {
        ("Moderate", Color::srgb(0.8, 0.8, 0.3))
    } else if rf.comm_fault_multiplier <= 1.2 {
        ("High", Color::srgb(1.0, 0.6, 0.3))
    } else {
        ("Critical", Color::srgb(1.0, 0.4, 0.3))
    };

    parent
        .spawn((
            Node {
                width: Val::Percent(100.0),
                padding: UiRect::all(Val::Px(10.0)),
                flex_direction: FlexDirection::Column,
                row_gap: Val::Px(6.0),
                ..default()
            },
            BackgroundColor(Color::srgb(0.12, 0.12, 0.15)),
            BorderRadius::all(Val::Px(6.0)),
        ))
        .with_children(|section| {
            section
                .spawn(Node {
                    flex_direction: FlexDirection::Row,
                    column_gap: Val::Px(8.0),
                    align_items: AlignItems::Center,
                    margin: UiRect::bottom(Val::Px(4.0)),
                    ..default()
                })
                .with_children(|header| {
                    header.spawn((
                        Text::new("RF ENVIRONMENT"),
                        TextFont {
                            font_size: 14.0,
                            ..default()
                        },
                        TextColor(colors::TEXT_PRIMARY),
                    ));
                });

            spawn_stat_row(
                section,
                "Noise Floor:",
                &format!("{noise_dbm:.0} dBm"),
                noise_color,
            );
            spawn_stat_row(section, "SNR:", &format!("{snr_db:.0} dB"), snr_color);
            spawn_stat_row(section, "Comm Fault Risk:", risk_label, risk_color);

            if rf.booster_count > 0 {
                spawn_stat_row(
                    section,
                    "Boosters:",
                    &format!("{} active", rf.booster_count),
                    colors::TEXT_SECONDARY,
                );
            } else {
                spawn_stat_row(section, "Boosters:", "None", Color::srgb(0.5, 0.5, 0.5));
            }

            if rf.staff_detection_bonus {
                spawn_stat_row(section, "Staff:", "On-site", Color::srgb(0.3, 0.8, 0.3));
            }
        });
}

fn spawn_stat_row(parent: &mut ChildSpawnerCommands, label: &str, value: &str, value_color: Color) {
    parent
        .spawn(Node {
            flex_direction: FlexDirection::Row,
            justify_content: JustifyContent::SpaceBetween,
            width: Val::Percent(100.0),
            ..default()
        })
        .with_children(|row| {
            row.spawn((
                Text::new(label),
                TextFont {
                    font_size: 12.0,
                    ..default()
                },
                TextColor(Color::srgb(0.6, 0.6, 0.6)),
            ));
            row.spawn((
                Text::new(value),
                TextFont {
                    font_size: 12.0,
                    ..default()
                },
                TextColor(value_color),
            ));
        });
}

fn spawn_progress_bar(parent: &mut ChildSpawnerCommands, progress: f32, fill_color: Color) {
    parent
        .spawn((
            Node {
                width: Val::Percent(100.0),
                height: Val::Px(6.0),
                margin: UiRect::top(Val::Px(4.0)),
                ..default()
            },
            BackgroundColor(Color::srgb(0.2, 0.2, 0.2)),
            BorderRadius::all(Val::Px(3.0)),
        ))
        .with_children(|bar_bg| {
            bar_bg.spawn((
                Node {
                    width: Val::Percent(progress * 100.0),
                    height: Val::Percent(100.0),
                    ..default()
                },
                BackgroundColor(fill_color),
                BorderRadius::all(Val::Px(3.0)),
            ));
        });
}

fn spawn_fault_row(
    parent: &mut ChildSpawnerCommands,
    charger_entity: Entity,
    charger: &Charger,
    tech_state: &TechnicianState,
    image_assets: &ImageAssets,
) {
    let Some(fault) = charger.current_fault else {
        return;
    };

    let status_color = match charger.state() {
        ChargerState::Offline => Color::srgb(1.0, 0.3, 0.3),
        ChargerState::Warning => Color::srgb(1.0, 0.8, 0.2),
        _ => Color::srgb(0.7, 0.7, 0.7),
    };

    let is_being_serviced = Some(charger_entity) == tech_state.target_charger;

    parent
        .spawn((
            Node {
                width: Val::Percent(100.0),
                padding: UiRect::new(Val::Px(8.0), Val::Px(8.0), Val::Px(6.0), Val::Px(6.0)),
                flex_direction: FlexDirection::Row,
                justify_content: JustifyContent::SpaceBetween,
                align_items: AlignItems::Center,
                ..default()
            },
            BackgroundColor(Color::srgb(0.18, 0.12, 0.12)),
            BorderRadius::all(Val::Px(4.0)),
        ))
        .with_children(|row| {
            // Charger ID and fault name
            row.spawn((
                Text::new(format!("{}: {}", charger.id, fault.display_name())),
                TextFont {
                    font_size: 12.0,
                    ..default()
                },
                TextColor(status_color),
            ));

            // Service indicator icon (technician is working on this charger)
            if is_being_serviced {
                row.spawn((
                    ImageNode::new(image_assets.icon_technician.clone()),
                    Node {
                        width: Val::Px(14.0),
                        height: Val::Px(14.0),
                        ..default()
                    },
                ));
            }
        });
}

/// Handle clicking on fault rows to select charger (placeholder)
pub fn handle_fault_row_clicks(
    _selected: ResMut<SelectedChargerEntity>,
    _interaction_query: Query<&Interaction, (Changed<Interaction>, With<Button>)>,
) {
    // TODO: Implement charger selection from fault rows
    // This would require adding a component to fault row buttons with the charger entity
}

/// Open ledger modal when the "View Ledger" button is clicked.
pub fn handle_view_ledger_button(
    interaction: Query<&Interaction, (Changed<Interaction>, With<ViewLedgerButton>)>,
    mut ledger_modal: ResMut<crate::ui::ledger_modal::LedgerModalState>,
) {
    for i in &interaction {
        if *i == Interaction::Pressed {
            ledger_modal.toggle();
        }
    }
}
