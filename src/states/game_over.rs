use bevy::prelude::*;

use crate::states::AppState;

/// Marker component for game over UI elements.
#[derive(Component)]
pub struct GameOverUI;

/// Types of buttons in the game over screen.
#[derive(Component, Debug, Clone, Copy)]
pub enum GameOverButton {
    PlayAgain,
    MainMenu,
}

/// Handle game over screen button interactions.
pub fn game_over_system(
    keyboard: Res<ButtonInput<KeyCode>>,
    mut next_state: ResMut<NextState<AppState>>,
    mut interaction_query: Query<
        (&Interaction, &GameOverButton),
        (Changed<Interaction>, With<Button>),
    >,
) {
    for (interaction, button) in &mut interaction_query {
        if *interaction == Interaction::Pressed {
            match button {
                GameOverButton::PlayAgain => next_state.set(AppState::Loading),
                GameOverButton::MainMenu => next_state.set(AppState::Loading),
            }
        }
    }

    if keyboard.just_pressed(KeyCode::Space) {
        next_state.set(AppState::Loading);
    }
    if keyboard.just_pressed(KeyCode::Escape) {
        next_state.set(AppState::Loading);
    }
}

/// Setup game over UI.
pub fn setup_game_over(mut commands: Commands, game_state: Res<crate::resources::GameState>) {
    let (title, color) = match game_state.result {
        crate::resources::GameResult::Won => ("YOU WIN!", Color::srgb(0.2, 0.8, 0.2)),
        crate::resources::GameResult::LostBankruptcy => ("BANKRUPT!", Color::srgb(0.8, 0.2, 0.2)),
        crate::resources::GameResult::LostReputation => {
            ("REPUTATION LOST!", Color::srgb(0.8, 0.4, 0.2))
        }
        crate::resources::GameResult::LostTimeout => ("TIME'S UP!", Color::srgb(0.8, 0.6, 0.2)),
        crate::resources::GameResult::InProgress => ("GAME OVER", Color::WHITE),
    };

    commands
        .spawn((
            GameOverUI,
            Node {
                width: Val::Percent(100.0),
                height: Val::Percent(100.0),
                position_type: PositionType::Absolute,
                justify_content: JustifyContent::Center,
                align_items: AlignItems::Center,
                flex_direction: FlexDirection::Column,
                row_gap: Val::Px(20.0),
                ..default()
            },
            BackgroundColor(Color::srgba(0.0, 0.0, 0.0, 0.85)),
            GlobalZIndex(1000),
        ))
        .with_children(|parent| {
            parent.spawn((
                Text::new(title),
                TextFont {
                    font_size: 64.0,
                    ..default()
                },
                TextColor(color),
            ));

            let total_cable_theft: f32 = {
                use rust_decimal::prelude::ToPrimitive;
                game_state
                    .ledger
                    .account_balance(crate::resources::ledger::Account::CableTheft)
                    .to_f32()
                    .unwrap_or(0.0)
            };

            parent.spawn((
                Text::new(format!(
                    "Revenue: ${:.0}\nReputation: {}\nSessions: {}",
                    game_state.ledger.net_revenue_f32(),
                    game_state.reputation,
                    game_state.sessions_completed
                )),
                TextFont {
                    font_size: 24.0,
                    ..default()
                },
                TextColor(Color::WHITE),
            ));

            if total_cable_theft > 0.01 {
                parent.spawn((
                    Text::new(format!(
                        "🔌 Total Cable Theft Losses: -${:.0}",
                        total_cable_theft
                    )),
                    TextFont {
                        font_size: 22.0,
                        ..default()
                    },
                    TextColor(Color::srgb(1.0, 0.2, 0.2)),
                ));
            }

            parent
                .spawn((
                    Button,
                    Node {
                        width: Val::Px(200.0),
                        height: Val::Px(50.0),
                        justify_content: JustifyContent::Center,
                        align_items: AlignItems::Center,
                        ..default()
                    },
                    BackgroundColor(Color::srgb(0.2, 0.6, 0.2)),
                    GameOverButton::PlayAgain,
                ))
                .with_children(|btn| {
                    btn.spawn((
                        Text::new("Play Again (Space)"),
                        TextFont {
                            font_size: 20.0,
                            ..default()
                        },
                        TextColor(Color::WHITE),
                    ));
                });

            parent
                .spawn((
                    Button,
                    Node {
                        width: Val::Px(200.0),
                        height: Val::Px(50.0),
                        justify_content: JustifyContent::Center,
                        align_items: AlignItems::Center,
                        ..default()
                    },
                    BackgroundColor(Color::srgb(0.4, 0.4, 0.4)),
                    GameOverButton::MainMenu,
                ))
                .with_children(|btn| {
                    btn.spawn((
                        Text::new("Main Menu (Esc)"),
                        TextFont {
                            font_size: 20.0,
                            ..default()
                        },
                        TextColor(Color::WHITE),
                    ));
                });
        });
}

/// Cleanup game over UI.
pub fn cleanup_game_over(mut commands: Commands, query: Query<Entity, With<GameOverUI>>) {
    for entity in &query {
        commands.entity(entity).try_despawn();
    }
}
