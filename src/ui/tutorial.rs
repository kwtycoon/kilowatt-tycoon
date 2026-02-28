//! Tutorial walkthrough overlay system

use bevy::prelude::*;
use bevy::window::PrimaryWindow;

use crate::helpers::ui_builders::colors;
use crate::resources::{
    BuildState, CharacterKind, ImageAssets, MultiSiteManager, PlayerProfile, TutorialState,
    TutorialStep,
};

// ============ Marker Components ============

#[derive(Component)]
pub struct TutorialOverlayRoot;

#[derive(Component)]
pub struct TutorialOverlayPanel;

#[derive(Component)]
pub struct TutorialTitle;

#[derive(Component)]
pub struct TutorialDescription;

#[derive(Component)]
pub struct TutorialStepIndicator;

#[derive(Component)]
pub struct TutorialNextButton;

#[derive(Component)]
pub struct TutorialSkipButton;

#[derive(Component)]
pub struct TutorialDoneButton;

/// Component for highlighting UI elements during tutorial
#[derive(Component)]
pub struct TutorialHighlight {
    pub timer: Timer,
}

impl Default for TutorialHighlight {
    fn default() -> Self {
        Self {
            timer: Timer::from_seconds(1.0, TimerMode::Repeating),
        }
    }
}

// ============ Pointer Components ============

#[derive(Component)]
pub struct TutorialPointerRoot;

#[derive(Component)]
pub struct TutorialPointerText;

#[derive(Component)]
pub struct TutorialPointerTitle;

#[derive(Component)]
pub struct TutorialPointerDescription;

#[derive(Component)]
pub struct TutorialPointerArrow;

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum ArrowDirection {
    Down,
    Up,
}

// ============ Setup ============

/// Start the tutorial if this is the player's first time playing
pub fn start_tutorial_on_first_play(mut tutorial_state: ResMut<TutorialState>) {
    // Only start tutorial if not already completed or skipped
    // This runs on OnEnter(Playing), so on first play it will start the tutorial
    if !tutorial_state.completed && !tutorial_state.skipped {
        tutorial_state.start();
        info!("Tutorial: Starting on first play");
    }
}

pub fn setup_tutorial(
    mut commands: Commands,
    existing: Query<Entity, With<TutorialPointerRoot>>,
    player_profile: Res<PlayerProfile>,
    image_assets: Res<ImageAssets>,
) {
    if !existing.is_empty() {
        return;
    }

    // Format welcome message with player name
    let welcome_text = if !player_profile.name.is_empty() {
        format!("WELCOME, {}!", player_profile.name.to_uppercase())
    } else {
        "WELCOME!".to_string()
    };

    // Get character avatar handle
    let avatar_handle = match player_profile.character {
        Some(CharacterKind::Ant) => Some(image_assets.character_main_ant.clone()),
        Some(CharacterKind::Mallard) => Some(image_assets.character_main_mallard.clone()),
        Some(CharacterKind::Raccoon) => Some(image_assets.character_main_raccoon.clone()),
        None => None,
    };

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
            BackgroundColor(colors::OVERLAY_BG),
            Visibility::Hidden,
            TutorialOverlayRoot,
            ZIndex(10000), // Above everything else
        ))
        .with_children(|overlay| {
            // Tutorial Panel -- green neon border glow
            overlay
                .spawn((
                    Node {
                        width: Val::Px(540.0),
                        flex_direction: FlexDirection::Column,
                        align_items: AlignItems::Center,
                        padding: UiRect::all(Val::Px(32.0)),
                        row_gap: Val::Px(20.0),
                        border: UiRect::all(Val::Px(3.0)),
                        ..default()
                    },
                    BackgroundColor(Color::srgb(0.12, 0.14, 0.17)),
                    BorderColor::all(colors::MODAL_BORDER_GLOW),
                    BorderRadius::all(Val::Px(12.0)),
                    Outline {
                        width: Val::Px(4.0),
                        offset: Val::Px(2.0),
                        color: Color::srgba(0.0, 0.9, 0.7, 0.3),
                    },
                    TutorialOverlayPanel,
                ))
                .with_children(|panel| {
                    // Title (gold, large)
                    panel.spawn((
                        Text::new(welcome_text),
                        TextFont {
                            font_size: 36.0,
                            ..default()
                        },
                        TextColor(colors::TITLE_GOLD),
                        TutorialTitle,
                    ));

                    // Description
                    panel.spawn((
                        Text::new("Your goal is build a profitable EV charging network. Let's get started!"),
                        TextFont {
                            font_size: 20.0,
                            ..default()
                        },
                        TextColor(colors::TEXT_PRIMARY),
                        Node {
                            max_width: Val::Px(460.0),
                            ..default()
                        },
                        TextLayout::new_with_linebreak(bevy::text::LineBreak::WordBoundary),
                        TutorialDescription,
                    ));

                    // Character avatar (circular with purple ring)
                    if let Some(handle) = avatar_handle {
                        panel.spawn((
                            ImageNode::new(handle),
                            Node {
                                width: Val::Px(100.0),
                                height: Val::Px(100.0),
                                border: UiRect::all(Val::Px(3.0)),
                                ..default()
                            },
                            BorderColor::all(Color::srgb(0.5, 0.2, 0.7)),
                            BorderRadius::all(Val::Percent(50.0)),
                        ));
                    }

                    // Step indicator dots
                    panel
                        .spawn((
                            Node {
                                column_gap: Val::Px(8.0),
                                margin: UiRect::vertical(Val::Px(4.0)),
                                ..default()
                            },
                            TutorialStepIndicator,
                        ))
                        .with_children(|dots| {
                            for _ in 0..TutorialStep::total_steps() {
                                dots.spawn((
                                    Node {
                                        width: Val::Px(8.0),
                                        height: Val::Px(8.0),
                                        border: UiRect::all(Val::Px(1.0)),
                                        ..default()
                                    },
                                    BackgroundColor(Color::srgba(0.4, 0.4, 0.4, 0.5)),
                                    BorderColor::all(Color::srgb(0.6, 0.6, 0.6)),
                                    BorderRadius::all(Val::Px(4.0)),
                                ));
                            }
                        });

                    // Button container
                    panel
                        .spawn(Node {
                            column_gap: Val::Px(16.0),
                            margin: UiRect::top(Val::Px(16.0)),
                            ..default()
                        })
                        .with_children(|btns| {
                            // Skip button
                            btns.spawn((
                                Button,
                                Node {
                                    padding: UiRect::axes(Val::Px(24.0), Val::Px(12.0)),
                                    border: UiRect::all(Val::Px(1.0)),
                                    ..default()
                                },
                                BackgroundColor(Color::srgba(0.3, 0.3, 0.35, 0.8)),
                                BorderColor::all(Color::srgb(0.45, 0.47, 0.5)),
                                BorderRadius::all(Val::Px(6.0)),
                                TutorialSkipButton,
                            ))
                            .with_child((
                                Text::new("SKIP TUTORIAL"),
                                TextFont {
                                    font_size: 16.0,
                                    ..default()
                                },
                                TextColor(Color::srgb(0.9, 0.9, 0.9)),
                            ));

                            // Next button (gold)
                            btns.spawn((
                                Button,
                                Node {
                                    padding: UiRect::axes(Val::Px(32.0), Val::Px(12.0)),
                                    ..default()
                                },
                                BackgroundColor(colors::BUTTON_GOLD),
                                BorderRadius::all(Val::Px(6.0)),
                                TutorialNextButton,
                            ))
                            .with_child((
                                Text::new("NEXT"),
                                TextFont {
                                    font_size: 16.0,
                                    ..default()
                                },
                                TextColor(Color::WHITE),
                            ));

                            // Done button (gold, initially hidden)
                            btns.spawn((
                                Button,
                                Node {
                                    padding: UiRect::axes(Val::Px(32.0), Val::Px(12.0)),
                                    ..default()
                                },
                                BackgroundColor(colors::BUTTON_GOLD),
                                BorderRadius::all(Val::Px(6.0)),
                                Visibility::Hidden,
                                TutorialDoneButton,
                            ))
                            .with_child((
                                Text::new("GOT IT!"),
                                TextFont {
                                    font_size: 16.0,
                                    ..default()
                                },
                                TextColor(Color::WHITE),
                            ));
                        });
                });
        });

    // Pointer UI (for interactive steps) - initially hidden
    commands
        .spawn((
            Node {
                position_type: PositionType::Absolute,
                left: Val::Px(0.0),
                top: Val::Px(0.0),
                flex_direction: FlexDirection::Column,
                align_items: AlignItems::Center,
                row_gap: Val::Px(0.0), // No gap, arrow touches box
                ..default()
            },
            Visibility::Hidden,
            TutorialPointerRoot,
            ZIndex(10000),
        ))
        .with_children(|parent| {
            // Large prominent text box
            parent
                .spawn((
                    Node {
                        padding: UiRect::all(Val::Px(24.0)), // More padding
                        width: Val::Px(450.0),               // Wider box
                        border: UiRect::all(Val::Px(2.0)),
                        flex_direction: FlexDirection::Column,
                        row_gap: Val::Px(12.0),
                        ..default()
                    },
                    BackgroundColor(Color::srgba(0.12, 0.14, 0.17, 0.95)),
                    BorderColor::all(colors::MODAL_BORDER_GLOW),
                    TutorialPointerText,
                ))
                .with_children(|text_box| {
                    // Title
                    text_box.spawn((
                        Text::new(""),
                        TextFont {
                            font_size: 20.0, // Larger title
                            ..default()
                        },
                        TextColor(colors::MODAL_BORDER_GLOW),
                        TutorialPointerTitle,
                    ));

                    // Description
                    text_box.spawn((
                        Text::new(""),
                        TextFont {
                            font_size: 16.0,
                            ..default()
                        },
                        TextColor(Color::WHITE),
                        TutorialPointerDescription,
                    ));

                    // Skip button (absolute top-right, doesn't affect layout)
                    text_box
                        .spawn((
                            Button,
                            Node {
                                position_type: PositionType::Absolute,
                                top: Val::Px(4.0),
                                right: Val::Px(4.0),
                                padding: UiRect::axes(Val::Px(8.0), Val::Px(2.0)),
                                ..default()
                            },
                            BackgroundColor(Color::srgba(0.3, 0.3, 0.35, 0.8)),
                            BorderRadius::all(Val::Px(4.0)),
                            TutorialSkipButton,
                        ))
                        .with_child((
                            Text::new("SKIP"),
                            TextFont {
                                font_size: 11.0,
                                ..default()
                            },
                            TextColor(Color::srgb(0.7, 0.7, 0.7)),
                        ));
                });

            // Larger arrow (CSS triangle pointing down)
            parent.spawn((
                Node {
                    width: Val::Px(0.0),
                    height: Val::Px(0.0),
                    border: UiRect {
                        left: Val::Px(25.0), // Wider triangle
                        right: Val::Px(25.0),
                        top: Val::Px(30.0), // Taller triangle
                        bottom: Val::Px(0.0),
                    },
                    ..default()
                },
                BorderColor {
                    left: Color::NONE,
                    right: Color::NONE,
                    top: colors::MODAL_BORDER_GLOW,
                    bottom: Color::NONE,
                },
                TutorialPointerArrow,
            ));
        });
}

// ============ Update Systems ============

/// Update tutorial overlay visibility and content based on current step
pub fn update_tutorial_visibility(
    tutorial_state: Res<TutorialState>,
    player_profile: Res<PlayerProfile>,
    mut overlay_q: Query<&mut Visibility, With<TutorialOverlayRoot>>,
    mut pointer_q: Query<
        &mut Visibility,
        (With<TutorialPointerRoot>, Without<TutorialOverlayRoot>),
    >,
    mut title_q: Query<&mut Text, With<TutorialTitle>>,
    mut description_q: Query<&mut Text, (With<TutorialDescription>, Without<TutorialTitle>)>,
    mut next_btn_q: Query<
        &mut Visibility,
        (
            With<TutorialNextButton>,
            Without<TutorialOverlayRoot>,
            Without<TutorialDoneButton>,
            Without<TutorialPointerRoot>,
        ),
    >,
    mut done_btn_q: Query<
        &mut Visibility,
        (
            With<TutorialDoneButton>,
            Without<TutorialOverlayRoot>,
            Without<TutorialNextButton>,
            Without<TutorialPointerRoot>,
            Without<TutorialSkipButton>,
        ),
    >,
    mut skip_btn_q: Query<
        &mut Visibility,
        (
            With<TutorialSkipButton>,
            Without<TutorialOverlayRoot>,
            Without<TutorialNextButton>,
            Without<TutorialPointerRoot>,
            Without<TutorialDoneButton>,
        ),
    >,
    step_indicator_q: Query<&Children, With<TutorialStepIndicator>>,
    mut dot_q: Query<
        &mut BackgroundColor,
        (Without<TutorialOverlayRoot>, Without<TutorialPointerText>),
    >,
) {
    let Some(current_step) = tutorial_state.current_step else {
        // Hide both modal and pointer when tutorial is inactive
        for mut vis in &mut overlay_q {
            *vis = Visibility::Hidden;
        }
        for mut vis in &mut pointer_q {
            *vis = Visibility::Hidden;
        }
        return;
    };

    // Show modal OR pointer based on step type
    for mut overlay_vis in &mut overlay_q {
        *overlay_vis = if current_step.shows_modal() {
            Visibility::Inherited
        } else {
            Visibility::Hidden
        };
    }

    for mut pointer_vis in &mut pointer_q {
        *pointer_vis = if current_step.shows_pointer() {
            Visibility::Inherited
        } else {
            Visibility::Hidden
        };
    }

    // Only update modal content if showing modal
    if !current_step.shows_modal() {
        return;
    };

    // Update title (personalize welcome message with player name)
    for mut text in &mut title_q {
        **text = if matches!(current_step, TutorialStep::Welcome) && !player_profile.name.is_empty()
        {
            format!("Welcome, {}!", player_profile.name)
        } else {
            current_step.title().to_string()
        };
    }

    // Update description
    for mut text in &mut description_q {
        **text = current_step.description().to_string();
    }

    // Update button visibility
    let show_next = current_step.has_next_button();
    let is_last_step = matches!(current_step, TutorialStep::SwitchSite);

    for mut btn_vis in &mut next_btn_q {
        *btn_vis = if show_next {
            Visibility::Inherited
        } else {
            Visibility::Hidden
        };
    }

    for mut btn_vis in &mut done_btn_q {
        *btn_vis = if is_last_step {
            Visibility::Inherited
        } else {
            Visibility::Hidden
        };
    }

    // Hide skip button on the last step
    for mut btn_vis in &mut skip_btn_q {
        *btn_vis = if is_last_step {
            Visibility::Hidden
        } else {
            Visibility::Inherited
        };
    }

    // Update step indicator dots
    for children in &step_indicator_q {
        let current_index = current_step.index();
        let mut i = 0;
        for child_entity in children.iter() {
            if let Ok(mut bg_color) = dot_q.get_mut(child_entity) {
                if i == current_index {
                    *bg_color = BackgroundColor(colors::MODAL_BORDER_GLOW);
                } else if i < current_index {
                    *bg_color = BackgroundColor(Color::srgb(0.2, 0.6, 0.3));
                } else {
                    *bg_color = BackgroundColor(Color::srgba(0.4, 0.4, 0.4, 0.5));
                }
                i += 1;
            }
        }
    }
}

/// Position tutorial pointer near target UI elements
pub fn position_tutorial_pointer(
    tutorial_state: Res<TutorialState>,
    mut pointer_root_q: Query<&mut Node, With<TutorialPointerRoot>>,
    mut pointer_title_q: Query<
        &mut Text,
        (
            With<TutorialPointerTitle>,
            Without<TutorialPointerDescription>,
            Without<TutorialTitle>,
            Without<TutorialDescription>,
        ),
    >,
    mut pointer_desc_q: Query<
        &mut Text,
        (
            With<TutorialPointerDescription>,
            Without<TutorialPointerTitle>,
            Without<TutorialTitle>,
            Without<TutorialDescription>,
        ),
    >,
    // Query real UI positions
    start_day_btn_q: Query<&GlobalTransform, With<crate::ui::sidebar::StartDayButton>>,
    window_q: Query<&Window, With<PrimaryWindow>>,
) {
    let Some(current_step) = tutorial_state.current_step else {
        return;
    };

    if !current_step.shows_pointer() {
        return;
    }

    // Update title
    for mut text in &mut pointer_title_q {
        **text = current_step.title().to_string();
    }

    // Update description
    for mut text in &mut pointer_desc_q {
        **text = current_step.description().to_string();
    }

    // Get window size for coordinate conversion
    let Ok(window) = window_q.single() else {
        return;
    };
    let window_width = window.width();
    let window_height = window.height();

    // Position pointer with offset to account for box width (450px)
    for mut node in &mut pointer_root_q {
        let target_pos = match current_step {
            TutorialStep::PlaceCharger => {
                // Position above a charger spot, moved up one tile (64px)
                (650.0, 116.0) // Was 180, moved up 64px
            }
            TutorialStep::PlaceTransformer => {
                // Position above grass area (left side)
                (350.0, 150.0)
            }
            TutorialStep::StartDay => {
                // Query actual button position for perfect alignment
                if let Ok(transform) = start_day_btn_q.single() {
                    let world_pos = transform.translation();
                    // Convert world coordinates to screen coordinates
                    let screen_x = world_pos.x + window_width / 2.0;
                    let screen_y = window_height / 2.0 - world_pos.y;

                    // Position pointer below button
                    // Offset: -225px to center the 450px box, +70px below button
                    (screen_x - 225.0, screen_y + 70.0)
                } else {
                    // Fallback position
                    (950.0, 90.0)
                }
            }
            TutorialStep::FixCharger => {
                // Position above center charger area
                (650.0, 120.0)
            }
            _ => (0.0, 0.0),
        };

        node.left = Val::Px(target_pos.0);
        node.top = Val::Px(target_pos.1);
    }
}

/// Update arrow direction and flex layout based on tutorial step
pub fn update_tutorial_arrow_direction(
    tutorial_state: Res<TutorialState>,
    mut pointer_root_q: Query<&mut Node, With<TutorialPointerRoot>>,
    mut arrow_q: Query<
        (&mut Node, &mut BorderColor),
        (With<TutorialPointerArrow>, Without<TutorialPointerRoot>),
    >,
) {
    let Some(current_step) = tutorial_state.current_step else {
        return;
    };

    if !current_step.shows_pointer() {
        return;
    }

    let needs_up_arrow = matches!(current_step, TutorialStep::StartDay);

    // Update arrow triangle orientation
    for (mut node, mut border_color) in &mut arrow_q {
        if needs_up_arrow {
            // Arrow points UP (flip the triangle)
            node.border = UiRect {
                left: Val::Px(25.0),
                right: Val::Px(25.0),
                top: Val::Px(0.0),
                bottom: Val::Px(30.0), // Bottom becomes the base
            };
            *border_color = BorderColor {
                left: Color::NONE,
                right: Color::NONE,
                top: Color::NONE,
                bottom: colors::MODAL_BORDER_GLOW,
            };
        } else {
            // Arrow points DOWN (default)
            node.border = UiRect {
                left: Val::Px(25.0),
                right: Val::Px(25.0),
                top: Val::Px(30.0),
                bottom: Val::Px(0.0),
            };
            *border_color = BorderColor {
                left: Color::NONE,
                right: Color::NONE,
                top: colors::MODAL_BORDER_GLOW,
                bottom: Color::NONE,
            };
        }
    }

    // Update flex direction for pointer root (arrow above or below text)
    for mut node in &mut pointer_root_q {
        if needs_up_arrow {
            node.flex_direction = FlexDirection::ColumnReverse; // Arrow first, then text
        } else {
            node.flex_direction = FlexDirection::Column; // Text first, then arrow
        }
    }
}

/// Handle tutorial button clicks
pub fn handle_tutorial_buttons(
    mut tutorial_state: ResMut<TutorialState>,
    keyboard: Res<ButtonInput<KeyCode>>,
    next_btns: Query<&Interaction, (Changed<Interaction>, With<TutorialNextButton>)>,
    skip_btns: Query<&Interaction, (Changed<Interaction>, With<TutorialSkipButton>)>,
    done_btns: Query<&Interaction, (Changed<Interaction>, With<TutorialDoneButton>)>,
) {
    // Handle Next button (click or Enter/Space key)
    let key_advance =
        keyboard.just_pressed(KeyCode::Enter) || keyboard.just_pressed(KeyCode::Space);
    let key_skip = keyboard.just_pressed(KeyCode::Escape);

    for interaction in &next_btns {
        if *interaction == Interaction::Pressed
            && let Some(current_step) = tutorial_state.current_step
            && let Some(next_step) = current_step.next()
        {
            tutorial_state.advance_to(next_step);
            info!("Tutorial: Advanced to {:?}", next_step);
        }
    }

    // Keyboard: Enter/Space advances modal steps, Escape skips any step.
    if tutorial_state.is_active()
        && let Some(current_step) = tutorial_state.current_step
    {
        if current_step.shows_modal() && key_advance {
            if let Some(next_step) = current_step.next() {
                tutorial_state.advance_to(next_step);
                info!("Tutorial: Advanced to {:?} (keyboard)", next_step);
            } else {
                tutorial_state.complete();
                info!("Tutorial: Completed (keyboard)");
            }
        }
        if key_skip {
            tutorial_state.skip();
            info!("Tutorial: Skipped (keyboard)");
        }
    }

    // Handle Skip button (click)
    for interaction in &skip_btns {
        if *interaction == Interaction::Pressed {
            tutorial_state.skip();
            info!("Tutorial: Skipped by user");
        }
    }

    // Handle Done button (click)
    for interaction in &done_btns {
        if *interaction == Interaction::Pressed {
            tutorial_state.complete();
            info!("Tutorial: Completed!");
        }
    }
}

/// Add button hover effects
pub fn update_tutorial_button_colors(
    mut next_btn_q: Query<
        (&Interaction, &mut BackgroundColor),
        (Changed<Interaction>, With<TutorialNextButton>),
    >,
    mut skip_btn_q: Query<
        (&Interaction, &mut BackgroundColor),
        (
            Changed<Interaction>,
            With<TutorialSkipButton>,
            Without<TutorialNextButton>,
        ),
    >,
    mut done_btn_q: Query<
        (&Interaction, &mut BackgroundColor),
        (
            Changed<Interaction>,
            With<TutorialDoneButton>,
            Without<TutorialNextButton>,
            Without<TutorialSkipButton>,
        ),
    >,
) {
    // Next button (gold)
    for (interaction, mut bg_color) in &mut next_btn_q {
        *bg_color = match interaction {
            Interaction::Pressed => BackgroundColor(Color::srgb(0.8, 0.6, 0.05)),
            Interaction::Hovered => BackgroundColor(colors::BUTTON_GOLD_HOVER),
            Interaction::None => BackgroundColor(colors::BUTTON_GOLD),
        };
    }

    // Skip button
    for (interaction, mut bg_color) in &mut skip_btn_q {
        *bg_color = match interaction {
            Interaction::Pressed => BackgroundColor(Color::srgba(0.2, 0.2, 0.25, 0.8)),
            Interaction::Hovered => BackgroundColor(Color::srgba(0.4, 0.4, 0.45, 0.9)),
            Interaction::None => BackgroundColor(Color::srgba(0.3, 0.3, 0.35, 0.8)),
        };
    }

    // Done button (gold)
    for (interaction, mut bg_color) in &mut done_btn_q {
        *bg_color = match interaction {
            Interaction::Pressed => BackgroundColor(Color::srgb(0.8, 0.6, 0.05)),
            Interaction::Hovered => BackgroundColor(colors::BUTTON_GOLD_HOVER),
            Interaction::None => BackgroundColor(colors::BUTTON_GOLD),
        };
    }
}

// ============ Progress Checking ============

/// Check if tutorial steps should auto-advance based on player actions
pub fn check_tutorial_progress(
    mut tutorial_state: ResMut<TutorialState>,
    multi_site: Res<MultiSiteManager>,
    build_state: Res<BuildState>,
    charger_q: Query<&crate::components::Charger>,
    tutorial_fault: Option<Res<TutorialFaultInjected>>,
) {
    if !tutorial_state.is_active() {
        return;
    }

    let Some(current_step) = tutorial_state.current_step else {
        return;
    };

    // Only check auto-advancing steps
    if !current_step.auto_advances() {
        return;
    }

    let should_advance = match current_step {
        TutorialStep::PlaceCharger => {
            // Check if player has placed any charger (check if any Charger components exist)
            !charger_q.is_empty()
        }
        TutorialStep::PlaceTransformer => {
            // Check if player has placed any transformer on the active site
            if let Some(active_site) = multi_site.active_site() {
                !active_site.grid.transformers.is_empty()
            } else {
                false
            }
        }
        TutorialStep::StartDay => {
            // Check if day has started (build mode is open)
            build_state.is_open
        }
        TutorialStep::FixCharger => {
            // Only check the specific charger that received the tutorial fault,
            // rather than requiring ALL chargers to be fault-free (which races
            // with scripted/stochastic fault injection).
            let tutorial_charger_fixed = tutorial_fault
                .as_ref()
                .and_then(|tf| tf.charger_id.as_ref())
                .map(|target_id| {
                    charger_q
                        .iter()
                        .any(|c| c.id == *target_id && c.current_fault.is_none())
                })
                .unwrap_or(false);
            build_state.is_open && tutorial_charger_fixed
        }
        _ => false,
    };

    if should_advance && let Some(next_step) = current_step.next() {
        tutorial_state.advance_to(next_step);
        info!(
            "Tutorial: Auto-advanced from {:?} to {:?}",
            current_step, next_step
        );
    }
}

// ============ Highlight System ============

/// Update pulsing highlight effects on UI elements during tutorial
pub fn update_tutorial_highlights(
    time: Res<Time>,
    mut highlight_q: Query<(&mut TutorialHighlight, &mut BorderColor)>,
) {
    for (mut highlight, mut border_color) in &mut highlight_q {
        highlight.timer.tick(time.delta());

        // Pulse the border color using a sine wave
        let t = highlight.timer.fraction();
        let pulse = (t * std::f32::consts::TAU).sin() * 0.5 + 0.5; // 0.0 to 1.0

        // Interpolate between green and bright green-cyan
        let r = 0.0 + pulse * 0.1;
        let g = 0.7 + pulse * 0.2;
        let b = 0.5 + pulse * 0.2;

        *border_color = BorderColor::all(Color::srgb(r, g, b));
    }
}

/// Manage which UI elements should be highlighted based on current tutorial step
pub fn manage_tutorial_highlights(
    tutorial_state: Res<TutorialState>,
    mut commands: Commands,
    // Query for elements that might need highlights
    start_day_button_q: Query<Entity, With<crate::ui::sidebar::StartDayButton>>,
    site_tabs_q: Query<Entity, With<crate::ui::site_tabs::SiteTabsContainer>>,
    // Query for existing highlights to remove
    existing_highlights_q: Query<Entity, With<TutorialHighlight>>,
) {
    // Remove all existing highlights first
    for entity in &existing_highlights_q {
        commands.entity(entity).try_remove::<TutorialHighlight>();
    }

    // If tutorial is not active, we're done
    if !tutorial_state.is_active() {
        return;
    }

    let Some(current_step) = tutorial_state.current_step else {
        return;
    };

    // Add highlights based on current step
    match current_step {
        TutorialStep::StartDay => {
            // Highlight the start day button
            for entity in &start_day_button_q {
                commands
                    .entity(entity)
                    .try_insert(TutorialHighlight::default());
            }
        }
        TutorialStep::SwitchSite => {
            // Highlight the site tabs
            for entity in &site_tabs_q {
                commands
                    .entity(entity)
                    .try_insert(TutorialHighlight::default());
            }
        }
        _ => {
            // Other steps might highlight build panel buttons, etc.
            // Those can be added as needed
        }
    }
}

/// Inject a tutorial fault after day starts
/// This system runs once to create a fault for the tutorial.
/// Tracks which charger received the fault so advancement only checks that charger.
#[derive(Resource, Default)]
pub struct TutorialFaultInjected {
    pub injected: bool,
    /// The ID of the charger that received the tutorial fault
    pub charger_id: Option<String>,
}

pub fn inject_tutorial_fault(
    mut injected: ResMut<TutorialFaultInjected>,
    tutorial_state: Res<TutorialState>,
    build_state: Res<BuildState>,
    mut charger_q: Query<&mut crate::components::Charger>,
) {
    // Only inject once per tutorial run
    if injected.injected {
        return;
    }

    // Only inject when on the FixCharger step and day is running
    if !matches!(tutorial_state.current_step, Some(TutorialStep::FixCharger)) {
        return;
    }

    if !build_state.is_open {
        return;
    }

    // Find first available charger without a fault
    for mut charger in &mut charger_q {
        if charger.current_fault.is_none() && !charger.is_disabled {
            // Inject a communication error fault (instant fix via remote)
            charger.current_fault = Some(crate::components::FaultType::CommunicationError);
            injected.injected = true;
            injected.charger_id = Some(charger.id.clone());
            info!(
                "Tutorial: Injected communication error fault on charger {}",
                charger.id
            );
            break;
        }
    }
}
