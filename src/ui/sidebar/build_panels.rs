//! Build panels: Chargers, Infrastructure, Amenities, Upgrades

use super::{ActivePanel, colors, panel::*};
use crate::resources::{
    BuildState, BuildTool, GameState, ImageAssets, MultiSiteManager, SiteUpgrades, UpgradeId,
};
use bevy::ecs::hierarchy::ChildSpawnerCommands;
use bevy::prelude::*;

// ============ Panel Markers ============

#[derive(Component)]
pub struct BuildChargersPanel;

#[derive(Component)]
pub struct BuildInfraPanel;

#[derive(Component)]
pub struct BuildAmenitiesPanel;

#[derive(Component)]
pub struct UpgradesPanel;

// ============ Build Tool Button ============

#[derive(Component)]
pub struct BuildToolButton {
    pub tool: BuildTool,
}

#[derive(Component)]
pub struct BuildToolButtonText {
    pub tool: BuildTool,
}

// Upgrade button
#[derive(Component)]
pub struct UpgradeButton {
    pub upgrade_id: UpgradeId,
}

/// Marker for the price/status text on upgrade buttons
#[derive(Component)]
pub struct UpgradeStatusText {
    pub upgrade_id: UpgradeId,
}

/// Marker for the checkmark icon on active upgrade buttons
#[derive(Component)]
pub struct UpgradeStatusIcon {
    pub upgrade_id: UpgradeId,
}

/// Marker for the upgrade name text (for locked/disabled styling)
#[derive(Component)]
pub struct UpgradeNameText {
    pub upgrade_id: UpgradeId,
}

/// Marker for the upgrade description text (for locked/disabled styling)
#[derive(Component)]
pub struct UpgradeDescText {
    pub upgrade_id: UpgradeId,
}

/// Clickable info icon on an amenity build button.
#[derive(Component)]
pub struct AmenityInfoButton {
    pub tool: BuildTool,
}

/// Collapsible help text below an amenity build button (hidden by default).
#[derive(Component)]
pub struct AmenityInfoHelpText {
    pub tool: BuildTool,
}

/// Marker for the utility max label in infrastructure panel
#[derive(Component)]
pub struct UtilityMaxLabel;

/// Marker for the "No Transformer" warning in infrastructure panel
#[derive(Component)]
pub struct NoTransformerWarning;

// ============ Spawn Functions ============

/// Spawn all build panels (including upgrades)
pub fn spawn_build_panels(parent: &mut ChildSpawnerCommands, image_assets: &ImageAssets) {
    spawn_build_chargers_panel(parent, image_assets);
    spawn_build_infra_panel(parent, image_assets);
    spawn_build_amenities_panel(parent, image_assets);
    spawn_upgrades_panel(parent, image_assets);
}

fn spawn_build_chargers_panel(parent: &mut ChildSpawnerCommands, image_assets: &ImageAssets) {
    spawn_panel_container(
        parent,
        ActivePanel::BuildChargers,
        BuildChargersPanel,
        false,
    )
    .with_children(|panel| {
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
                    ImageNode::new(image_assets.icon_plug.clone()),
                    Node {
                        width: Val::Px(20.0),
                        height: Val::Px(20.0),
                        ..default()
                    },
                ));
                header.spawn((
                    Text::new("CHARGERS"),
                    TextFont {
                        font_size: 16.0,
                        ..default()
                    },
                    TextColor(colors::TEXT_PRIMARY),
                ));
            });

        spawn_build_tool_button(panel, BuildTool::ChargerL2);
        spawn_build_tool_button(panel, BuildTool::ChargerDCFC50);
        spawn_build_tool_button(panel, BuildTool::ChargerDCFC100);
        spawn_build_tool_button(panel, BuildTool::ChargerDCFC150);
        spawn_build_tool_button(panel, BuildTool::ChargerDCFC350);

        spawn_separator(panel);

        panel.spawn((
            Text::new("OTHER"),
            TextFont {
                font_size: 12.0,
                ..default()
            },
            TextColor(colors::TEXT_SECONDARY),
        ));
        spawn_build_tool_button(panel, BuildTool::Sell);

        panel.spawn((
            Text::new("Click to select, then click on parking bays to place."),
            TextFont {
                font_size: 11.0,
                ..default()
            },
            TextColor(colors::TEXT_SECONDARY),
        ));
    });
}

fn spawn_build_infra_panel(parent: &mut ChildSpawnerCommands, image_assets: &ImageAssets) {
    spawn_panel_container(parent, ActivePanel::BuildInfra, BuildInfraPanel, false).with_children(
        |panel| {
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
                        ImageNode::new(image_assets.icon_infrastructure.clone()),
                        Node {
                            width: Val::Px(20.0),
                            height: Val::Px(20.0),
                            ..default()
                        },
                    ));
                    header.spawn((
                        Text::new("INFRASTRUCTURE"),
                        TextFont {
                            font_size: 16.0,
                            ..default()
                        },
                        TextColor(colors::TEXT_PRIMARY),
                    ));
                });

            // Utility max display
            panel.spawn((
                Text::new("Utility Max: -- kVA"),
                TextFont {
                    font_size: 12.0,
                    ..default()
                },
                TextColor(colors::TEXT_SECONDARY),
                UtilityMaxLabel,
            ));

            // No transformer warning (hidden by default, shown when no transformer placed)
            panel
                .spawn((
                    Node {
                        flex_direction: FlexDirection::Row,
                        column_gap: Val::Px(4.0),
                        align_items: AlignItems::Center,
                        margin: UiRect::bottom(Val::Px(4.0)),
                        ..default()
                    },
                    NoTransformerWarning,
                    Visibility::Hidden,
                ))
                .with_children(|warning| {
                    warning.spawn((
                        ImageNode::new(image_assets.icon_warning.clone()),
                        Node {
                            width: Val::Px(12.0),
                            height: Val::Px(12.0),
                            ..default()
                        },
                    ));
                    warning.spawn((
                        Text::new("No transformer! Required for DCFC chargers."),
                        TextFont {
                            font_size: 10.0,
                            ..default()
                        },
                        TextColor(Color::srgb(1.0, 0.6, 0.2)), // Warning orange
                    ));
                });

            spawn_build_tool_button(panel, BuildTool::Transformer100kVA);
            spawn_build_tool_button(panel, BuildTool::Transformer500kVA);
            spawn_build_tool_button(panel, BuildTool::Transformer1000kVA);
            spawn_build_tool_button(panel, BuildTool::Transformer2500kVA);
            spawn_build_tool_button(panel, BuildTool::SolarCanopy);
            spawn_build_tool_button(panel, BuildTool::BatteryStorage);
            spawn_build_tool_button(panel, BuildTool::SecuritySystem);
            spawn_build_tool_button(panel, BuildTool::RfBooster);

            panel.spawn((
                Text::new(
                    "Add power infrastructure, security, and RF boosters to support your chargers.",
                ),
                TextFont {
                    font_size: 11.0,
                    ..default()
                },
                TextColor(colors::TEXT_SECONDARY),
            ));
        },
    );
}

fn spawn_build_amenities_panel(parent: &mut ChildSpawnerCommands, image_assets: &ImageAssets) {
    spawn_panel_container(
        parent,
        ActivePanel::BuildAmenities,
        BuildAmenitiesPanel,
        false,
    )
    .with_children(|panel| {
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
                    ImageNode::new(image_assets.icon_coffee.clone()),
                    Node {
                        width: Val::Px(20.0),
                        height: Val::Px(20.0),
                        ..default()
                    },
                ));
                header.spawn((
                    Text::new("AMENITIES"),
                    TextFont {
                        font_size: 16.0,
                        ..default()
                    },
                    TextColor(colors::TEXT_PRIMARY),
                ));
            });

        spawn_amenity_tool_button(panel, BuildTool::AmenityWifiRestrooms, image_assets);
        spawn_amenity_tool_button(panel, BuildTool::AmenityLoungeSnacks, image_assets);
        spawn_amenity_tool_button(panel, BuildTool::AmenityRestaurant, image_assets);
        spawn_amenity_tool_button(panel, BuildTool::AmenityDriverRestLounge, image_assets);

        panel.spawn((
            Text::new("Amenities attract more customers and improve satisfaction. Build more for greater effect."),
            TextFont {
                font_size: 11.0,
                ..default()
            },
            TextColor(colors::TEXT_SECONDARY),
        ));
    });
}

fn spawn_build_tool_button(parent: &mut ChildSpawnerCommands, tool: BuildTool) {
    let cost_text = if tool.cost() > 0 {
        let cost = tool.cost();
        let cost_str = if cost >= 1000 {
            format!("${}k", cost / 1000)
        } else {
            format!("${cost}")
        };
        format!("{} ({})", tool.display_name(), cost_str)
    } else {
        tool.display_name().to_string()
    };

    parent
        .spawn((
            Button,
            Node {
                width: Val::Percent(100.0),
                height: Val::Px(32.0),
                justify_content: JustifyContent::Center,
                align_items: AlignItems::Center,
                ..default()
            },
            BackgroundColor(colors::BUTTON_NORMAL),
            BuildToolButton { tool },
        ))
        .with_children(|btn| {
            btn.spawn((
                Text::new(cost_text),
                TextFont {
                    font_size: 12.0,
                    ..default()
                },
                TextColor(colors::TEXT_PRIMARY),
                BuildToolButtonText { tool },
            ));
        });
}

/// Amenity build button with an inline info icon and collapsible help text.
fn spawn_amenity_tool_button(
    parent: &mut ChildSpawnerCommands,
    tool: BuildTool,
    image_assets: &ImageAssets,
) {
    let cost_text = if tool.cost() > 0 {
        let cost = tool.cost();
        let cost_str = if cost >= 1000 {
            format!("${}k", cost / 1000)
        } else {
            format!("${cost}")
        };
        format!("{} ({})", tool.display_name(), cost_str)
    } else {
        tool.display_name().to_string()
    };

    let help_text = tool.description();

    parent
        .spawn(Node {
            flex_direction: FlexDirection::Column,
            width: Val::Percent(100.0),
            ..default()
        })
        .with_children(|wrapper| {
            wrapper
                .spawn((
                    Button,
                    Node {
                        width: Val::Percent(100.0),
                        height: Val::Px(32.0),
                        flex_direction: FlexDirection::Row,
                        justify_content: JustifyContent::Center,
                        align_items: AlignItems::Center,
                        column_gap: Val::Px(4.0),
                        ..default()
                    },
                    BackgroundColor(colors::BUTTON_NORMAL),
                    BuildToolButton { tool },
                ))
                .with_children(|btn| {
                    btn.spawn((
                        Text::new(cost_text),
                        TextFont {
                            font_size: 12.0,
                            ..default()
                        },
                        TextColor(colors::TEXT_PRIMARY),
                        BuildToolButtonText { tool },
                    ));
                    if help_text.is_some() {
                        btn.spawn((
                            Button,
                            Node {
                                width: Val::Px(14.0),
                                height: Val::Px(14.0),
                                justify_content: JustifyContent::Center,
                                align_items: AlignItems::Center,
                                ..default()
                            },
                            BackgroundColor(Color::NONE),
                            AmenityInfoButton { tool },
                        ))
                        .with_child((
                            ImageNode::new(image_assets.icon_info.clone()),
                            Node {
                                width: Val::Px(12.0),
                                height: Val::Px(12.0),
                                ..default()
                            },
                        ));
                    }
                });

            if let Some(text) = help_text {
                wrapper.spawn((
                    Text::new(text),
                    TextFont {
                        font_size: 10.0,
                        ..default()
                    },
                    TextColor(colors::TEXT_SECONDARY),
                    Node {
                        display: Display::None,
                        margin: UiRect::horizontal(Val::Px(4.0)),
                        ..default()
                    },
                    AmenityInfoHelpText { tool },
                ));
            }
        });
}

fn spawn_upgrades_panel(parent: &mut ChildSpawnerCommands, image_assets: &ImageAssets) {
    spawn_panel_container(parent, ActivePanel::Upgrades, UpgradesPanel, false).with_children(
        |panel| {
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
                        ImageNode::new(image_assets.icon_upgrade.clone()),
                        Node {
                            width: Val::Px(20.0),
                            height: Val::Px(20.0),
                            ..default()
                        },
                    ));
                    header.spawn((
                        Text::new("UPGRADES"),
                        TextFont {
                            font_size: 16.0,
                            ..default()
                        },
                        TextColor(colors::TEXT_PRIMARY),
                    ));
                });

            panel.spawn((
                Text::new("Hardware:"),
                TextFont {
                    font_size: 12.0,
                    ..default()
                },
                TextColor(colors::TEXT_SECONDARY),
            ));

            for info in SiteUpgrades::upgrade_info() {
                spawn_upgrade_button(
                    panel,
                    image_assets,
                    info.id,
                    info.name,
                    info.description,
                    info.cost,
                );
            }

            panel.spawn((
                Text::new("Purchase upgrades to improve efficiency and reduce costs."),
                TextFont {
                    font_size: 11.0,
                    ..default()
                },
                TextColor(colors::TEXT_SECONDARY),
            ));
        },
    );
}

fn spawn_upgrade_button(
    parent: &mut ChildSpawnerCommands,
    image_assets: &ImageAssets,
    upgrade_id: UpgradeId,
    name: &str,
    description: &str,
    cost: f32,
) {
    let cost_text = if cost >= 1000.0 {
        format!("${}k", (cost / 1000.0) as i32)
    } else {
        format!("${}", cost as i32)
    };

    parent
        .spawn((
            Button,
            Node {
                flex_direction: FlexDirection::Column,
                padding: UiRect::all(Val::Px(8.0)),
                margin: UiRect::bottom(Val::Px(4.0)),
                width: Val::Percent(100.0),
                ..default()
            },
            BackgroundColor(colors::BUTTON_NORMAL),
            UpgradeButton { upgrade_id },
        ))
        .with_children(|btn| {
            btn.spawn(Node {
                flex_direction: FlexDirection::Row,
                justify_content: JustifyContent::SpaceBetween,
                width: Val::Percent(100.0),
                ..default()
            })
            .with_children(|row| {
                row.spawn((
                    Text::new(name.to_string()),
                    TextFont {
                        font_size: 12.0,
                        ..default()
                    },
                    TextColor(colors::TEXT_PRIMARY),
                    UpgradeNameText { upgrade_id },
                ));
                // Status area: icon + text row
                row.spawn(Node {
                    flex_direction: FlexDirection::Row,
                    align_items: AlignItems::Center,
                    column_gap: Val::Px(4.0),
                    ..default()
                })
                .with_children(|status_row| {
                    // Checkmark icon (hidden by default, shown when active)
                    status_row.spawn((
                        ImageNode::new(image_assets.icon_success.clone()),
                        Node {
                            width: Val::Px(12.0),
                            height: Val::Px(12.0),
                            ..default()
                        },
                        Visibility::Hidden,
                        UpgradeStatusIcon { upgrade_id },
                    ));
                    // Price/status text
                    status_row.spawn((
                        Text::new(cost_text),
                        TextFont {
                            font_size: 12.0,
                            ..default()
                        },
                        TextColor(colors::TYCOON_GREEN),
                        UpgradeStatusText { upgrade_id },
                    ));
                });
            });
            btn.spawn((
                Text::new(description.to_string()),
                TextFont {
                    font_size: 10.0,
                    ..default()
                },
                TextColor(colors::TEXT_SECONDARY),
                UpgradeDescText { upgrade_id },
            ));
        });
}

// ============ Update Systems ============

pub fn handle_build_tool_buttons(
    game_state: Res<GameState>,
    mut build_state: ResMut<BuildState>,
    mut interaction_query: Query<(&Interaction, &BuildToolButton), Changed<Interaction>>,
) {
    for (interaction, tool_btn) in &mut interaction_query {
        if *interaction == Interaction::Pressed {
            let cost = tool_btn.tool.cost();
            let can_afford = cost == 0 || game_state.cash >= cost as f32;

            if can_afford {
                build_state.selected_tool = tool_btn.tool;
            }
        }
    }
}

pub fn update_build_tool_button_colors(
    build_state: Res<BuildState>,
    game_state: Res<GameState>,
    mut button_query: Query<(&BuildToolButton, &mut BackgroundColor)>,
    mut text_query: Query<(&BuildToolButtonText, &mut TextColor)>,
) {
    for (tool_btn, mut bg) in &mut button_query {
        let cost = tool_btn.tool.cost();
        let can_afford = cost == 0 || game_state.cash >= cost as f32;
        let is_selected = build_state.selected_tool == tool_btn.tool;

        if !can_afford {
            *bg = BackgroundColor(colors::BUTTON_DISABLED);
        } else if is_selected {
            *bg = BackgroundColor(colors::BUTTON_SELECTED);
        } else {
            *bg = BackgroundColor(colors::BUTTON_NORMAL);
        }
    }

    for (btn_text, mut text_color) in &mut text_query {
        let cost = btn_text.tool.cost();
        let can_afford = cost == 0 || game_state.cash >= cost as f32;

        if !can_afford {
            *text_color = TextColor(colors::TEXT_DISABLED);
        } else {
            *text_color = TextColor(colors::TEXT_PRIMARY);
        }
    }
}

/// Color for purchased/active upgrades - distinct dark teal
const UPGRADE_ACTIVE_BG: Color = Color::srgb(0.1, 0.3, 0.35);
/// Border/accent color for active upgrades
const UPGRADE_ACTIVE_TEXT: Color = Color::srgb(0.4, 0.9, 0.95);
/// Amber color for "LOCKED" status text
const UPGRADE_LOCKED_TEXT: Color = Color::srgb(0.6, 0.5, 0.3);

pub fn handle_upgrade_purchases(
    mut game_state: ResMut<GameState>,
    mut multi_site: ResMut<MultiSiteManager>,
    upgrade_buttons: Query<(&Interaction, &UpgradeButton), Changed<Interaction>>,
    mut oem_events: MessageWriter<crate::events::OemUpgradeEvent>,
) {
    let Some(site_id) = multi_site.viewed_site_id else {
        return;
    };
    let Some(site_state) = multi_site.active_site_mut() else {
        return;
    };

    for (interaction, upgrade_btn) in &upgrade_buttons {
        if *interaction != Interaction::Pressed {
            continue;
        }
        if site_state
            .site_upgrades
            .is_purchased(upgrade_btn.upgrade_id)
        {
            continue;
        }
        if !site_state
            .site_upgrades
            .can_purchase_oem(upgrade_btn.upgrade_id)
        {
            continue;
        }
        let cost = SiteUpgrades::get_cost(upgrade_btn.upgrade_id);
        if game_state.cash < cost {
            continue;
        }
        game_state.spend_upgrade(cost);
        site_state.site_upgrades.purchase(upgrade_btn.upgrade_id);

        if matches!(
            upgrade_btn.upgrade_id,
            UpgradeId::OemDetect | UpgradeId::OemOptimize
        ) {
            oem_events.write(crate::events::OemUpgradeEvent {
                site_id,
                new_tier: site_state.site_upgrades.oem_tier,
            });
        }

        info!(
            "Purchased upgrade: {:?} for ${:.0}",
            upgrade_btn.upgrade_id, cost
        );
    }
}

/// Continuously sync upgrade button visuals with purchase/prerequisite state.
/// Three visual states: Purchased, Locked (prerequisite missing), Available.
pub fn update_upgrade_button_states(
    multi_site: Res<MultiSiteManager>,
    mut button_colors: Query<(&UpgradeButton, &mut BackgroundColor)>,
    mut status_texts: Query<(&UpgradeStatusText, &mut Text, &mut TextColor)>,
    mut status_icons: Query<(&UpgradeStatusIcon, &mut Visibility)>,
    mut name_texts: Query<(&UpgradeNameText, &mut TextColor), Without<UpgradeStatusText>>,
    mut desc_texts: Query<
        (&UpgradeDescText, &mut TextColor),
        (Without<UpgradeStatusText>, Without<UpgradeNameText>),
    >,
) {
    let Some(site_state) = multi_site.active_site() else {
        return;
    };
    let upgrades = &site_state.site_upgrades;

    let boost_active = upgrades.is_demand_boost_active();

    for (upgrade_btn, mut bg_color) in &mut button_colors {
        let id = upgrade_btn.upgrade_id;
        let is_active = upgrades.is_purchased(id) || (id == UpgradeId::DemandBoost && boost_active);
        if is_active {
            *bg_color = BackgroundColor(UPGRADE_ACTIVE_BG);
        } else if !upgrades.can_purchase_oem(id) {
            *bg_color = BackgroundColor(colors::BUTTON_DISABLED);
        } else {
            *bg_color = BackgroundColor(colors::BUTTON_NORMAL);
        }
    }

    for (status_icon, mut visibility) in &mut status_icons {
        let id = status_icon.upgrade_id;
        let is_active = upgrades.is_purchased(id) || (id == UpgradeId::DemandBoost && boost_active);
        *visibility = if is_active {
            Visibility::Inherited
        } else {
            Visibility::Hidden
        };
    }

    for (status_text, mut text, mut text_color) in &mut status_texts {
        let id = status_text.upgrade_id;
        if id == UpgradeId::DemandBoost && boost_active {
            **text = upgrades.demand_boost_time_remaining_display();
            *text_color = TextColor(UPGRADE_ACTIVE_TEXT);
        } else if upgrades.is_purchased(id) {
            **text = "ACTIVE".to_string();
            *text_color = TextColor(UPGRADE_ACTIVE_TEXT);
        } else if !upgrades.can_purchase_oem(id) {
            **text = "LOCKED".to_string();
            *text_color = TextColor(UPGRADE_LOCKED_TEXT);
        } else {
            let cost = SiteUpgrades::get_cost(id);
            **text = if cost >= 1000.0 {
                format!("${}k", (cost / 1000.0) as i32)
            } else {
                format!("${}", cost as i32)
            };
            *text_color = TextColor(colors::TYCOON_GREEN);
        }
    }

    for (name_text, mut text_color) in &mut name_texts {
        let id = name_text.upgrade_id;
        let is_active = upgrades.is_purchased(id) || (id == UpgradeId::DemandBoost && boost_active);
        if is_active {
            *text_color = TextColor(UPGRADE_ACTIVE_TEXT);
        } else if !upgrades.can_purchase_oem(id) {
            *text_color = TextColor(colors::TEXT_DISABLED);
        } else {
            *text_color = TextColor(colors::TEXT_PRIMARY);
        }
    }

    for (desc_text, mut text_color) in &mut desc_texts {
        let id = desc_text.upgrade_id;
        let is_active = upgrades.is_purchased(id) || (id == UpgradeId::DemandBoost && boost_active);
        if is_active {
            *text_color = TextColor(colors::TEXT_SECONDARY);
        } else if !upgrades.can_purchase_oem(id) {
            *text_color = TextColor(colors::TEXT_DISABLED);
        } else {
            *text_color = TextColor(colors::TEXT_SECONDARY);
        }
    }
}

/// Update the utility max label and transformer warning in the infrastructure panel
pub fn update_utility_max_label(
    multi_site: Res<MultiSiteManager>,
    mut label_query: Query<&mut Text, With<UtilityMaxLabel>>,
    mut warning_query: Query<&mut Visibility, With<NoTransformerWarning>>,
) {
    let Some(site_state) = multi_site.active_site() else {
        return;
    };

    for mut text in &mut label_query {
        **text = format!("Utility Max: {} kVA", site_state.grid_capacity_kva as i32);
    }

    // Show warning only when there are DCFC chargers but no transformer
    // L2 chargers use split-phase 240V directly, no transformer needed
    let has_transformer = site_state.grid.has_transformer();
    let has_dcfc = site_state
        .grid
        .get_charger_bays()
        .iter()
        .any(|(_, _, ct)| ct.is_dcfc());
    for mut visibility in &mut warning_query {
        *visibility = if !has_transformer && has_dcfc {
            Visibility::Visible
        } else {
            Visibility::Hidden
        };
    }
}

/// Toggle amenity help text visibility when an info button is clicked.
pub fn handle_amenity_info_clicks(
    info_buttons: Query<(&Interaction, &AmenityInfoButton), Changed<Interaction>>,
    mut help_texts: Query<(&AmenityInfoHelpText, &mut Node)>,
) {
    for (interaction, info_btn) in &info_buttons {
        if *interaction != Interaction::Pressed {
            continue;
        }
        for (help, mut node) in &mut help_texts {
            if help.tool == info_btn.tool {
                node.display = match node.display {
                    Display::None => Display::Flex,
                    _ => Display::None,
                };
            }
        }
    }
}
