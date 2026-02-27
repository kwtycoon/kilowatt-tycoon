//! Speech bubble UI for driver and technician emotions

use crate::components::driver::Driver;
use crate::components::emotion::{DriverEmotion, EmotionReason, TechnicianEmotion};
use crate::components::technician::Technician;
use crate::resources::GameClock;
use crate::systems::WorldCamera;
use bevy::prelude::*;

/// Marker for speech bubble UI
#[derive(Component)]
pub struct SpeechBubble {
    pub driver_entity: Entity,
    pub spawn_time: f32,
    /// Combined text to display (speech_text + frustration reason if applicable)
    pub display_text: String,
}

/// Marker for the text inside a speech bubble
#[derive(Component)]
pub struct SpeechBubbleText;

/// Colors for speech bubbles
mod colors {
    use bevy::prelude::Color;

    pub const BUBBLE_BG: Color = Color::srgba(1.0, 1.0, 1.0, 0.95);
    pub const BUBBLE_BORDER: Color = Color::srgb(0.3, 0.3, 0.3);
    pub const TEXT: Color = Color::srgb(0.1, 0.1, 0.1);
}

/// Build display text for a driver emotion, including frustration reason if leaving angry
fn build_display_text(emotion: &DriverEmotion) -> Option<String> {
    let speech_text = emotion.speech_text?;

    // If leaving angry and we have a frustration reason, append it
    if emotion.reason == EmotionReason::LeavingAngry
        && let Some(frustration_reason) = emotion.last_frustration_reason
        && let Some(label) = frustration_reason.frustration_label()
    {
        return Some(format!("{speech_text}\n({label})"));
    }

    Some(speech_text.to_string())
}

/// Spawn speech bubbles for drivers with active emotions
pub fn spawn_speech_bubbles(
    mut commands: Commands,
    drivers: Query<(Entity, &DriverEmotion, &GlobalTransform), Changed<DriverEmotion>>,
    mut existing_bubbles: Query<(Entity, &mut SpeechBubble, &Children)>,
    mut text_query: Query<&mut Text, With<SpeechBubbleText>>,
    game_clock: Res<GameClock>,
    camera_query: Query<(&Camera, &GlobalTransform), With<WorldCamera>>,
) {
    let Ok((camera, camera_transform)) = camera_query.single() else {
        return;
    };

    for (driver_entity, emotion, global_transform) in &drivers {
        // Build display text (includes frustration reason if leaving angry)
        let Some(display_text) = build_display_text(emotion) else {
            continue;
        };

        // Check if bubble already exists for this driver
        let mut found_existing = false;
        for (_bubble_entity, mut bubble, children) in &mut existing_bubbles {
            if bubble.driver_entity == driver_entity {
                // Refresh existing bubble
                bubble.spawn_time = game_clock.total_real_time;

                // Update text content if it has changed
                if bubble.display_text != display_text {
                    bubble.display_text = display_text.clone();
                    for child in children.iter() {
                        if let Ok(mut text) = text_query.get_mut(child) {
                            *text = Text::new(display_text.clone());
                        }
                    }
                }

                found_existing = true;
                break;
            }
        }

        if found_existing {
            continue;
        }

        // Get world position above driver (use GlobalTransform for hierarchy-aware position)
        // Y offset = 100.0 to appear above SoC bar (60) and frustration indicator (80)
        let world_pos = global_transform.translation() + Vec3::new(0.0, 100.0, 0.0);

        // Convert to screen position
        let Ok(screen_pos) = camera.world_to_viewport(camera_transform, world_pos) else {
            continue;
        };

        // Spawn speech bubble
        commands
            .spawn((
                Node {
                    position_type: PositionType::Absolute,
                    left: Val::Px(screen_pos.x - 60.0), // Center the bubble
                    top: Val::Px(screen_pos.y - 15.0),
                    width: Val::Px(120.0),
                    height: Val::Auto,
                    padding: UiRect::all(Val::Px(8.0)),
                    justify_content: JustifyContent::Center,
                    align_items: AlignItems::Center,
                    border: UiRect::all(Val::Px(2.0)),
                    ..default()
                },
                BackgroundColor(colors::BUBBLE_BG),
                BorderColor::all(colors::BUBBLE_BORDER),
                SpeechBubble {
                    driver_entity,
                    spawn_time: game_clock.total_real_time,
                    display_text: display_text.clone(),
                },
            ))
            .with_children(|bubble| {
                bubble.spawn((
                    Text::new(display_text),
                    TextFont {
                        font_size: 11.0,
                        ..default()
                    },
                    TextColor(colors::TEXT),
                    SpeechBubbleText,
                ));
            });
    }
}

/// Update speech bubble positions to follow drivers
pub fn update_speech_bubble_positions(
    drivers: Query<(Entity, &GlobalTransform), With<Driver>>,
    mut bubbles: Query<(&SpeechBubble, &mut Node)>,
    camera_query: Query<(&Camera, &GlobalTransform), With<WorldCamera>>,
) {
    let Ok((camera, camera_transform)) = camera_query.single() else {
        return;
    };

    for (bubble, mut node) in &mut bubbles {
        if let Ok((_, driver_global_transform)) = drivers.get(bubble.driver_entity) {
            // Y offset = 100.0 to match spawn offset
            let world_pos = driver_global_transform.translation() + Vec3::new(0.0, 100.0, 0.0);

            if let Ok(screen_pos) = camera.world_to_viewport(camera_transform, world_pos) {
                node.left = Val::Px(screen_pos.x - 60.0);
                node.top = Val::Px(screen_pos.y - 15.0);
            }
        }
    }
}

/// Remove expired speech bubbles
pub fn cleanup_speech_bubbles(
    mut commands: Commands,
    game_clock: Res<GameClock>,
    drivers: Query<(Entity, &DriverEmotion)>,
    bubbles: Query<(Entity, &SpeechBubble)>,
) {
    for (bubble_entity, bubble) in &bubbles {
        // Remove if driver is gone
        let Ok((_, emotion)) = drivers.get(bubble.driver_entity) else {
            commands.entity(bubble_entity).try_despawn();
            continue;
        };

        // Remove if emotion expired
        if emotion.is_expired(game_clock.total_real_time) {
            commands.entity(bubble_entity).try_despawn();
        }
    }
}

// ============ Technician Speech Bubbles ============

/// Marker for technician speech bubble UI
#[derive(Component)]
pub struct TechnicianSpeechBubble {
    pub technician_entity: Entity,
    pub spawn_time: f32,
}

/// Marker for the text inside a technician speech bubble
#[derive(Component)]
pub struct TechnicianSpeechBubbleText;

/// Colors for technician speech bubbles (slightly different tint)
mod tech_colors {
    use bevy::prelude::Color;

    pub const BUBBLE_BG: Color = Color::srgba(0.9, 0.95, 1.0, 0.95); // Slight blue tint
    pub const BUBBLE_BORDER: Color = Color::srgb(0.2, 0.4, 0.6); // Blue border
    pub const TEXT: Color = Color::srgb(0.1, 0.1, 0.2);
}

/// Spawn speech bubbles for technicians with active emotions
pub fn spawn_technician_speech_bubbles(
    mut commands: Commands,
    technicians: Query<(Entity, &TechnicianEmotion, &GlobalTransform), Changed<TechnicianEmotion>>,
    mut existing_bubbles: Query<(Entity, &mut TechnicianSpeechBubble, &Children)>,
    mut text_query: Query<&mut Text, With<TechnicianSpeechBubbleText>>,
    game_clock: Res<GameClock>,
    camera_query: Query<(&Camera, &GlobalTransform), With<WorldCamera>>,
) {
    let Ok((camera, camera_transform)) = camera_query.single() else {
        return;
    };

    for (tech_entity, emotion, global_transform) in &technicians {
        // Skip if no speech text
        let Some(speech_text) = emotion.speech_text else {
            continue;
        };

        // Check if bubble already exists for this technician
        let mut found_existing = false;
        for (_bubble_entity, mut bubble, children) in &mut existing_bubbles {
            if bubble.technician_entity == tech_entity {
                // Refresh existing bubble
                bubble.spawn_time = game_clock.total_real_time;

                // Update text content if it has changed
                for child in children.iter() {
                    if let Ok(mut text) = text_query.get_mut(child)
                        && text.0 != speech_text
                    {
                        *text = Text::new(speech_text);
                    }
                }

                found_existing = true;
                break;
            }
        }

        if found_existing {
            continue;
        }

        // Get world position above technician
        // Y offset = 60.0 to appear above technician sprite
        let world_pos = global_transform.translation() + Vec3::new(0.0, 60.0, 0.0);

        // Convert to screen position
        let Ok(screen_pos) = camera.world_to_viewport(camera_transform, world_pos) else {
            continue;
        };

        // Spawn speech bubble
        commands
            .spawn((
                Node {
                    position_type: PositionType::Absolute,
                    left: Val::Px(screen_pos.x - 60.0),
                    top: Val::Px(screen_pos.y - 15.0),
                    width: Val::Px(120.0),
                    height: Val::Auto,
                    padding: UiRect::all(Val::Px(8.0)),
                    justify_content: JustifyContent::Center,
                    align_items: AlignItems::Center,
                    border: UiRect::all(Val::Px(2.0)),
                    ..default()
                },
                BackgroundColor(tech_colors::BUBBLE_BG),
                BorderColor::all(tech_colors::BUBBLE_BORDER),
                TechnicianSpeechBubble {
                    technician_entity: tech_entity,
                    spawn_time: game_clock.total_real_time,
                },
            ))
            .with_children(|bubble| {
                bubble.spawn((
                    Text::new(speech_text),
                    TextFont {
                        font_size: 11.0,
                        ..default()
                    },
                    TextColor(tech_colors::TEXT),
                    TechnicianSpeechBubbleText,
                ));
            });
    }
}

/// Update technician speech bubble positions to follow technicians
pub fn update_technician_speech_bubble_positions(
    technicians: Query<(Entity, &GlobalTransform), With<Technician>>,
    mut bubbles: Query<(&TechnicianSpeechBubble, &mut Node)>,
    camera_query: Query<(&Camera, &GlobalTransform), With<WorldCamera>>,
) {
    let Ok((camera, camera_transform)) = camera_query.single() else {
        return;
    };

    for (bubble, mut node) in &mut bubbles {
        if let Ok((_, tech_global_transform)) = technicians.get(bubble.technician_entity) {
            // Y offset = 60.0 to match spawn offset
            let world_pos = tech_global_transform.translation() + Vec3::new(0.0, 60.0, 0.0);

            if let Ok(screen_pos) = camera.world_to_viewport(camera_transform, world_pos) {
                node.left = Val::Px(screen_pos.x - 60.0);
                node.top = Val::Px(screen_pos.y - 15.0);
            }
        }
    }
}

/// Remove expired technician speech bubbles
pub fn cleanup_technician_speech_bubbles(
    mut commands: Commands,
    game_clock: Res<GameClock>,
    technicians: Query<(Entity, &TechnicianEmotion)>,
    bubbles: Query<(Entity, &TechnicianSpeechBubble)>,
) {
    for (bubble_entity, bubble) in &bubbles {
        // Remove if technician is gone
        let Ok((_, emotion)) = technicians.get(bubble.technician_entity) else {
            commands.entity(bubble_entity).try_despawn();
            continue;
        };

        // Remove if emotion expired
        if emotion.is_expired(game_clock.total_real_time) {
            commands.entity(bubble_entity).try_despawn();
        }
    }
}
