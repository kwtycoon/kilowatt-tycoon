//! Character selection and name input state systems and UI

use bevy::ecs::hierarchy::ChildSpawnerCommands;
use bevy::prelude::*;
use rand::Rng;

use super::CharacterSetupUI;
use crate::helpers::ui_builders::colors;
use crate::resources::{CharacterKind, ImageAssets, PlayerProfile, TutorialState};

/// Internal step within the CharacterSetup state
#[derive(Resource, Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum SetupStep {
    #[default]
    CharacterSelection,
    NameInput,
}

/// Marker component for character selection cards
#[derive(Component, Debug, Clone, Copy)]
pub struct CharacterCard {
    pub kind: CharacterKind,
}

/// Marker component for the cards container (to enable despawn/respawn on selection change)
#[derive(Component)]
pub struct CardsContainer;

/// Marker component for the "Next" button in character selection
#[derive(Component)]
pub struct NextButton;

/// Marker component for the "Start" button in name input
#[derive(Component)]
pub struct StartButton;

/// Marker component for the text input field
#[derive(Component)]
pub struct NameInputText;

/// Marker component for the blinking cursor
#[derive(Component)]
pub struct BlinkingCursor {
    pub timer: Timer,
}

impl Default for BlinkingCursor {
    fn default() -> Self {
        Self {
            timer: Timer::from_seconds(0.5, TimerMode::Repeating),
        }
    }
}

/// Marker for the character preview in step 2
#[derive(Component)]
pub struct CharacterPreview;

/// Funny placeholder tycoon names that cycle in the name input field
const PLACEHOLDER_NAMES: &[&str] = &[
    "Wattson Prime",
    "Sir Charges-a-Lot",
    "Grid McConnected",
    "Volt Vibes",
    "Amped & Ready",
    "Pluggy Smalls",
    "Current Affairs",
    "Ohm Sweet Ohm",
    "Relay On",
    "Chargezilla",
    "Static Spark",
    "Kilowatt Cowboy",
    "Power Ranger-ish",
    "Zap Stack",
    "Load Balancer",
    "Peak Shaver",
    "Electron Whisperer",
    "Phase Changer",
    "The Gridfather",
    "Fast & the Curious",
    "OCPP-timus Prime",
    "The OCPP-erator",
    "Cap'n OCPP",
    "Agent OCPP",
    "OCPP-rah Winfrey",
    "OCPP & Loaded",
    "OpenADR-enaline",
    "ADR-ian Volt",
    "OpenADR-ift",
    "OCPI Wan Kenobi",
    "MC OCPI",
    "OCPI-derman",
    "Professor OCPI",
    "Surge Protector",
    "Flux Capacitor",
    "AC/DC Slater",
    "Amp-ire State",
    "Joule Thief",
    "Watt's Up Doc",
    "Megawatt Mind",
    "Circuit Breaker",
    "The Transformer",
    "Demand Charger",
];

/// Cycles through placeholder names in the name input field until the player types
#[derive(Component)]
pub struct PlaceholderCycler {
    pub timer: Timer,
    pub index: usize,
    pub active: bool,
}

/// Setup the character selection screen (runs as overlay during Playing state)
pub fn setup_character_selection(
    mut commands: Commands,
    image_assets: Res<ImageAssets>,
    mut profile: ResMut<PlayerProfile>,
    existing_step: Option<Res<SetupStep>>,
) {
    // Only show on first play; skip if setup already done or in progress
    if existing_step.is_some() || profile.character.is_some() {
        return;
    }

    // Initialize the setup step resource
    commands.insert_resource(SetupStep::CharacterSelection);

    // Default to Raccoon selected
    profile.character = Some(CharacterKind::Raccoon);

    spawn_character_selection_ui_with_profile(
        &mut commands,
        &image_assets,
        Some(CharacterKind::Raccoon),
    );
}

/// Spawn the character selection UI with an optional selected character
fn spawn_character_selection_ui_with_profile(
    commands: &mut Commands,
    image_assets: &ImageAssets,
    selected: Option<CharacterKind>,
) {
    // Create the root UI container (semi-transparent so the game world is visible behind)
    commands
        .spawn((
            CharacterSetupUI,
            Node {
                width: Val::Percent(100.0),
                height: Val::Percent(100.0),
                position_type: PositionType::Absolute,
                justify_content: JustifyContent::Center,
                align_items: AlignItems::Center,
                ..default()
            },
            BackgroundColor(colors::OVERLAY_BG),
            GlobalZIndex(1000),
        ))
        .with_children(|parent| {
            // Modal panel container
            parent
                .spawn((
                    Node {
                        flex_direction: FlexDirection::Column,
                        padding: UiRect::all(Val::Px(20.0)),
                        row_gap: Val::Px(20.0),
                        border: UiRect::all(Val::Px(2.0)),
                        ..default()
                    },
                    BackgroundColor(colors::PANEL_BG),
                    BorderColor::all(Color::srgba(0.0, 0.7, 0.5, 0.6)),
                    BorderRadius::all(Val::Px(12.0)),
                ))
                .with_children(|modal| {
                    // Character cards container
                    modal
                        .spawn((
                            Node {
                                flex_direction: FlexDirection::Row,
                                column_gap: Val::Px(30.0),
                                align_items: AlignItems::Center,
                                ..default()
                            },
                            CardsContainer,
                        ))
                        .with_children(|cards| {
                            // Determine card order: selected in center, others on sides
                            let characters = if let Some(sel) = selected {
                                // Reorder so selected is in the middle
                                let mut chars = CharacterKind::all().to_vec();
                                chars.retain(|&c| c != sel);
                                vec![chars[0], sel, chars[1]]
                            } else {
                                CharacterKind::all().to_vec()
                            };

                            for character in characters {
                                let is_selected = Some(character) == selected;
                                spawn_character_card(cards, image_assets, character, is_selected);
                            }
                        });
                });
        });
}

/// Spawn a character card (selected or unselected style)
fn spawn_character_card(
    parent: &mut ChildSpawnerCommands,
    image_assets: &ImageAssets,
    character: CharacterKind,
    is_selected: bool,
) {
    let image_handle = match character {
        CharacterKind::Ant => image_assets.character_main_ant.clone(),
        CharacterKind::Mallard => image_assets.character_main_mallard.clone(),
        CharacterKind::Raccoon => image_assets.character_main_raccoon.clone(),
    };

    let perk = character.perk();

    if is_selected {
        // Selected card: Large with full details
        parent
            .spawn((
                Node {
                    width: Val::Px(320.0),
                    height: Val::Px(620.0),
                    flex_direction: FlexDirection::Column,
                    justify_content: JustifyContent::FlexStart,
                    align_items: AlignItems::Center,
                    padding: UiRect::all(Val::Px(0.0)),
                    ..default()
                },
                BackgroundColor(colors::PANEL_BG),
                BorderRadius::all(Val::Px(12.0)),
                Outline {
                    width: Val::Px(4.0),
                    offset: Val::Px(0.0),
                    color: Color::srgb(0.2, 0.7, 0.2), // TYCOON_GREEN
                },
            ))
            .with_children(|card| {
                // Title at top
                card.spawn(Node {
                    width: Val::Percent(100.0),
                    padding: UiRect::all(Val::Px(16.0)),
                    justify_content: JustifyContent::Center,
                    ..default()
                })
                .with_children(|title_section| {
                    title_section.spawn((
                        Text::new("SELECT YOUR OPERATOR"),
                        TextFont {
                            font_size: 18.0,
                            ..default()
                        },
                        TextColor(colors::TEXT_PRIMARY),
                    ));
                });

                // Character portrait - clickable area
                card.spawn((
                    Button,
                    Node {
                        width: Val::Px(320.0),
                        height: Val::Px(240.0),
                        ..default()
                    },
                    CharacterCard { kind: character },
                ))
                .with_children(|img_btn| {
                    img_btn.spawn((
                        ImageNode::new(image_handle),
                        Node {
                            width: Val::Percent(100.0),
                            height: Val::Percent(100.0),
                            ..default()
                        },
                    ));
                });

                // Content section
                card.spawn(Node {
                    width: Val::Percent(100.0),
                    flex_direction: FlexDirection::Column,
                    padding: UiRect::all(Val::Px(16.0)),
                    row_gap: Val::Px(8.0),
                    flex_grow: 1.0,
                    ..default()
                })
                .with_children(|content| {
                    // Character name
                    content.spawn((
                        Text::new(character.display_name()),
                        TextFont {
                            font_size: 26.0,
                            ..default()
                        },
                        TextColor(colors::TEXT_PRIMARY),
                    ));

                    // Role
                    content.spawn((
                        Text::new(character.role()),
                        TextFont {
                            font_size: 16.0,
                            ..default()
                        },
                        TextColor(Color::srgb(0.2, 0.7, 0.2)), // Green accent
                    ));

                    // Bio
                    content.spawn((
                        Text::new(character.bio()),
                        TextFont {
                            font_size: 14.0,
                            ..default()
                        },
                        TextColor(colors::TEXT_SECONDARY),
                        TextLayout::new_with_linebreak(bevy::text::LineBreak::WordBoundary),
                    ));
                });

                // Perk section (darker background strip)
                card.spawn(Node {
                    width: Val::Percent(100.0),
                    flex_direction: FlexDirection::Column,
                    padding: UiRect::all(Val::Px(16.0)),
                    row_gap: Val::Px(4.0),
                    ..default()
                })
                .with_children(|perk_section| {
                    // Perk name only (no description — consistent across characters)
                    perk_section.spawn((
                        Text::new(format!("PERK: {}", perk.name().to_uppercase())),
                        TextFont {
                            font_size: 18.0,
                            ..default()
                        },
                        TextColor(colors::MODAL_BORDER_GLOW), // Bright neon green so perk stands out
                    ));
                });

                // Next button at bottom
                card.spawn(Node {
                    width: Val::Percent(100.0),
                    padding: UiRect::all(Val::Px(16.0)),
                    justify_content: JustifyContent::Center,
                    ..default()
                })
                .with_children(|btn_section| {
                    btn_section
                        .spawn((
                            Button,
                            Node {
                                width: Val::Percent(100.0),
                                height: Val::Px(50.0),
                                justify_content: JustifyContent::Center,
                                align_items: AlignItems::Center,
                                ..default()
                            },
                            BackgroundColor(colors::BUTTON_GOLD),
                            BorderRadius::all(Val::Px(6.0)),
                            NextButton,
                        ))
                        .with_children(|btn| {
                            btn.spawn((
                                Text::new("Next"),
                                TextFont {
                                    font_size: 20.0,
                                    ..default()
                                },
                                TextColor(colors::TEXT_PRIMARY),
                            ));
                        });
                });
            });
    } else {
        // Unselected card: Smaller with portrait and name only
        parent
            .spawn((
                Button,
                Node {
                    width: Val::Px(200.0),
                    height: Val::Px(280.0),
                    flex_direction: FlexDirection::Column,
                    justify_content: JustifyContent::FlexStart,
                    align_items: AlignItems::Center,
                    padding: UiRect::all(Val::Px(0.0)),
                    ..default()
                },
                BackgroundColor(Color::srgba(0.08, 0.1, 0.13, 0.8)), // Dimmed
                BorderRadius::all(Val::Px(12.0)),
                Outline {
                    width: Val::Px(0.0),
                    offset: Val::Px(0.0),
                    color: Color::srgb(0.2, 0.7, 0.2),
                },
                CharacterCard { kind: character },
            ))
            .with_children(|card| {
                // Character portrait
                card.spawn((
                    ImageNode::new(image_handle),
                    Node {
                        width: Val::Px(200.0),
                        height: Val::Px(200.0),
                        ..default()
                    },
                ));

                // Name section
                card.spawn(Node {
                    width: Val::Percent(100.0),
                    padding: UiRect::all(Val::Px(12.0)),
                    justify_content: JustifyContent::Center,
                    ..default()
                })
                .with_children(|name_section| {
                    name_section.spawn((
                        Text::new(character.display_name()),
                        TextFont {
                            font_size: 18.0,
                            ..default()
                        },
                        TextColor(colors::TEXT_SECONDARY),
                    ));
                });
            });
    }
}

/// Handle character card selection - despawn and respawn entire UI atomically
pub fn handle_character_selection(
    mut interaction_query: Query<
        (&Interaction, &CharacterCard),
        (Changed<Interaction>, With<Button>),
    >,
    mut profile: ResMut<PlayerProfile>,
    mut commands: Commands,
    setup_ui_query: Query<Entity, With<CharacterSetupUI>>,
    image_assets: Res<ImageAssets>,
) {
    for (interaction, card) in &mut interaction_query {
        if *interaction == Interaction::Pressed {
            // Select this character
            profile.character = Some(card.kind);

            // Despawn the entire UI
            for entity in &setup_ui_query {
                commands.entity(entity).try_despawn();
            }

            // Immediately respawn with new selection
            spawn_character_selection_ui_with_profile(
                &mut commands,
                &image_assets,
                Some(card.kind),
            );
        }
    }
}

/// Handle hover effects on character cards
pub fn handle_character_card_hover(
    mut interaction_query: Query<
        (&Interaction, &mut BackgroundColor),
        (Changed<Interaction>, With<CharacterCard>),
    >,
) {
    for (interaction, mut bg_color) in &mut interaction_query {
        match *interaction {
            Interaction::Hovered => {
                *bg_color = BackgroundColor(Color::srgb(0.12, 0.14, 0.17));
            }
            Interaction::None => {
                *bg_color = BackgroundColor(colors::PANEL_BG);
            }
            _ => {}
        }
    }
}

/// Handle Next button click to move to name input step
pub fn handle_next_button(
    mut interaction_query: Query<
        (&Interaction, &mut BackgroundColor),
        (Changed<Interaction>, With<NextButton>),
    >,
    mut step: ResMut<SetupStep>,
    mut commands: Commands,
    setup_ui: Query<Entity, With<CharacterSetupUI>>,
    image_assets: Res<ImageAssets>,
    profile: Res<PlayerProfile>,
) {
    for (interaction, mut bg_color) in &mut interaction_query {
        match *interaction {
            Interaction::Pressed => {
                // Transition to name input step
                *step = SetupStep::NameInput;

                // Despawn current UI
                for entity in &setup_ui {
                    commands.entity(entity).try_despawn();
                }

                // Spawn name input UI
                spawn_name_input_ui(&mut commands, &image_assets, &profile);
            }
            Interaction::Hovered => {
                *bg_color = BackgroundColor(colors::BUTTON_GOLD_HOVER);
            }
            Interaction::None => {
                *bg_color = BackgroundColor(colors::BUTTON_GOLD);
            }
        }
    }
}

/// Spawn the name input UI
fn spawn_name_input_ui(
    commands: &mut Commands,
    image_assets: &ImageAssets,
    profile: &PlayerProfile,
) {
    commands
        .spawn((
            CharacterSetupUI,
            Node {
                width: Val::Percent(100.0),
                height: Val::Percent(100.0),
                position_type: PositionType::Absolute,
                justify_content: JustifyContent::Center,
                align_items: AlignItems::Center,
                ..default()
            },
            BackgroundColor(colors::OVERLAY_BG),
            GlobalZIndex(1000),
        ))
        .with_children(|parent| {
            // Modal panel container with neon green border glow
            parent
                .spawn((
                    Node {
                        width: Val::Px(540.0),
                        flex_direction: FlexDirection::Column,
                        padding: UiRect::all(Val::Px(30.0)),
                        row_gap: Val::Px(20.0),
                        align_items: AlignItems::Center,
                        border: UiRect::all(Val::Px(3.0)),
                        ..default()
                    },
                    BackgroundColor(colors::PANEL_BG),
                    BorderColor::all(colors::MODAL_BORDER_GLOW),
                    BorderRadius::all(Val::Px(12.0)),
                    Outline {
                        width: Val::Px(4.0),
                        offset: Val::Px(2.0),
                        color: Color::srgba(0.0, 0.9, 0.7, 0.3),
                    },
                ))
                .with_children(|modal| {
                    // Title
                    modal.spawn((
                        Text::new("NAME YOUR OPERATOR"),
                        TextFont {
                            font_size: 48.0,
                            ..default()
                        },
                        TextColor(colors::TITLE_GOLD),
                    ));

                    // Subtitle
                    modal.spawn((
                        Text::new(
                            "Your goal is to build and operate a profitable EV charging network.",
                        ),
                        TextFont {
                            font_size: 22.0,
                            ..default()
                        },
                        TextColor(colors::TEXT_PRIMARY),
                        TextLayout::new_with_linebreak(bevy::text::LineBreak::WordBoundary),
                    ));

                    // Avatar + Input row (side by side)
                    modal
                        .spawn(Node {
                            width: Val::Percent(100.0),
                            flex_direction: FlexDirection::Row,
                            align_items: AlignItems::Center,
                            column_gap: Val::Px(20.0),
                            ..default()
                        })
                        .with_children(|row| {
                            // Character avatar (circular with purple ring)
                            if let Some(character) = profile.character {
                                let image_handle = match character {
                                    CharacterKind::Ant => image_assets.character_main_ant.clone(),
                                    CharacterKind::Mallard => {
                                        image_assets.character_main_mallard.clone()
                                    }
                                    CharacterKind::Raccoon => {
                                        image_assets.character_main_raccoon.clone()
                                    }
                                };

                                row.spawn((
                                    ImageNode::new(image_handle),
                                    Node {
                                        width: Val::Px(100.0),
                                        height: Val::Px(100.0),
                                        border: UiRect::all(Val::Px(3.0)),
                                        ..default()
                                    },
                                    BorderColor::all(Color::srgb(0.5, 0.2, 0.7)),
                                    BorderRadius::all(Val::Percent(50.0)),
                                    CharacterPreview,
                                ));
                            }

                            // Text input field (white background, dark text)
                            row.spawn((
                                Node {
                                    flex_grow: 1.0,
                                    height: Val::Px(55.0),
                                    justify_content: JustifyContent::FlexStart,
                                    align_items: AlignItems::Center,
                                    padding: UiRect::axes(Val::Px(15.0), Val::Px(10.0)),
                                    flex_direction: FlexDirection::Row,
                                    ..default()
                                },
                                BackgroundColor(Color::srgb(0.92, 0.92, 0.92)),
                                BorderRadius::all(Val::Px(6.0)),
                            ))
                            .with_children(|field| {
                                let is_default_name = profile.name == "Player";
                                let start_index = if is_default_name {
                                    rand::rng().random_range(0..PLACEHOLDER_NAMES.len())
                                } else {
                                    0
                                };
                                let display_text = if is_default_name {
                                    PLACEHOLDER_NAMES[start_index].to_string()
                                } else {
                                    profile.name.clone()
                                };
                                let text_color = if is_default_name {
                                    colors::TEXT_MUTED
                                } else {
                                    Color::srgb(0.1, 0.4, 0.1)
                                };

                                field.spawn((
                                    Text::new(display_text),
                                    TextFont {
                                        font_size: 24.0,
                                        ..default()
                                    },
                                    TextColor(text_color),
                                    NameInputText,
                                    PlaceholderCycler {
                                        timer: Timer::from_seconds(2.0, TimerMode::Repeating),
                                        index: start_index,
                                        active: is_default_name,
                                    },
                                ));

                                // Blinking cursor
                                field.spawn((
                                    Text::new("|"),
                                    TextFont {
                                        font_size: 24.0,
                                        ..default()
                                    },
                                    TextColor(text_color),
                                    BlinkingCursor::default(),
                                ));
                            });
                        });

                    // Start Mission button
                    modal
                        .spawn((
                            Button,
                            Node {
                                width: Val::Px(200.0),
                                height: Val::Px(50.0),
                                justify_content: JustifyContent::Center,
                                align_items: AlignItems::Center,
                                ..default()
                            },
                            BackgroundColor(if profile.name.is_empty() {
                                colors::BUTTON_DISABLED
                            } else {
                                colors::BUTTON_GOLD
                            }),
                            BorderRadius::all(Val::Px(6.0)),
                            StartButton,
                        ))
                        .with_children(|btn| {
                            btn.spawn((
                                Text::new("START MISSION"),
                                TextFont {
                                    font_size: 18.0,
                                    ..default()
                                },
                                TextColor(if profile.name.is_empty() {
                                    colors::TEXT_MUTED
                                } else {
                                    colors::TEXT_PRIMARY
                                }),
                            ));
                        });
                });
        });
}

/// Handle keyboard input for name entry
pub fn handle_name_input(
    keyboard: Res<ButtonInput<KeyCode>>,
    mut profile: ResMut<PlayerProfile>,
    mut text_query: Query<(&mut Text, &mut TextColor, &mut PlaceholderCycler), With<NameInputText>>,
    mut start_button_query: Query<&mut BackgroundColor, With<StartButton>>,
    mut cursor_color_query: Query<&mut TextColor, (With<BlinkingCursor>, Without<NameInputText>)>,
) {
    // Simple keyboard-based text input using common keys
    let mut changed = false;
    let shift_held = keyboard.pressed(KeyCode::ShiftLeft) || keyboard.pressed(KeyCode::ShiftRight);

    // Handle letter input (A-Z / a-z depending on shift)
    let letter_keys = [
        (KeyCode::KeyA, 'a'),
        (KeyCode::KeyB, 'b'),
        (KeyCode::KeyC, 'c'),
        (KeyCode::KeyD, 'd'),
        (KeyCode::KeyE, 'e'),
        (KeyCode::KeyF, 'f'),
        (KeyCode::KeyG, 'g'),
        (KeyCode::KeyH, 'h'),
        (KeyCode::KeyI, 'i'),
        (KeyCode::KeyJ, 'j'),
        (KeyCode::KeyK, 'k'),
        (KeyCode::KeyL, 'l'),
        (KeyCode::KeyM, 'm'),
        (KeyCode::KeyN, 'n'),
        (KeyCode::KeyO, 'o'),
        (KeyCode::KeyP, 'p'),
        (KeyCode::KeyQ, 'q'),
        (KeyCode::KeyR, 'r'),
        (KeyCode::KeyS, 's'),
        (KeyCode::KeyT, 't'),
        (KeyCode::KeyU, 'u'),
        (KeyCode::KeyV, 'v'),
        (KeyCode::KeyW, 'w'),
        (KeyCode::KeyX, 'x'),
        (KeyCode::KeyY, 'y'),
        (KeyCode::KeyZ, 'z'),
    ];

    for (key, lower) in letter_keys {
        if keyboard.just_pressed(key) && profile.name.len() < 20 {
            let ch = if shift_held {
                lower.to_ascii_uppercase()
            } else {
                lower
            };
            profile.name.push(ch);
            changed = true;
        }
    }

    // Handle digit input (0-9) — shift produces common symbols
    let digit_keys = [
        (KeyCode::Digit0, '0', ')'),
        (KeyCode::Digit1, '1', '!'),
        (KeyCode::Digit2, '2', '@'),
        (KeyCode::Digit3, '3', '#'),
        (KeyCode::Digit4, '4', '$'),
        (KeyCode::Digit5, '5', '%'),
        (KeyCode::Digit6, '6', '^'),
        (KeyCode::Digit7, '7', '&'),
        (KeyCode::Digit8, '8', '*'),
        (KeyCode::Digit9, '9', '('),
    ];

    for (key, normal, shifted) in digit_keys {
        if keyboard.just_pressed(key) && profile.name.len() < 20 {
            let ch = if shift_held { shifted } else { normal };
            profile.name.push(ch);
            changed = true;
        }
    }

    // Handle punctuation and symbol keys
    let symbol_keys: &[(KeyCode, char, char)] = &[
        (KeyCode::Minus, '-', '_'),
        (KeyCode::Equal, '=', '+'),
        (KeyCode::Period, '.', '>'),
        (KeyCode::Comma, ',', '<'),
        (KeyCode::Slash, '/', '?'),
        (KeyCode::Semicolon, ';', ':'),
        (KeyCode::Quote, '\'', '"'),
    ];

    for &(key, normal, shifted) in symbol_keys {
        if keyboard.just_pressed(key) && profile.name.len() < 20 {
            let ch = if shift_held { shifted } else { normal };
            profile.name.push(ch);
            changed = true;
        }
    }

    // Handle space
    if keyboard.just_pressed(KeyCode::Space) && profile.name.len() < 20 {
        profile.name.push(' ');
        changed = true;
    }

    // Handle backspace
    if keyboard.just_pressed(KeyCode::Backspace) {
        profile.name.pop();
        changed = true;
    }

    if !changed {
        return;
    }

    // On first keypress, clear the default "Player" name and deactivate placeholder
    if let Ok((mut text, mut text_color, mut cycler)) = text_query.single_mut() {
        if cycler.active {
            // First real keypress: clear default name, switch to typed input
            cycler.active = false;
            profile.name = profile
                .name
                .chars()
                .last()
                .map_or_else(String::new, String::from);
            let input_green = Color::srgb(0.1, 0.4, 0.1);
            *text_color = TextColor(input_green);

            // Also update cursor color to match
            if let Ok(mut cursor_color) = cursor_color_query.single_mut() {
                *cursor_color = TextColor(input_green);
            }
        }

        // Update text display
        **text = profile.name.clone();
    }

    // Update Start Mission button appearance based on whether name is empty
    if let Ok(mut bg_color) = start_button_query.single_mut() {
        *bg_color = BackgroundColor(if profile.name.is_empty() {
            colors::BUTTON_DISABLED
        } else {
            colors::BUTTON_GOLD
        });
    }
}

/// Animate the blinking cursor
pub fn animate_cursor(
    time: Res<Time>,
    mut cursor_query: Query<(&mut BlinkingCursor, &mut Visibility)>,
) {
    for (mut cursor, mut visibility) in &mut cursor_query {
        cursor.timer.tick(time.delta());
        if cursor.timer.just_finished() {
            *visibility = match *visibility {
                Visibility::Visible => Visibility::Hidden,
                _ => Visibility::Visible,
            };
        }
    }
}

/// Cycle through funny placeholder names in the name input field (random order)
pub fn cycle_placeholder_names(
    time: Res<Time>,
    mut query: Query<(&mut PlaceholderCycler, &mut Text), With<NameInputText>>,
) {
    for (mut cycler, mut text) in &mut query {
        if !cycler.active {
            continue;
        }

        cycler.timer.tick(time.delta());
        if cycler.timer.just_finished() {
            let mut rng = rand::rng();
            let mut next = rng.random_range(0..PLACEHOLDER_NAMES.len());
            if PLACEHOLDER_NAMES.len() > 1 {
                while next == cycler.index {
                    next = rng.random_range(0..PLACEHOLDER_NAMES.len());
                }
            }
            cycler.index = next;
            **text = PLACEHOLDER_NAMES[cycler.index].to_string();
        }
    }
}

/// Handle "START MISSION" button click -- dismiss overlay and start tutorial
pub fn handle_start_button(
    mut interaction_query: Query<
        (&Interaction, &mut BackgroundColor),
        (Changed<Interaction>, With<StartButton>),
    >,
    mut profile: ResMut<PlayerProfile>,
    keyboard: Res<ButtonInput<KeyCode>>,
    mut commands: Commands,
    setup_ui: Query<Entity, With<CharacterSetupUI>>,
    mut tutorial_state: ResMut<TutorialState>,
    cycler_query: Query<&PlaceholderCycler, With<NameInputText>>,
) {
    let finish_setup =
        |profile: &mut ResMut<PlayerProfile>,
         commands: &mut Commands,
         setup_ui: &Query<Entity, With<CharacterSetupUI>>,
         tutorial_state: &mut ResMut<TutorialState>,
         cycler_query: &Query<&PlaceholderCycler, With<NameInputText>>| {
            // If the user never typed anything, adopt the currently displayed placeholder name
            if profile.name == "Player"
                && let Ok(cycler) = cycler_query.single()
            {
                profile.name = PLACEHOLDER_NAMES[cycler.index].to_string();
            }

            info!(
                "Starting game with character {:?} and name '{}'",
                profile.character, profile.name
            );

            // Despawn character setup UI
            for entity in setup_ui {
                commands.entity(entity).try_despawn();
            }

            // Remove SetupStep so character setup systems stop running
            commands.remove_resource::<SetupStep>();

            // Start tutorial if not already completed/skipped
            if !tutorial_state.completed && !tutorial_state.skipped {
                tutorial_state.start();
            }
        };

    // Check for Enter key press
    if keyboard.just_pressed(KeyCode::Enter) && !profile.name.is_empty() {
        finish_setup(
            &mut profile,
            &mut commands,
            &setup_ui,
            &mut tutorial_state,
            &cycler_query,
        );
        return;
    }

    // Handle button click
    for (interaction, mut bg_color) in &mut interaction_query {
        match *interaction {
            Interaction::Pressed => {
                if !profile.name.is_empty() {
                    finish_setup(
                        &mut profile,
                        &mut commands,
                        &setup_ui,
                        &mut tutorial_state,
                        &cycler_query,
                    );
                }
            }
            Interaction::Hovered => {
                if !profile.name.is_empty() {
                    *bg_color = BackgroundColor(colors::BUTTON_GOLD_HOVER);
                }
            }
            Interaction::None => {
                *bg_color = BackgroundColor(if profile.name.is_empty() {
                    colors::BUTTON_DISABLED
                } else {
                    colors::BUTTON_GOLD
                });
            }
        }
    }
}

/// Cleanup character setup UI when exiting the state
pub fn cleanup_character_setup(
    mut commands: Commands,
    setup_ui: Query<Entity, With<CharacterSetupUI>>,
) {
    for entity in &setup_ui {
        commands.entity(entity).try_despawn();
    }
    commands.remove_resource::<SetupStep>();
}
