//! Win/lose overlay

use bevy::prelude::*;

use crate::resources::{GameClock, GameResult, GameState};

// ============ Marker Components ============

#[derive(Component)]
pub struct OverlayRoot;

#[derive(Component)]
pub struct OverlayTitle;

#[derive(Component)]
pub struct OverlayStats;

#[derive(Component)]
pub struct TryAgainButton;

#[derive(Component)]
pub struct ContinueButton;

// ============ Setup ============

pub fn setup_overlay(mut commands: Commands, existing: Query<Entity, With<OverlayRoot>>) {
    if !existing.is_empty() {
        return;
    }

    commands
        .spawn((
            Node {
                position_type: PositionType::Absolute,
                width: Val::Percent(100.0),
                height: Val::Percent(100.0),
                justify_content: JustifyContent::Center,
                align_items: AlignItems::Center,
                ..default()
            },
            BackgroundColor(Color::srgba(0.0, 0.0, 0.0, 0.8)),
            Visibility::Hidden,
            OverlayRoot,
        ))
        .with_children(|overlay| {
            // Panel
            overlay
                .spawn((
                    Node {
                        width: Val::Px(400.0),
                        flex_direction: FlexDirection::Column,
                        align_items: AlignItems::Center,
                        padding: UiRect::all(Val::Px(32.0)),
                        row_gap: Val::Px(16.0),
                        border: UiRect::all(Val::Px(2.0)),
                        ..default()
                    },
                    BackgroundColor(Color::srgb(0.15, 0.15, 0.15)),
                    BorderColor::all(Color::srgb(0.4, 0.4, 0.4)),
                ))
                .with_children(|panel| {
                    // Title
                    panel.spawn((
                        Text::new("YOU WIN!"),
                        TextFont {
                            font_size: 36.0,
                            ..default()
                        },
                        TextColor(Color::srgb(0.2, 0.8, 0.2)),
                        OverlayTitle,
                    ));

                    // Stats
                    panel.spawn((
                        Text::new("Revenue: $100\nSessions: 8"),
                        TextFont {
                            font_size: 18.0,
                            ..default()
                        },
                        TextColor(Color::WHITE),
                        OverlayStats,
                    ));

                    // Buttons
                    panel
                        .spawn(Node {
                            column_gap: Val::Px(16.0),
                            margin: UiRect::top(Val::Px(16.0)),
                            ..default()
                        })
                        .with_children(|btns| {
                            // Try Again
                            btns.spawn((
                                Button,
                                Node {
                                    padding: UiRect::axes(Val::Px(24.0), Val::Px(12.0)),
                                    ..default()
                                },
                                BackgroundColor(Color::srgb(0.3, 0.5, 0.3)),
                                TryAgainButton,
                            ))
                            .with_child((
                                Text::new("Try Again"),
                                TextFont {
                                    font_size: 18.0,
                                    ..default()
                                },
                            ));

                            // Continue
                            btns.spawn((
                                Button,
                                Node {
                                    padding: UiRect::axes(Val::Px(24.0), Val::Px(12.0)),
                                    ..default()
                                },
                                BackgroundColor(Color::srgb(0.3, 0.3, 0.5)),
                                ContinueButton,
                            ))
                            .with_child((
                                Text::new("Continue"),
                                TextFont {
                                    font_size: 18.0,
                                    ..default()
                                },
                            ));
                        });
                });
        });
}

// ============ Update Systems ============

pub fn update_overlay(
    game_state: Res<GameState>,
    mut overlay_q: Query<&mut Visibility, With<OverlayRoot>>,
    mut title_q: Query<(&mut Text, &mut TextColor), With<OverlayTitle>>,
    mut stats_q: Query<&mut Text, (With<OverlayStats>, Without<OverlayTitle>)>,
    mut continue_btn_q: Query<&mut Visibility, (With<ContinueButton>, Without<OverlayRoot>)>,
) {
    for mut overlay_vis in &mut overlay_q {
        if !game_state.result.is_ended() {
            *overlay_vis = Visibility::Hidden;
            continue;
        }

        *overlay_vis = Visibility::Inherited;
    }

    if !game_state.result.is_ended() {
        return;
    }

    // Update title
    for (mut text, mut color) in &mut title_q {
        **text = game_state.result.title().to_string();
        *color = TextColor(if matches!(game_state.result, GameResult::Won) {
            Color::srgb(0.2, 0.8, 0.2)
        } else {
            Color::srgb(0.8, 0.2, 0.2)
        });
    }

    // Update stats
    for mut text in &mut stats_q {
        **text = format!(
            "Revenue: ${:.0}\nSessions: {}",
            game_state.net_revenue, game_state.sessions_completed
        );
    }

    // Show continue button only on win
    for mut btn_vis in &mut continue_btn_q {
        *btn_vis = if matches!(game_state.result, GameResult::Won) {
            Visibility::Inherited
        } else {
            Visibility::Hidden
        };
    }
}

pub fn handle_overlay_buttons(
    mut game_state: ResMut<GameState>,
    mut game_clock: ResMut<GameClock>,
    try_again_btns: Query<&Interaction, (Changed<Interaction>, With<TryAgainButton>)>,
    continue_btns: Query<&Interaction, (Changed<Interaction>, With<ContinueButton>)>,
) {
    // Try Again
    for interaction in &try_again_btns {
        if *interaction == Interaction::Pressed {
            game_state.reset();
            game_clock.reset();
            info!("Game reset");
        }
    }

    // Continue
    for interaction in &continue_btns {
        if *interaction == Interaction::Pressed {
            // Allow continuing in sandbox mode
            game_state.result = GameResult::InProgress;
            game_clock.resume();
            info!("Continuing in sandbox mode");
        }
    }
}
