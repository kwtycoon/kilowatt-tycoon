//! Start Day floating button

use super::colors;
use crate::resources::{BuildState, GameClock, ImageAssets, MultiSiteManager};
use bevy::prelude::*;

// ============ Components ============

#[derive(Component)]
pub struct StartDayButton;

#[derive(Component)]
pub struct StartDayButtonText;

#[derive(Component)]
pub struct StartDayButtonIcon;

#[derive(Component)]
pub struct StartDayPulseTimer {
    pub timer: Timer,
}

// ============ Spawn Functions ============

/// Spawn the Start Day button inline in the top bar (compact version)
pub fn spawn_start_day_button(
    parent: &mut bevy::ecs::hierarchy::ChildSpawnerCommands,
    image_assets: &ImageAssets,
) {
    // Main button container
    parent
        .spawn((
            Button,
            Node {
                padding: UiRect::axes(Val::Px(16.0), Val::Px(6.0)),
                flex_direction: FlexDirection::Row,
                justify_content: JustifyContent::Center,
                align_items: AlignItems::Center,
                column_gap: Val::Px(6.0),
                border: UiRect::all(Val::Px(2.0)),
                ..default()
            },
            BackgroundColor(colors::START_DAY_BRIGHT),
            BorderColor::all(colors::START_DAY_BORDER),
            StartDayButton,
            StartDayPulseTimer {
                timer: Timer::from_seconds(1.5, TimerMode::Repeating),
            },
        ))
        .with_children(|btn| {
            // Icon (play/pause)
            btn.spawn((
                ImageNode::new(image_assets.icon_speed_1x.clone()),
                Node {
                    width: Val::Px(16.0),
                    height: Val::Px(16.0),
                    ..default()
                },
                StartDayButtonIcon,
            ));

            // Button text
            btn.spawn((
                Text::new("START DAY"),
                TextFont {
                    font_size: 16.0,
                    ..default()
                },
                TextColor(Color::WHITE),
                StartDayButtonText,
            ));
        });
}

// ============ Update Systems ============

/// Handle Start Day button clicks
pub fn handle_start_day_button(
    mut build_state: ResMut<BuildState>,
    mut game_clock: ResMut<GameClock>,
    multi_site: Res<MultiSiteManager>,
    mut interaction_query: Query<&Interaction, (Changed<Interaction>, With<StartDayButton>)>,
) {
    // Get active site grid or return early
    let Some(site) = multi_site.active_site() else {
        return;
    };
    let grid = &site.grid;

    for interaction in &mut interaction_query {
        if *interaction == Interaction::Pressed {
            if build_state.is_open {
                // Day is running - toggle pause
                game_clock.toggle_pause();
                info!(
                    "Game {}",
                    if game_clock.is_paused() {
                        "paused"
                    } else {
                        "resumed"
                    }
                );
            } else {
                // Day hasn't started - start it if validation passes
                let validation = grid.validate_for_open();
                if validation.can_open {
                    build_state.is_open = true;
                    info!("Day started!");
                }
            }
        }
    }
}

/// Update Start Day button state and colors
pub fn update_start_day_button(
    build_state: Res<BuildState>,
    game_clock: Res<GameClock>,
    image_assets: Res<ImageAssets>,
    multi_site: Res<MultiSiteManager>,
    mut button_query: Query<(&Interaction, &mut BackgroundColor), With<StartDayButton>>,
    mut button_text_query: Query<&mut Text, With<StartDayButtonText>>,
    mut button_icon_query: Query<&mut ImageNode, With<StartDayButtonIcon>>,
) {
    // Get active site grid or return early
    let Some(site) = multi_site.active_site() else {
        return;
    };
    let grid = &site.grid;

    let validation = grid.validate_for_open();

    // Determine button state
    let (icon, text, base_color) = if !build_state.is_open {
        // Not started yet
        if !validation.can_open {
            // Validation errors
            (
                image_assets.icon_speed_1x.clone(),
                "START DAY",
                colors::START_DAY_ERROR,
            )
        } else {
            // Ready to start
            (
                image_assets.icon_speed_1x.clone(),
                "START DAY",
                colors::START_DAY_BRIGHT,
            )
        }
    } else if game_clock.is_paused() {
        // Day is paused
        (
            image_assets.icon_speed_1x.clone(),
            "PAUSED",
            Color::srgb(0.7, 0.5, 0.1), // Yellow/gold
        )
    } else {
        // Day is running
        (
            image_assets.icon_pause.clone(),
            "RUNNING",
            Color::srgb(0.2, 0.6, 0.7), // Blue/teal
        )
    };

    // Update icon
    for mut image_node in &mut button_icon_query {
        image_node.image = icon.clone();
    }

    // Update text
    for mut text_component in &mut button_text_query {
        **text_component = text.to_string();
    }

    // Update button color based on state and interaction
    for (interaction, mut bg) in &mut button_query {
        if !build_state.is_open && !validation.can_open {
            // Validation error - no hover effect
            *bg = BackgroundColor(base_color);
        } else {
            // Apply hover effect
            match interaction {
                Interaction::Pressed => {
                    *bg = BackgroundColor(base_color);
                }
                Interaction::Hovered => {
                    // Brighten color on hover
                    let hover_color = Color::srgba(
                        base_color.to_srgba().red * 1.1,
                        base_color.to_srgba().green * 1.1,
                        base_color.to_srgba().blue * 1.1,
                        base_color.to_srgba().alpha,
                    );
                    *bg = BackgroundColor(hover_color);
                }
                Interaction::None => {
                    *bg = BackgroundColor(base_color);
                }
            }
        }
    }
}

/// Animate Start Day button with a subtle pulse when ready
pub fn animate_start_day_pulse(
    time: Res<Time>,
    build_state: Res<BuildState>,
    multi_site: Res<MultiSiteManager>,
    mut button_query: Query<
        (&mut StartDayPulseTimer, &mut BorderColor, &Interaction),
        With<StartDayButton>,
    >,
) {
    // Get active site grid or return early
    let Some(site) = multi_site.active_site() else {
        return;
    };
    let grid = &site.grid;

    let validation = grid.validate_for_open();

    // Only pulse when ready and not hovered
    if build_state.is_open || !validation.can_open {
        return;
    }

    for (mut pulse_timer, mut border_color, interaction) in &mut button_query {
        // Don't pulse while hovering (hover effect takes precedence)
        if *interaction != Interaction::None {
            continue;
        }

        pulse_timer.timer.tick(time.delta());

        // Use sine wave for smooth pulsing (0.5 to 1.0 range)
        let progress = pulse_timer.timer.fraction();
        let pulse = 0.5 + 0.5 * (progress * std::f32::consts::TAU).sin();

        // Pulse the border brightness
        let base_color = colors::START_DAY_BORDER;
        let pulsed = Color::srgba(
            base_color.to_srgba().red * (0.7 + 0.3 * pulse),
            base_color.to_srgba().green * (0.7 + 0.3 * pulse),
            base_color.to_srgba().blue * (0.7 + 0.3 * pulse),
            0.4 + 0.4 * pulse, // Pulse alpha from 0.4 to 0.8
        );

        *border_color = BorderColor::all(pulsed);
    }
}
