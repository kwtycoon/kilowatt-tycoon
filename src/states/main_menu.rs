//! Main menu state systems and UI.
//!
//! Features a dynamic splash screen with an isometric city background
//! and subtle static noise effect for a retro cyberpunk atmosphere.

use bevy::prelude::*;

use super::{AppState, MainMenuUI};
use crate::resources::ImageAssets;

/// Types of buttons in the main menu
#[derive(Component, Debug, Clone, Copy)]
pub enum MainMenuButton {
    StartGame,
    Quit,
}

// ============ Traffic Animation Components ============

/// Marker for animated background sprites
#[derive(Component)]
pub struct MenuBackgroundSprite;

/// Resource to track if we've spawned the background yet
#[derive(Resource)]
pub struct MenuBackgroundSpawned(pub bool);

/// Lightning bolt glow animation in title
#[derive(Component)]
pub struct LightningBoltGlow {
    pub timer: f32,
}

/// Resource to track if we've spawned the UI yet
#[derive(Resource)]
pub struct MenuUISpawned(pub bool);

/// Setup the main menu UI
pub fn setup_main_menu(mut commands: Commands) {
    commands.insert_resource(MenuBackgroundSpawned(false));
    commands.insert_resource(MenuUISpawned(false));
}

/// Spawn animated background once assets are loaded
pub fn spawn_menu_background_when_ready(
    mut commands: Commands,
    image_assets: Option<Res<ImageAssets>>,
    mut spawned: ResMut<MenuBackgroundSpawned>,
) {
    // Already spawned, nothing to do
    if spawned.0 {
        return;
    }

    // Assets not ready yet, try again next frame
    let Some(image_assets) = image_assets else {
        return;
    };

    spawned.0 = true;

    // Spawn the background image (centered, scaled to fill screen)
    commands.spawn((
        MenuBackgroundSprite,
        Sprite {
            image: image_assets.splash_background.clone(),
            ..default()
        },
        Transform::from_xyz(450.0, 200.0, 0.0).with_scale(Vec3::splat(1.2)),
    ));
}

/// Spawn menu UI once assets are loaded
pub fn spawn_menu_ui_when_ready(
    mut commands: Commands,
    image_assets: Option<Res<ImageAssets>>,
    mut spawned: ResMut<MenuUISpawned>,
) {
    // Already spawned, nothing to do
    if spawned.0 {
        return;
    }

    // Assets not ready yet, try again next frame
    let Some(image_assets) = image_assets else {
        return;
    };

    spawned.0 = true;
    spawn_menu_ui(&mut commands, &image_assets);
}

/// Animate lightning bolt with pulsing glow effect
pub fn animate_lightning_bolt_glow(
    time: Res<Time>,
    mut bolt_query: Query<(&mut LightningBoltGlow, &mut ImageNode)>,
) {
    for (mut glow, mut image_node) in &mut bolt_query {
        glow.timer += time.delta_secs();

        // Create smooth pulsing glow using sine wave (cycle every ~1.5 seconds)
        let pulse = (glow.timer * 4.0).sin() * 0.5 + 0.5; // 0.0 to 1.0

        // Glow between warm yellow and bright white-yellow
        let r = 1.0;
        let g = 0.75 + pulse * 0.25; // 0.75 to 1.0
        let b = 0.2 + pulse * 0.6; // 0.2 to 0.8
        image_node.color = Color::srgb(r, g, b);
    }
}

/// Spawn the menu UI overlay
fn spawn_menu_ui(commands: &mut Commands, image_assets: &ImageAssets) {
    commands
        .spawn((
            MainMenuUI,
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
            BackgroundColor(Color::srgba(0.08, 0.10, 0.15, 0.75)), // Semi-transparent overlay
            GlobalZIndex(900),
        ))
        .with_children(|parent| {
            // ============ TITLE SECTION ============
            // "KIL⚡WATT" on first line
            parent
                .spawn(Node {
                    flex_direction: FlexDirection::Row,
                    align_items: AlignItems::Center,
                    ..default()
                })
                .with_children(|row| {
                    // "KIL" in orange/yellow
                    row.spawn((
                        Text::new("KIL"),
                        TextFont {
                            font_size: 84.0,
                            ..default()
                        },
                        TextColor(Color::srgb(1.0, 0.75, 0.2)), // Orange-yellow
                    ));

                    // Lightning bolt icon with glow animation
                    row.spawn((
                        LightningBoltGlow { timer: 0.0 },
                        ImageNode::new(image_assets.icon_bolt.clone()),
                        Node {
                            width: Val::Px(64.0),
                            height: Val::Px(64.0),
                            margin: UiRect::horizontal(Val::Px(4.0)),
                            ..default()
                        },
                    ));

                    // "WATT" in orange/yellow
                    row.spawn((
                        Text::new("WATT"),
                        TextFont {
                            font_size: 84.0,
                            ..default()
                        },
                        TextColor(Color::srgb(1.0, 0.75, 0.2)), // Orange-yellow
                    ));
                });

            // "TYCOON" in cyan/teal
            parent.spawn((
                Text::new("TYCOON"),
                TextFont {
                    font_size: 72.0,
                    ..default()
                },
                TextColor(Color::srgb(0.2, 0.8, 0.9)), // Cyan/teal
            ));

            // Tagline
            parent.spawn((
                Text::new("Build. Charge. Profit."),
                TextFont {
                    font_size: 18.0,
                    ..default()
                },
                TextColor(Color::srgb(0.7, 0.75, 0.8)),
            ));

            // Spacer
            parent.spawn(Node {
                height: Val::Px(40.0),
                ..default()
            });

            // ============ BUTTONS SECTION ============
            // Start game button (green)
            parent
                .spawn((
                    Button,
                    Node {
                        width: Val::Px(280.0),
                        height: Val::Px(55.0),
                        justify_content: JustifyContent::Center,
                        align_items: AlignItems::Center,
                        ..default()
                    },
                    BackgroundColor(Color::srgb(0.3, 0.69, 0.31)), // #4CAF50
                    BorderRadius::all(Val::Px(6.0)),
                    MainMenuButton::StartGame,
                ))
                .with_children(|btn| {
                    btn.spawn((
                        Text::new("START GAME"),
                        TextFont {
                            font_size: 24.0,
                            ..default()
                        },
                        TextColor(Color::WHITE),
                    ));
                });

            // Spacer between buttons
            parent.spawn(Node {
                height: Val::Px(10.0),
                ..default()
            });

            // Quit button (red/coral)
            parent
                .spawn((
                    Button,
                    Node {
                        width: Val::Px(280.0),
                        height: Val::Px(55.0),
                        justify_content: JustifyContent::Center,
                        align_items: AlignItems::Center,
                        ..default()
                    },
                    BackgroundColor(Color::srgb(0.9, 0.45, 0.45)), // #E57373
                    BorderRadius::all(Val::Px(6.0)),
                    MainMenuButton::Quit,
                ))
                .with_children(|btn| {
                    btn.spawn((
                        Text::new("QUIT"),
                        TextFont {
                            font_size: 24.0,
                            ..default()
                        },
                        TextColor(Color::WHITE),
                    ));
                });

            // ============ FOOTER SECTION ============
            // Spacer
            parent.spawn(Node {
                height: Val::Px(60.0),
                ..default()
            });

            // Platform icons row
            parent
                .spawn(Node {
                    flex_direction: FlexDirection::Row,
                    align_items: AlignItems::Center,
                    column_gap: Val::Px(24.0),
                    ..default()
                })
                .with_children(|row| {
                    // Windows
                    row.spawn((
                        ImageNode::new(image_assets.icon_platform_windows.clone()),
                        Node {
                            width: Val::Px(24.0),
                            height: Val::Px(24.0),
                            ..default()
                        },
                    ));
                    // macOS
                    row.spawn((
                        ImageNode::new(image_assets.icon_platform_macos.clone()),
                        Node {
                            width: Val::Px(24.0),
                            height: Val::Px(24.0),
                            ..default()
                        },
                    ));
                    // Linux
                    row.spawn((
                        ImageNode::new(image_assets.icon_platform_linux.clone()),
                        Node {
                            width: Val::Px(24.0),
                            height: Val::Px(24.0),
                            ..default()
                        },
                    ));
                });

            // Version text
            parent.spawn((
                Text::new("v0.1.0 - Open Source EV CPO Simulator"),
                TextFont {
                    font_size: 14.0,
                    ..default()
                },
                TextColor(Color::srgb(0.4, 0.45, 0.5)),
            ));
        });
}

/// Cleanup main menu when leaving
pub fn cleanup_main_menu(
    mut commands: Commands,
    ui_query: Query<Entity, With<MainMenuUI>>,
    bg_query: Query<Entity, With<MenuBackgroundSprite>>,
) {
    for entity in &ui_query {
        commands.entity(entity).try_despawn();
    }
    for entity in &bg_query {
        commands.entity(entity).try_despawn();
    }
    commands.remove_resource::<MenuBackgroundSpawned>();
    commands.remove_resource::<MenuUISpawned>();
}

/// Handle main menu button interactions
pub fn main_menu_system(
    mut next_state: ResMut<NextState<AppState>>,
    mut interaction_query: Query<
        (&Interaction, &MainMenuButton, &mut BackgroundColor),
        (Changed<Interaction>, With<Button>),
    >,
    keyboard: Res<ButtonInput<KeyCode>>,
) {
    // Handle button hover and click
    for (interaction, button, mut bg_color) in &mut interaction_query {
        match *interaction {
            Interaction::Pressed => match button {
                MainMenuButton::StartGame => {
                    next_state.set(AppState::Loading);
                }
                MainMenuButton::Quit => {
                    std::process::exit(0);
                }
            },
            Interaction::Hovered => match button {
                MainMenuButton::StartGame => {
                    *bg_color = BackgroundColor(Color::srgb(0.4, 0.78, 0.4)); // Lighter green
                }
                MainMenuButton::Quit => {
                    *bg_color = BackgroundColor(Color::srgb(0.95, 0.55, 0.55)); // Lighter red
                }
            },
            Interaction::None => match button {
                MainMenuButton::StartGame => {
                    *bg_color = BackgroundColor(Color::srgb(0.3, 0.69, 0.31)); // #4CAF50
                }
                MainMenuButton::Quit => {
                    *bg_color = BackgroundColor(Color::srgb(0.9, 0.45, 0.45)); // #E57373
                }
            },
        }
    }

    // Keyboard shortcuts
    if keyboard.just_pressed(KeyCode::Enter) || keyboard.just_pressed(KeyCode::Space) {
        next_state.set(AppState::Loading);
    }
    if keyboard.just_pressed(KeyCode::Escape) {
        std::process::exit(0);
    }
}
