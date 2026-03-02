//! Leaderboard modal - displays global leaderboard rankings in a popup

use bevy::prelude::*;

use crate::resources::{GameState, LeaderboardData};

// ============ Components ============

/// Marker component for leaderboard modal UI elements
#[derive(Component)]
pub struct LeaderboardModalUI;

/// Close button marker for leaderboard modal
#[derive(Component, Debug, Clone, Copy)]
pub struct LeaderboardCloseButton;

/// Marker for leaderboard entry rows (to be despawned on rebuild)
#[derive(Component)]
pub struct LeaderboardEntryRow;

/// Scrollable container for leaderboard entries
#[derive(Component)]
pub struct LeaderboardEntriesContainer;

/// Loading text component
#[derive(Component)]
pub struct LeaderboardLoadingText;

/// Error text component
#[derive(Component)]
pub struct LeaderboardErrorText;

/// Marker for the player's own score value text
#[derive(Component)]
pub struct YourScoreText;

// ============ Resource ============

/// Resource to control leaderboard modal visibility
#[derive(Resource, Default, Debug)]
pub struct LeaderboardModalState {
    pub is_open: bool,
}

impl LeaderboardModalState {
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

/// Spawn the leaderboard modal when it's opened
pub fn spawn_leaderboard_modal(
    mut commands: Commands,
    modal_state: Res<LeaderboardModalState>,
    existing_modal: Query<Entity, With<LeaderboardModalUI>>,
    game_state: Res<GameState>,
) {
    // Only spawn if open and doesn't already exist
    if !modal_state.is_open || !existing_modal.is_empty() {
        return;
    }

    // Spawn modal overlay (dim background)
    commands
        .spawn((
            LeaderboardModalUI,
            Node {
                width: Val::Percent(100.0),
                height: Val::Percent(100.0),
                position_type: PositionType::Absolute,
                justify_content: JustifyContent::Center,
                align_items: AlignItems::Center,
                ..default()
            },
            BackgroundColor(Color::srgba(0.0, 0.0, 0.0, 0.7)),
            GlobalZIndex(2000), // Above day-end modal
        ))
        .with_children(|overlay| {
            // Modal container
            overlay
                .spawn((
                    Node {
                        width: Val::Px(500.0),
                        max_height: Val::Percent(80.0),
                        flex_direction: FlexDirection::Column,
                        padding: UiRect::all(Val::Px(24.0)),
                        row_gap: Val::Px(12.0),
                        ..default()
                    },
                    BackgroundColor(Color::srgb(0.12, 0.14, 0.18)),
                    BorderColor::all(Color::srgb(0.3, 0.35, 0.4)),
                    BorderRadius::all(Val::Px(12.0)),
                ))
                .with_children(|modal| {
                    // Header row with title and close button
                    modal
                        .spawn(Node {
                            width: Val::Percent(100.0),
                            justify_content: JustifyContent::SpaceBetween,
                            align_items: AlignItems::Center,
                            margin: UiRect::bottom(Val::Px(8.0)),
                            ..default()
                        })
                        .with_children(|header| {
                            // Title (removed emoji for WASM compatibility)
                            header.spawn((
                                Text::new("LEADERBOARD"),
                                TextFont {
                                    font_size: 24.0,
                                    ..default()
                                },
                                TextColor(Color::srgb(1.0, 0.84, 0.0)), // Gold
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
                                    LeaderboardCloseButton,
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

                    // Divider
                    modal.spawn((
                        Node {
                            width: Val::Percent(100.0),
                            height: Val::Px(2.0),
                            margin: UiRect::bottom(Val::Px(8.0)),
                            ..default()
                        },
                        BackgroundColor(Color::srgb(0.3, 0.35, 0.4)),
                    ));

                    // Loading state text (Display::None so it doesn't reserve layout space)
                    modal.spawn((
                        Text::new("Loading leaderboard..."),
                        TextFont {
                            font_size: 13.0,
                            ..default()
                        },
                        TextColor(Color::srgb(0.7, 0.7, 0.7)),
                        Node {
                            margin: UiRect::all(Val::Px(12.0)),
                            display: Display::None,
                            ..default()
                        },
                        LeaderboardLoadingText,
                    ));

                    // Error state text (Display::None so it doesn't reserve layout space)
                    modal.spawn((
                        Text::new("Failed to load leaderboard"),
                        TextFont {
                            font_size: 13.0,
                            ..default()
                        },
                        TextColor(Color::srgb(0.9, 0.3, 0.3)),
                        Node {
                            margin: UiRect::all(Val::Px(12.0)),
                            display: Display::None,
                            ..default()
                        },
                        LeaderboardErrorText,
                    ));

                    // Scrollable container for entries
                    modal
                        .spawn((
                            Node {
                                width: Val::Percent(100.0),
                                height: Val::Px(450.0),
                                flex_direction: FlexDirection::Column,
                                overflow: Overflow::clip_y(),
                                ..default()
                            },
                            BackgroundColor(Color::srgba(0.0, 0.0, 0.0, 0.3)),
                            BorderRadius::all(Val::Px(6.0)),
                            LeaderboardEntriesContainer,
                        ))
                        .with_children(|_scroll_container| {
                            // Entries will be spawned/updated dynamically
                        });

                    // "Your Score" bar
                    let current_score = game_state.calculate_cumulative_score();
                    let score_color = if current_score >= 0 {
                        Color::srgb(0.4, 0.9, 0.4)
                    } else {
                        Color::srgb(0.9, 0.4, 0.4)
                    };

                    modal
                        .spawn((
                            Node {
                                width: Val::Percent(100.0),
                                flex_direction: FlexDirection::Row,
                                justify_content: JustifyContent::SpaceBetween,
                                align_items: AlignItems::Center,
                                padding: UiRect::new(
                                    Val::Px(12.0),
                                    Val::Px(12.0),
                                    Val::Px(10.0),
                                    Val::Px(10.0),
                                ),
                                margin: UiRect::top(Val::Px(4.0)),
                                ..default()
                            },
                            BackgroundColor(Color::srgba(1.0, 0.84, 0.0, 0.08)),
                            BorderRadius::all(Val::Px(6.0)),
                        ))
                        .with_children(|row| {
                            row.spawn((
                                Text::new("Your Score"),
                                TextFont {
                                    font_size: 15.0,
                                    ..default()
                                },
                                TextColor(Color::srgb(0.85, 0.85, 0.85)),
                            ));

                            row.spawn((
                                Text::new(format_score(current_score)),
                                TextFont {
                                    font_size: 15.0,
                                    ..default()
                                },
                                TextColor(score_color),
                                YourScoreText,
                            ));
                        });

                    // Help text
                    modal.spawn((
                        Text::new("Your score is automatically submitted at the end of each day!"),
                        TextFont {
                            font_size: 12.0,
                            ..default()
                        },
                        TextColor(Color::srgb(0.6, 0.6, 0.6)),
                        Node {
                            margin: UiRect::top(Val::Px(8.0)),
                            ..default()
                        },
                    ));
                });
        });
}

/// Despawn the leaderboard modal when it's closed
pub fn despawn_leaderboard_modal(
    mut commands: Commands,
    modal_state: Res<LeaderboardModalState>,
    modal_query: Query<Entity, With<LeaderboardModalUI>>,
) {
    // Despawn if closed and exists
    if !modal_state.is_open {
        for entity in &modal_query {
            commands.entity(entity).try_despawn();
        }
    }
}

/// Handle close button clicks
pub fn handle_leaderboard_close_button(
    interaction_query: Query<&Interaction, (Changed<Interaction>, With<LeaderboardCloseButton>)>,
    mut modal_state: ResMut<LeaderboardModalState>,
) {
    for interaction in &interaction_query {
        if *interaction == Interaction::Pressed {
            modal_state.close();
        }
    }
}

/// Update the leaderboard entries display
pub fn update_leaderboard_modal_content(
    leaderboard_data: Res<LeaderboardData>,
    mut commands: Commands,
    container_query: Query<Entity, With<LeaderboardEntriesContainer>>,
    entry_rows: Query<Entity, With<LeaderboardEntryRow>>,
    mut loading_text: Query<
        &mut Node,
        (
            With<LeaderboardLoadingText>,
            Without<LeaderboardErrorText>,
            Without<YourScoreText>,
        ),
    >,
    mut error_text: Query<
        (&mut Node, &mut Text),
        (
            With<LeaderboardErrorText>,
            Without<LeaderboardLoadingText>,
            Without<YourScoreText>,
        ),
    >,
    mut your_score_text: Query<
        (&mut Text, &mut TextColor),
        (
            With<YourScoreText>,
            Without<LeaderboardLoadingText>,
            Without<LeaderboardErrorText>,
        ),
    >,
    game_state: Res<GameState>,
    time: Res<Time>,
) {
    let now = time.elapsed_secs_f64();

    // Update loading/error display (use Display::None/Flex to avoid reserving layout space)
    for mut node in &mut loading_text {
        node.display = if leaderboard_data.is_loading {
            Display::Flex
        } else {
            Display::None
        };
    }

    for (mut node, mut text) in &mut error_text {
        if let Some(error) = &leaderboard_data.error {
            node.display = Display::Flex;

            // Build error message with retry info
            let mut error_msg = error.clone();
            if leaderboard_data.failed_attempts > 0 {
                let retry_in = leaderboard_data.retry_remaining_secs(now);
                if retry_in > 0.0 {
                    error_msg.push_str(&format!(
                        "\nRetrying in {:.0} seconds... (attempt {}/8)",
                        retry_in, leaderboard_data.failed_attempts
                    ));
                } else {
                    error_msg.push_str(&format!(
                        "\nRetrying... (attempt {}/8)",
                        leaderboard_data.failed_attempts
                    ));
                }
            }

            text.0 = error_msg;
        } else {
            node.display = Display::None;
        }
    }

    // Update "Your Score" display
    let current_score = game_state.calculate_cumulative_score();
    for (mut text, mut color) in &mut your_score_text {
        text.0 = format_score(current_score);
        *color = if current_score >= 0 {
            TextColor(Color::srgb(0.4, 0.9, 0.4))
        } else {
            TextColor(Color::srgb(0.9, 0.4, 0.4))
        };
    }

    // Rebuild entries when data changes or the container was freshly spawned with no
    // rows yet. This handles re-opening the modal with cached data: the UI is re-created
    // but LeaderboardData hasn't changed, so is_changed() alone would skip the rebuild
    // and leave the user looking at a blank leaderboard.
    let container_is_fresh = !container_query.is_empty() && entry_rows.is_empty();
    if !leaderboard_data.is_changed() && !container_is_fresh {
        return;
    }

    // Despawn all existing entry rows
    for entity in &entry_rows {
        commands.entity(entity).despawn();
    }

    // Rebuild the entries container
    for container_entity in &container_query {
        commands.entity(container_entity).with_children(|parent| {
            if leaderboard_data.entries.is_empty() {
                // No entries
                parent.spawn((
                    Text::new("No entries yet"),
                    TextFont {
                        font_size: 14.0,
                        ..default()
                    },
                    TextColor(Color::srgb(0.6, 0.6, 0.6)),
                    Node {
                        margin: UiRect::all(Val::Px(16.0)),
                        ..default()
                    },
                    LeaderboardEntryRow,
                ));
            } else {
                // Display entries
                for (index, entry) in leaderboard_data.entries.iter().enumerate() {
                    let rank = index + 1;

                    // Entry row
                    parent
                        .spawn((
                            Node {
                                width: Val::Percent(100.0),
                                flex_direction: FlexDirection::Row,
                                justify_content: JustifyContent::SpaceBetween,
                                align_items: AlignItems::Center,
                                padding: UiRect::all(Val::Px(12.0)),
                                border: UiRect::bottom(Val::Px(1.0)),
                                ..default()
                            },
                            BorderColor::all(Color::srgba(0.3, 0.35, 0.4, 0.5)),
                            LeaderboardEntryRow,
                        ))
                        .with_children(|row| {
                            // Left side: Rank and Name
                            row.spawn(Node {
                                flex_direction: FlexDirection::Row,
                                column_gap: Val::Px(16.0),
                                align_items: AlignItems::Center,
                                ..default()
                            })
                            .with_children(|left| {
                                // Rank with special colors for top 3
                                let rank_color = match rank {
                                    1 => Color::srgb(1.0, 0.84, 0.0),   // Gold
                                    2 => Color::srgb(0.75, 0.75, 0.75), // Silver
                                    3 => Color::srgb(0.8, 0.5, 0.2),    // Bronze
                                    _ => Color::srgb(0.6, 0.6, 0.6),
                                };

                                left.spawn((
                                    Text::new(format!("#{}", rank)),
                                    TextFont {
                                        font_size: 16.0,
                                        ..default()
                                    },
                                    TextColor(rank_color),
                                    Node {
                                        width: Val::Px(50.0),
                                        ..default()
                                    },
                                ));

                                // Player name
                                left.spawn((
                                    Text::new(&entry.player_name),
                                    TextFont {
                                        font_size: 15.0,
                                        ..default()
                                    },
                                    TextColor(Color::srgb(0.9, 0.9, 0.9)),
                                ));
                            });

                            // Right side: Score
                            let score_color = if entry.score >= 0 {
                                Color::srgb(0.4, 0.9, 0.4) // Green for positive
                            } else {
                                Color::srgb(0.9, 0.4, 0.4) // Red for negative
                            };

                            row.spawn((
                                Text::new(format_score(entry.score)),
                                TextFont {
                                    font_size: 15.0,
                                    ..default()
                                },
                                TextColor(score_color),
                            ));
                        });
                }
            }
        });
    }
}

/// Format a score with dollar sign and thousand separators (e.g., $1,250,000)
fn format_score(score: i64) -> String {
    let abs_score = score.abs();
    let sign = if score < 0 { "-" } else { "" };

    if abs_score >= 1_000_000 {
        format!("{}${:.2}M", sign, abs_score as f64 / 1_000_000.0)
    } else if abs_score >= 1_000 {
        format!("{}${:.1}K", sign, abs_score as f64 / 1_000.0)
    } else {
        format!("{}${}", sign, abs_score)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // ---- LeaderboardModalState ----

    #[test]
    fn test_modal_state_default_is_closed() {
        let state = LeaderboardModalState::default();
        assert!(!state.is_open);
    }

    #[test]
    fn test_modal_state_open() {
        let mut state = LeaderboardModalState::default();
        state.open();
        assert!(state.is_open);
    }

    #[test]
    fn test_modal_state_close() {
        let mut state = LeaderboardModalState::default();
        state.open();
        state.close();
        assert!(!state.is_open);
    }

    #[test]
    fn test_modal_state_toggle() {
        let mut state = LeaderboardModalState::default();
        assert!(!state.is_open);

        state.toggle();
        assert!(state.is_open);

        state.toggle();
        assert!(!state.is_open);
    }

    // ---- format_score ----

    #[test]
    fn test_format_score_zero() {
        assert_eq!(format_score(0), "$0");
    }

    #[test]
    fn test_format_score_small_positive() {
        assert_eq!(format_score(42), "$42");
        assert_eq!(format_score(999), "$999");
    }

    #[test]
    fn test_format_score_thousands() {
        assert_eq!(format_score(1_000), "$1.0K");
        assert_eq!(format_score(2_100), "$2.1K");
        assert_eq!(format_score(999_999), "$1000.0K");
    }

    #[test]
    fn test_format_score_millions() {
        assert_eq!(format_score(1_000_000), "$1.00M");
        assert_eq!(format_score(1_500_000), "$1.50M");
        assert_eq!(format_score(12_345_678), "$12.35M");
    }

    #[test]
    fn test_format_score_negative_small() {
        assert_eq!(format_score(-23), "-$23");
        assert_eq!(format_score(-500), "-$500");
    }

    #[test]
    fn test_format_score_negative_thousands() {
        assert_eq!(format_score(-1_000), "-$1.0K");
        assert_eq!(format_score(-97_000), "-$97.0K");
    }

    #[test]
    fn test_format_score_negative_millions() {
        assert_eq!(format_score(-2_500_000), "-$2.50M");
    }
}
