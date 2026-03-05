use bevy::prelude::*;

use crate::resources::SelectedChargerEntity;
use crate::states::AppState;

/// Marker component for pause menu UI elements.
#[derive(Component)]
pub struct PauseMenuUI;

/// Types of buttons in the pause menu.
#[derive(Component, Debug, Clone, Copy)]
pub enum PauseMenuButton {
    Resume,
    MainMenu,
}

/// Toggle pause state with Escape key.
pub fn toggle_pause(
    keyboard: Res<ButtonInput<KeyCode>>,
    current_state: Res<State<AppState>>,
    mut next_state: ResMut<NextState<AppState>>,
    selected: Res<SelectedChargerEntity>,
    tutorial: Res<crate::resources::TutorialState>,
) {
    if keyboard.just_pressed(KeyCode::Escape) {
        if selected.0.is_some() {
            return;
        }
        if tutorial.is_active() {
            return;
        }
        match current_state.get() {
            AppState::Playing => next_state.set(AppState::Paused),
            AppState::Paused => next_state.set(AppState::Playing),
            _ => {}
        }
    }
}

/// Handle pause menu button interactions.
pub fn pause_menu_system(
    keyboard: Res<ButtonInput<KeyCode>>,
    mut next_state: ResMut<NextState<AppState>>,
    mut interaction_query: Query<
        (&Interaction, &PauseMenuButton),
        (Changed<Interaction>, With<Button>),
    >,
) {
    for (interaction, button) in &mut interaction_query {
        if *interaction == Interaction::Pressed {
            match button {
                PauseMenuButton::Resume => next_state.set(AppState::Playing),
                PauseMenuButton::MainMenu => next_state.set(AppState::Loading),
            }
        }
    }

    if keyboard.just_pressed(KeyCode::KeyR) {
        next_state.set(AppState::Playing);
    }
}

/// Called when entering the paused state.
pub fn on_enter_paused(mut commands: Commands) {
    commands
        .spawn((
            PauseMenuUI,
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
            BackgroundColor(Color::srgba(0.0, 0.0, 0.0, 0.7)),
            GlobalZIndex(1000),
        ))
        .with_children(|parent| {
            parent.spawn((
                Text::new("PAUSED"),
                TextFont {
                    font_size: 48.0,
                    ..default()
                },
                TextColor(Color::WHITE),
            ));

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
                    PauseMenuButton::Resume,
                ))
                .with_children(|btn| {
                    btn.spawn((
                        Text::new("Resume (R)"),
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
                    BackgroundColor(Color::srgb(0.6, 0.2, 0.2)),
                    PauseMenuButton::MainMenu,
                ))
                .with_children(|btn| {
                    btn.spawn((
                        Text::new("Main Menu"),
                        TextFont {
                            font_size: 20.0,
                            ..default()
                        },
                        TextColor(Color::WHITE),
                    ));
                });
        });
}

/// Called when exiting the paused state.
pub fn on_exit_paused(mut commands: Commands, query: Query<Entity, With<PauseMenuUI>>) {
    for entity in &query {
        commands.entity(entity).try_despawn();
    }
}
