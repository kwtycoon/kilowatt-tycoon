//! Achievement modal - displays all achievements and their unlock status

use bevy::prelude::*;

use crate::resources::achievements::{AchievementKind, AchievementState};

// ============ Components ============

/// Marker component for the achievement modal overlay
#[derive(Component)]
pub struct AchievementModalUI;

/// Close button marker for the achievement modal
#[derive(Component, Debug, Clone, Copy)]
pub struct AchievementCloseButton;

/// Marker for the scrollable achievement entries area (for dynamic rebuilds)
#[derive(Component)]
pub struct AchievementEntriesContainer;

/// Marker for individual achievement rows (despawned on rebuild)
#[derive(Component)]
pub struct AchievementEntryRow;

/// Marker for the progress text at the bottom
#[derive(Component)]
pub struct AchievementProgressText;

/// Marker for the progress bar fill
#[derive(Component)]
pub struct AchievementProgressFill;

// ============ Resource ============

/// Resource to control achievement modal visibility
#[derive(Resource, Default, Debug)]
pub struct AchievementModalState {
    pub is_open: bool,
}

impl AchievementModalState {
    pub fn open(&mut self) {
        self.is_open = true;
    }

    pub fn close(&mut self) {
        self.is_open = false;
    }

    pub fn toggle(&mut self) {
        self.is_open = !self.is_open;
    }
}

// ============ Systems ============

/// Spawn the achievement modal when it's opened
pub fn spawn_achievement_modal(
    mut commands: Commands,
    modal_state: Res<AchievementModalState>,
    achievement_state: Res<AchievementState>,
    existing_modal: Query<Entity, With<AchievementModalUI>>,
) {
    // Only spawn if open and doesn't already exist
    if !modal_state.is_open || !existing_modal.is_empty() {
        return;
    }

    // Spawn modal overlay (dim background)
    commands
        .spawn((
            AchievementModalUI,
            Node {
                width: Val::Percent(100.0),
                height: Val::Percent(100.0),
                position_type: PositionType::Absolute,
                justify_content: JustifyContent::Center,
                align_items: AlignItems::Center,
                ..default()
            },
            BackgroundColor(Color::srgba(0.0, 0.0, 0.0, 0.7)),
            GlobalZIndex(2000),
        ))
        .with_children(|overlay| {
            // Modal container (no scroll here -- scroll is on the entries area)
            overlay
                .spawn((
                    Node {
                        width: Val::Px(750.0),
                        max_height: Val::Percent(85.0),
                        flex_direction: FlexDirection::Column,
                        padding: UiRect::all(Val::Px(24.0)),
                        row_gap: Val::Px(12.0),
                        ..default()
                    },
                    BackgroundColor(Color::srgb(0.12, 0.14, 0.18)),
                    BorderRadius::all(Val::Px(12.0)),
                ))
                .with_children(|modal| {
                    // ---- Header (pinned) ----
                    modal
                        .spawn(Node {
                            width: Val::Percent(100.0),
                            justify_content: JustifyContent::SpaceBetween,
                            align_items: AlignItems::Center,
                            margin: UiRect::bottom(Val::Px(4.0)),
                            flex_shrink: 0.0,
                            ..default()
                        })
                        .with_children(|header| {
                            header.spawn((
                                Text::new("ACHIEVEMENTS"),
                                TextFont {
                                    font_size: 24.0,
                                    ..default()
                                },
                                TextColor(Color::srgb(1.0, 0.84, 0.0)),
                            ));

                            // Close button (X)
                            header
                                .spawn((
                                    Button,
                                    Node {
                                        width: Val::Px(32.0),
                                        height: Val::Px(32.0),
                                        justify_content: JustifyContent::Center,
                                        align_items: AlignItems::Center,
                                        ..default()
                                    },
                                    BackgroundColor(Color::srgba(1.0, 1.0, 1.0, 0.1)),
                                    BorderRadius::all(Val::Px(4.0)),
                                    AchievementCloseButton,
                                ))
                                .with_child((
                                    Text::new("X"),
                                    TextFont {
                                        font_size: 18.0,
                                        ..default()
                                    },
                                    TextColor(Color::srgb(0.7, 0.7, 0.7)),
                                ));
                        });

                    // ---- Divider (pinned) ----
                    modal.spawn((
                        Node {
                            width: Val::Percent(100.0),
                            height: Val::Px(2.0),
                            margin: UiRect::bottom(Val::Px(8.0)),
                            flex_shrink: 0.0,
                            ..default()
                        },
                        BackgroundColor(Color::srgb(0.3, 0.35, 0.4)),
                    ));

                    // ---- Achievement entries by tier (scrollable) ----
                    modal
                        .spawn((
                            Node {
                                width: Val::Percent(100.0),
                                flex_direction: FlexDirection::Column,
                                row_gap: Val::Px(16.0),
                                flex_grow: 1.0,
                                overflow: Overflow::scroll_y(),
                                ..default()
                            },
                            ScrollPosition::default(),
                            AchievementEntriesContainer,
                        ))
                        .with_children(|container| {
                            spawn_achievement_entries(container, &achievement_state);
                        });

                    // ---- Bottom: Progress bar (pinned) ----
                    spawn_progress_bar(modal, &achievement_state);
                });
        });
}

/// Spawn all achievement entries grouped by tier
fn spawn_achievement_entries(
    parent: &mut ChildSpawnerCommands,
    achievement_state: &AchievementState,
) {
    for (tier, achievements) in AchievementKind::by_tier() {
        // Tier section wrapper
        parent
            .spawn((
                Node {
                    width: Val::Percent(100.0),
                    flex_direction: FlexDirection::Column,
                    row_gap: Val::Px(8.0),
                    ..default()
                },
                AchievementEntryRow,
            ))
            .with_children(|tier_section| {
                // Tier label with accent bar (full width)
                tier_section
                    .spawn(Node {
                        width: Val::Percent(100.0),
                        flex_direction: FlexDirection::Column,
                        row_gap: Val::Px(4.0),
                        ..default()
                    })
                    .with_children(|header| {
                        header.spawn((
                            Text::new(tier.label()),
                            TextFont {
                                font_size: 14.0,
                                ..default()
                            },
                            TextColor(tier.color()),
                        ));

                        // Accent bar
                        header.spawn((
                            Node {
                                width: Val::Percent(100.0),
                                height: Val::Px(2.0),
                                ..default()
                            },
                            BackgroundColor(tier.color()),
                        ));
                    });

                // Two-column wrapping grid for achievement cards
                tier_section
                    .spawn(Node {
                        width: Val::Percent(100.0),
                        flex_direction: FlexDirection::Row,
                        flex_wrap: FlexWrap::Wrap,
                        column_gap: Val::Px(8.0),
                        row_gap: Val::Px(8.0),
                        ..default()
                    })
                    .with_children(|grid| {
                        for kind in &achievements {
                            let is_unlocked = achievement_state.is_unlocked(*kind);
                            spawn_achievement_card(grid, *kind, is_unlocked);
                        }
                    });
            });
    }
}

/// Spawn a single achievement card (sized for two-column grid)
fn spawn_achievement_card(
    parent: &mut ChildSpawnerCommands,
    kind: AchievementKind,
    is_unlocked: bool,
) {
    let row_opacity = if is_unlocked { 1.0 } else { 0.35 };
    let row_bg = if is_unlocked {
        Color::srgba(0.2, 0.25, 0.3, 0.6)
    } else {
        Color::srgba(0.15, 0.17, 0.22, 0.4)
    };

    // ~48% width so two cards fit per row with the 8px gap
    parent
        .spawn((
            Node {
                width: Val::Percent(48.5),
                flex_direction: FlexDirection::Row,
                align_items: AlignItems::Center,
                padding: UiRect::all(Val::Px(10.0)),
                column_gap: Val::Px(10.0),
                ..default()
            },
            BackgroundColor(row_bg),
            BorderRadius::all(Val::Px(6.0)),
        ))
        .with_children(|row| {
            // Icon placeholder (colored square based on tier)
            let icon_color = if is_unlocked {
                kind.tier().color()
            } else {
                Color::srgb(0.3, 0.3, 0.3)
            };

            row.spawn((
                Node {
                    width: Val::Px(44.0),
                    height: Val::Px(44.0),
                    justify_content: JustifyContent::Center,
                    align_items: AlignItems::Center,
                    flex_shrink: 0.0,
                    ..default()
                },
                BackgroundColor(icon_color),
                BorderRadius::all(Val::Px(6.0)),
            ))
            .with_child((
                Text::new(kind.icon_emoji()),
                TextFont {
                    font_size: 22.0,
                    ..default()
                },
                TextColor(Color::WHITE),
            ));

            // Text column (name, description, quote)
            row.spawn(Node {
                flex_direction: FlexDirection::Column,
                flex_grow: 1.0,
                row_gap: Val::Px(2.0),
                ..default()
            })
            .with_children(|text_col| {
                // Name
                text_col.spawn((
                    Text::new(kind.name()),
                    TextFont {
                        font_size: 15.0,
                        ..default()
                    },
                    TextColor(Color::srgba(0.95, 0.95, 0.95, row_opacity)),
                ));

                // Description
                text_col.spawn((
                    Text::new(kind.description()),
                    TextFont {
                        font_size: 12.0,
                        ..default()
                    },
                    TextColor(Color::srgba(0.7, 0.7, 0.7, row_opacity)),
                ));

                // Quote (italic feel via smaller size and color)
                text_col.spawn((
                    Text::new(kind.quote()),
                    TextFont {
                        font_size: 11.0,
                        ..default()
                    },
                    TextColor(Color::srgba(0.5, 0.6, 0.5, row_opacity)),
                ));
            });

            // Status indicator (check or lock)
            let (status_text, status_color) = if is_unlocked {
                ("OK", Color::srgb(0.3, 0.9, 0.3))
            } else {
                ("--", Color::srgb(0.5, 0.5, 0.5))
            };

            row.spawn((
                Node {
                    width: Val::Px(32.0),
                    justify_content: JustifyContent::Center,
                    align_items: AlignItems::Center,
                    flex_shrink: 0.0,
                    ..default()
                },
                Text::new(status_text),
                TextFont {
                    font_size: 14.0,
                    ..default()
                },
                TextColor(status_color),
            ));
        });
}

/// Spawn the progress bar at the bottom
fn spawn_progress_bar(parent: &mut ChildSpawnerCommands, achievement_state: &AchievementState) {
    let unlocked = achievement_state.unlocked_count();
    let total = achievement_state.total_count();
    let progress_pct = achievement_state.progress() * 100.0;

    // Progress text
    parent.spawn((
        Text::new(format!("{unlocked}/{total} Achievements Unlocked")),
        TextFont {
            font_size: 13.0,
            ..default()
        },
        TextColor(Color::srgb(0.7, 0.7, 0.7)),
        Node {
            margin: UiRect::top(Val::Px(4.0)),
            ..default()
        },
        AchievementProgressText,
    ));

    // Progress bar background
    parent
        .spawn((
            Node {
                width: Val::Percent(100.0),
                height: Val::Px(8.0),
                ..default()
            },
            BackgroundColor(Color::srgba(0.0, 0.0, 0.0, 0.4)),
            BorderRadius::all(Val::Px(4.0)),
        ))
        .with_children(|bar_bg| {
            // Progress bar fill
            bar_bg.spawn((
                Node {
                    width: Val::Percent(progress_pct),
                    height: Val::Percent(100.0),
                    ..default()
                },
                BackgroundColor(Color::srgb(0.2, 0.8, 0.5)),
                BorderRadius::all(Val::Px(4.0)),
                AchievementProgressFill,
            ));
        });
}

/// Despawn the achievement modal when it's closed
pub fn despawn_achievement_modal(
    mut commands: Commands,
    modal_state: Res<AchievementModalState>,
    modal_query: Query<Entity, With<AchievementModalUI>>,
) {
    if !modal_state.is_open {
        for entity in &modal_query {
            commands.entity(entity).try_despawn();
        }
    }
}

/// Handle close button clicks
pub fn handle_achievement_close_button(
    interaction_query: Query<&Interaction, (Changed<Interaction>, With<AchievementCloseButton>)>,
    mut modal_state: ResMut<AchievementModalState>,
) {
    for interaction in &interaction_query {
        if *interaction == Interaction::Pressed {
            modal_state.close();
        }
    }
}

/// Handle Escape key to close the modal
pub fn handle_achievement_escape_key(
    keyboard: Res<ButtonInput<KeyCode>>,
    mut modal_state: ResMut<AchievementModalState>,
) {
    if modal_state.is_open && keyboard.just_pressed(KeyCode::Escape) {
        modal_state.close();
    }
}

/// Rebuild the modal content when achievement state changes
pub fn update_achievement_modal_content(
    achievement_state: Res<AchievementState>,
    mut commands: Commands,
    container_query: Query<Entity, With<AchievementEntriesContainer>>,
    entry_rows: Query<Entity, With<AchievementEntryRow>>,
    mut progress_text: Query<&mut Text, With<AchievementProgressText>>,
    mut progress_fill: Query<&mut Node, With<AchievementProgressFill>>,
) {
    // Only rebuild when achievement state changes
    if !achievement_state.is_changed() {
        return;
    }

    // Despawn existing entry rows
    for entity in &entry_rows {
        commands.entity(entity).despawn();
    }

    // Rebuild entries
    for container_entity in &container_query {
        commands.entity(container_entity).with_children(|parent| {
            spawn_achievement_entries(parent, &achievement_state);
        });
    }

    // Update progress text
    let unlocked = achievement_state.unlocked_count();
    let total = achievement_state.total_count();
    for mut text in &mut progress_text {
        **text = format!("{unlocked}/{total} Achievements Unlocked");
    }

    // Update progress bar fill
    let progress_pct = achievement_state.progress() * 100.0;
    for mut node in &mut progress_fill {
        node.width = Val::Percent(progress_pct);
    }
}

#[cfg(test)]
mod tests {
    use crate::ui::achievement_modal::*;

    #[test]
    fn test_modal_state_default_is_closed() {
        let state = AchievementModalState::default();
        assert!(!state.is_open);
    }

    #[test]
    fn test_modal_state_open() {
        let mut state = AchievementModalState::default();
        state.open();
        assert!(state.is_open);
    }

    #[test]
    fn test_modal_state_close() {
        let mut state = AchievementModalState::default();
        state.open();
        state.close();
        assert!(!state.is_open);
    }

    #[test]
    fn test_modal_state_toggle() {
        let mut state = AchievementModalState::default();
        assert!(!state.is_open);
        state.toggle();
        assert!(state.is_open);
        state.toggle();
        assert!(!state.is_open);
    }
}
