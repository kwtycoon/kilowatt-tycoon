//! Generic panel helpers and shared UI components

use super::{ActivePanel, colors};
use crate::resources::ImageAssets;
use bevy::ecs::hierarchy::ChildSpawnerCommands;
use bevy::prelude::*;

// ============ Panel Content Marker ============

/// Marker component that identifies which panel this content belongs to.
/// Used for generic visibility toggling.
#[derive(Component)]
pub struct PanelContent(pub ActivePanel);

// ============ Spawn Helpers ============

/// Spawn a panel container with the given marker and content
pub fn spawn_panel_container<'a, M: Component>(
    parent: &'a mut ChildSpawnerCommands,
    panel: ActivePanel,
    marker: M,
    default_visible: bool,
) -> bevy::ecs::system::EntityCommands<'a> {
    parent.spawn((
        Node {
            flex_direction: FlexDirection::Column,
            row_gap: Val::Px(10.0),
            width: Val::Percent(100.0),
            display: if default_visible {
                Display::Flex
            } else {
                Display::None
            },
            ..default()
        },
        PanelContent(panel),
        marker,
    ))
}

/// Spawn a labeled row with a value (e.g., "Cash: $1,000,000")
pub fn spawn_labeled_row<M: Component>(
    parent: &mut ChildSpawnerCommands,
    label: &str,
    initial_value: &str,
    label_marker: M,
) {
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
                TextColor(colors::TEXT_SECONDARY),
            ));
            row.spawn((
                Text::new(initial_value),
                TextFont {
                    font_size: 12.0,
                    ..default()
                },
                TextColor(colors::TYCOON_GREEN),
                label_marker,
            ));
        });
}

/// Component linking a control to what it adjusts
#[derive(Component, Clone, Copy, PartialEq, Eq)]
pub enum StrategyControl {
    EnergyPrice,
    IdleFee,
    VideoAdPrice,
    PowerDensity,
    Maintenance,
    SolarExportPolicy,
    PricingMode,
    TouOffPeakPrice,
    TouOnPeakPrice,
    CostPlusMarkup,
    CostPlusFloor,
    CostPlusCeiling,
    SurgeBasePrice,
    SurgeMultiplier,
    SurgeThreshold,
    WarrantyTier,
    BessMode,
    PeakShaveThreshold,
}

/// Marker for minus buttons
#[derive(Component)]
pub struct MinusButton;

/// Marker for plus buttons
#[derive(Component)]
pub struct PlusButton;

/// Marker for slider fill bars - stores which control it represents
#[derive(Component, Clone, Copy)]
pub struct SliderFill(pub StrategyControl);

/// Marker for slider track bars - stores which control it represents
#[derive(Component, Clone, Copy)]
pub struct SliderTrack(pub StrategyControl);

/// Marker for slider label text (the descriptive label, not the value)
#[derive(Component, Clone, Copy)]
pub struct SliderLabelText(pub StrategyControl);

/// Marker for the outermost column node wrapping an entire slider control.
/// Toggling `Display` on this hides/shows the whole slider (label, value, buttons, bar).
#[derive(Component, Clone, Copy)]
pub struct SliderContainer(pub StrategyControl);

/// Marker for the info icon button that toggles help text.
#[derive(Component, Clone, Copy)]
pub struct InfoButton(pub StrategyControl);

/// Marker for the collapsible help text below a slider control.
#[derive(Component, Clone, Copy)]
pub struct InfoHelpText(pub StrategyControl);

/// Spawn a slider control with +/- buttons and a visual bar.
///
/// If `help_text` is `Some`, an info icon is added to the label row and a
/// hidden help text block is spawned below the slider (toggled via
/// `handle_info_button_clicks`).
pub fn spawn_slider_control<M: Component>(
    parent: &mut ChildSpawnerCommands,
    label: &str,
    initial_value: &str,
    control: StrategyControl,
    label_marker: M,
    image_assets: &ImageAssets,
    help_text: Option<&str>,
) {
    parent
        .spawn((
            Node {
                flex_direction: FlexDirection::Column,
                row_gap: Val::Px(4.0),
                width: Val::Percent(100.0),
                ..default()
            },
            SliderContainer(control),
        ))
        .with_children(|container| {
            // Label row with value (and optional info icon)
            container
                .spawn(Node {
                    flex_direction: FlexDirection::Row,
                    justify_content: JustifyContent::SpaceBetween,
                    align_items: AlignItems::Center,
                    width: Val::Percent(100.0),
                    ..default()
                })
                .with_children(|row| {
                    // Left group: label + optional info icon
                    row.spawn(Node {
                        flex_direction: FlexDirection::Row,
                        align_items: AlignItems::Center,
                        column_gap: Val::Px(4.0),
                        ..default()
                    })
                    .with_children(|left| {
                        left.spawn((
                            Text::new(label),
                            TextFont {
                                font_size: 12.0,
                                ..default()
                            },
                            TextColor(colors::TEXT_SECONDARY),
                            SliderLabelText(control),
                        ));
                        if help_text.is_some() {
                            left.spawn((
                                Button,
                                Node {
                                    width: Val::Px(14.0),
                                    height: Val::Px(14.0),
                                    justify_content: JustifyContent::Center,
                                    align_items: AlignItems::Center,
                                    ..default()
                                },
                                BackgroundColor(Color::NONE),
                                InfoButton(control),
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
                    row.spawn((
                        Text::new(initial_value),
                        TextFont {
                            font_size: 12.0,
                            ..default()
                        },
                        TextColor(colors::TYCOON_GREEN),
                        label_marker,
                    ));
                });

            // Button row: [-] [bar] [+]
            container
                .spawn(Node {
                    flex_direction: FlexDirection::Row,
                    column_gap: Val::Px(4.0),
                    width: Val::Percent(100.0),
                    height: Val::Px(24.0),
                    ..default()
                })
                .with_children(|row| {
                    // Minus button
                    row.spawn((
                        Button,
                        Node {
                            width: Val::Px(32.0),
                            height: Val::Percent(100.0),
                            justify_content: JustifyContent::Center,
                            align_items: AlignItems::Center,
                            ..default()
                        },
                        BackgroundColor(colors::BUTTON_NORMAL),
                        control,
                        MinusButton,
                    ))
                    .with_child((
                        ImageNode::new(image_assets.icon_minus.clone()),
                        Node {
                            width: Val::Px(16.0),
                            height: Val::Px(16.0),
                            ..default()
                        },
                    ));

                    // Visual bar (non-interactive indicator)
                    row.spawn((
                        Node {
                            flex_grow: 1.0,
                            height: Val::Percent(100.0),
                            ..default()
                        },
                        BackgroundColor(colors::SLIDER_TRACK),
                        SliderTrack(control),
                    ))
                    .with_child((
                        Node {
                            width: Val::Percent(50.0),
                            height: Val::Percent(100.0),
                            ..default()
                        },
                        BackgroundColor(colors::SLIDER_FILL),
                        SliderFill(control),
                    ));

                    // Plus button
                    row.spawn((
                        Button,
                        Node {
                            width: Val::Px(32.0),
                            height: Val::Percent(100.0),
                            justify_content: JustifyContent::Center,
                            align_items: AlignItems::Center,
                            ..default()
                        },
                        BackgroundColor(colors::BUTTON_NORMAL),
                        control,
                        PlusButton,
                    ))
                    .with_child((
                        ImageNode::new(image_assets.icon_plus.clone()),
                        Node {
                            width: Val::Px(16.0),
                            height: Val::Px(16.0),
                            ..default()
                        },
                    ));
                });

            // Collapsible help text (hidden by default)
            if let Some(text) = help_text {
                container.spawn((
                    Text::new(text),
                    TextFont {
                        font_size: 10.0,
                        ..default()
                    },
                    TextColor(colors::TEXT_SECONDARY),
                    Node {
                        display: Display::None,
                        ..default()
                    },
                    InfoHelpText(control),
                ));
            }
        });
}

/// Spawn a separator line
pub fn spawn_separator(parent: &mut ChildSpawnerCommands) {
    parent.spawn((
        Node {
            width: Val::Percent(100.0),
            height: Val::Px(1.0),
            margin: UiRect::vertical(Val::Px(4.0)),
            ..default()
        },
        BackgroundColor(Color::srgba(1.0, 1.0, 1.0, 0.2)),
    ));
}

// ============ Visibility System ============

/// Generic panel visibility system - replaces the 9-query approach
pub fn update_panel_visibility(
    active: Res<ActivePanel>,
    mut panels: Query<(&PanelContent, &mut Node)>,
) {
    // Only update if the active panel changed
    if !active.is_changed() {
        return;
    }

    for (content, mut node) in &mut panels {
        node.display = if content.0 == *active {
            Display::Flex
        } else {
            Display::None
        };
    }
}

/// Toggle help text visibility when an info button is clicked.
pub fn handle_info_button_clicks(
    info_buttons: Query<(&Interaction, &InfoButton), Changed<Interaction>>,
    mut help_texts: Query<(&InfoHelpText, &mut Node)>,
) {
    for (interaction, info_btn) in &info_buttons {
        if *interaction != Interaction::Pressed {
            continue;
        }
        for (help, mut node) in &mut help_texts {
            if help.0 == info_btn.0 {
                node.display = match node.display {
                    Display::None => Display::Flex,
                    _ => Display::None,
                };
            }
        }
    }
}
