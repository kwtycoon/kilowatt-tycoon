//! Toast notification system for game events

use bevy::prelude::*;

use crate::events::{AchievementUnlockedEvent, ChargerFaultEvent, RepairFailedEvent};
use crate::resources::{GameClock, ImageAssets};

// ============ Components ============

#[derive(Component)]
pub struct ToastNotification {
    /// Time when toast was created (game time)
    pub created_at: f32,
    /// Duration to display in game seconds
    pub duration: f32,
}

/// Real-time based toast notification (doesn't speed up with game time)
#[derive(Component)]
pub struct RealTimeToast {
    /// Time when toast was created (real elapsed time)
    pub created_at_real: f32,
    /// Duration to display in real seconds
    pub duration_real: f32,
}

#[derive(Component)]
struct ToastText;

// ============ Constants ============

const TOAST_DURATION_REAL: f32 = 5.0; // 5 real seconds

// ============ Spawning System ============

/// Spawn toast notifications for charger faults
pub fn spawn_fault_toasts(
    mut commands: Commands,
    mut fault_events: MessageReader<ChargerFaultEvent>,
    game_clock: Res<GameClock>,
    time: Res<Time>,
    image_assets: Res<ImageAssets>,
) {
    for event in fault_events.read() {
        let message = format!(
            "Charger {} fault: {}",
            event.charger_id,
            event.fault_type.display_name()
        );

        spawn_toast(
            &mut commands,
            message,
            game_clock.game_time,
            time.elapsed_secs(),
            image_assets.icon_fault.clone(),
        );
    }
}

/// Spawn toast notifications for repair failures
pub fn spawn_repair_failed_toasts(
    mut commands: Commands,
    mut repair_failed_events: MessageReader<RepairFailedEvent>,
    game_clock: Res<GameClock>,
    time: Res<Time>,
    image_assets: Res<ImageAssets>,
) {
    for event in repair_failed_events.read() {
        let message = format!(
            "Repair failed on {}: {}",
            event.charger_id, event.failure_reason
        );

        spawn_toast_custom(
            &mut commands,
            message,
            game_clock.game_time,
            time.elapsed_secs(),
            image_assets.icon_fault.clone(),
            Color::srgba(0.95, 0.7, 0.2, 0.95), // Warning yellow/orange
        );
    }
}

const ACHIEVEMENT_TOAST_DURATION_REAL: f32 = 6.0; // 6 real seconds

/// Spawn a celebratory toast when an achievement is unlocked.
/// Uses real-time duration so it doesn't fly by at 30x game speed.
pub fn spawn_achievement_toasts(
    mut commands: Commands,
    mut unlock_events: MessageReader<AchievementUnlockedEvent>,
    game_clock: Res<GameClock>,
    image_assets: Res<ImageAssets>,
    time: Res<Time>,
) {
    for event in unlock_events.read() {
        let kind = event.kind;

        // Tier-colored background (darkened, with high alpha)
        let tier_color = kind.tier().color();
        let bg_color = {
            let [r, g, b, _] = tier_color.to_srgba().to_f32_array();
            Color::srgba(r * 0.6, g * 0.6, b * 0.6, 0.95)
        };

        let message = format!("Achievement Unlocked: {}\n{}", kind.name(), kind.quote());

        spawn_achievement_toast(
            &mut commands,
            message,
            game_clock.game_time,
            time.elapsed_secs(),
            image_assets.icon_star_filled.clone(),
            bg_color,
        );
    }
}

fn spawn_achievement_toast(
    commands: &mut Commands,
    message: String,
    game_time: f32,
    real_time: f32,
    icon: Handle<Image>,
    bg_color: Color,
) {
    commands
        .spawn((
            Node {
                position_type: PositionType::Absolute,
                top: Val::Px(142.0),
                right: Val::Px(20.0),
                width: Val::Px(340.0),
                padding: UiRect::all(Val::Px(15.0)),
                flex_direction: FlexDirection::Row,
                column_gap: Val::Px(10.0),
                align_items: AlignItems::Center,
                ..default()
            },
            BackgroundColor(bg_color),
            BorderRadius::all(Val::Px(8.0)),
            ZIndex(9999),
            // Game-time toast (required by update_toasts query)
            ToastNotification {
                created_at: game_time,
                duration: ACHIEVEMENT_TOAST_DURATION_REAL * 100.0, // Large value so game-time path never expires it
            },
            // Real-time toast takes precedence in update_toasts
            RealTimeToast {
                created_at_real: real_time,
                duration_real: ACHIEVEMENT_TOAST_DURATION_REAL,
            },
        ))
        .with_children(|parent| {
            // Star icon
            parent.spawn((
                ImageNode::new(icon),
                Node {
                    width: Val::Px(28.0),
                    height: Val::Px(28.0),
                    ..default()
                },
            ));

            // Message text
            parent.spawn((
                Text::new(message),
                TextFont {
                    font_size: 14.0,
                    ..default()
                },
                TextColor(Color::srgb(1.0, 1.0, 1.0)),
                ToastText,
            ));
        });
}

const GRID_EVENT_TOAST_DURATION_REAL: f32 = 8.0;

/// Spawn a prominent toast when a grid event starts and spot prices spike.
pub fn spawn_grid_event_toast(
    commands: &mut Commands,
    event_name: &str,
    spot_price: f32,
    multiplier: f32,
    game_time: f32,
    real_time: f32,
    icon: Handle<Image>,
) {
    let message = format!(
        "{event_name}!\nSpot price ${spot_price:.2}/kWh ({multiplier:.0}x) — export solar now!"
    );

    let bg_color = Color::srgba(0.1, 0.55, 0.85, 0.95);

    commands
        .spawn((
            Node {
                position_type: PositionType::Absolute,
                top: Val::Px(142.0),
                right: Val::Px(20.0),
                width: Val::Px(320.0),
                padding: UiRect::all(Val::Px(15.0)),
                flex_direction: FlexDirection::Row,
                column_gap: Val::Px(10.0),
                align_items: AlignItems::Center,
                ..default()
            },
            BackgroundColor(bg_color),
            BorderRadius::all(Val::Px(8.0)),
            ZIndex(9999),
            ToastNotification {
                created_at: game_time,
                duration: GRID_EVENT_TOAST_DURATION_REAL * 100.0,
            },
            RealTimeToast {
                created_at_real: real_time,
                duration_real: GRID_EVENT_TOAST_DURATION_REAL,
            },
        ))
        .with_children(|parent| {
            parent.spawn((
                ImageNode::new(icon),
                Node {
                    width: Val::Px(24.0),
                    height: Val::Px(24.0),
                    ..default()
                },
            ));

            parent.spawn((
                Text::new(message),
                TextFont {
                    font_size: 14.0,
                    ..default()
                },
                TextColor(Color::srgb(1.0, 1.0, 1.0)),
                ToastText,
            ));
        });
}

fn spawn_toast(
    commands: &mut Commands,
    message: String,
    game_time: f32,
    real_time: f32,
    icon: Handle<Image>,
) {
    spawn_toast_custom(
        commands,
        message,
        game_time,
        real_time,
        icon,
        Color::srgba(0.9, 0.4, 0.2, 0.95), // Default red
    );
}

fn spawn_toast_custom(
    commands: &mut Commands,
    message: String,
    game_time: f32,
    real_time: f32,
    icon: Handle<Image>,
    bg_color: Color,
) {
    commands
        .spawn((
            Node {
                position_type: PositionType::Absolute,
                top: Val::Px(142.0), // Below header
                right: Val::Px(20.0),
                width: Val::Px(300.0),
                padding: UiRect::all(Val::Px(15.0)),
                flex_direction: FlexDirection::Row,
                column_gap: Val::Px(10.0),
                align_items: AlignItems::Center,
                ..default()
            },
            BackgroundColor(bg_color),
            BorderRadius::all(Val::Px(8.0)),
            ZIndex(9999),
            ToastNotification {
                created_at: game_time,
                duration: TOAST_DURATION_REAL * 10.0, // generous game-time fallback
            },
            RealTimeToast {
                created_at_real: real_time,
                duration_real: TOAST_DURATION_REAL,
            },
        ))
        .with_children(|parent| {
            // Icon
            parent.spawn((
                ImageNode::new(icon),
                Node {
                    width: Val::Px(24.0),
                    height: Val::Px(24.0),
                    ..default()
                },
            ));

            // Message text
            parent.spawn((
                Text::new(message),
                TextFont {
                    font_size: 14.0,
                    ..default()
                },
                TextColor(Color::srgb(1.0, 1.0, 1.0)),
                ToastText,
            ));
        });
}

// ============ Update System ============

/// Update toast positions and despawn expired toasts
pub fn update_toasts(
    mut commands: Commands,
    mut toast_query: Query<(
        Entity,
        &ToastNotification,
        &mut Node,
        Option<&RealTimeToast>,
        &mut BackgroundColor,
        &Children,
    )>,
    mut text_query: Query<&mut TextColor>,
    mut image_query: Query<&mut ImageNode>,
    game_clock: Res<GameClock>,
    time: Res<Time>,
) {
    let mut active_toasts: Vec<(Entity, f32, f32)> = Vec::new();

    // Check for expired toasts and collect active ones
    for (entity, toast, _, real_time_toast, _, _) in &toast_query {
        // Use real time if RealTimeToast component exists, otherwise game time
        let (age, duration) = if let Some(rt_toast) = real_time_toast {
            let age_real = time.elapsed_secs() - rt_toast.created_at_real;
            (age_real, rt_toast.duration_real)
        } else {
            let age_game = game_clock.game_time - toast.created_at;
            (age_game, toast.duration)
        };

        if age >= duration {
            // Despawn expired toast
            commands.entity(entity).try_despawn();
        } else {
            active_toasts.push((entity, age, duration));
        }
    }

    // Sort by age (oldest first)
    active_toasts.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap());

    // Update positions and fade animation (stack vertically)
    for (index, (entity, age, duration)) in active_toasts.iter().enumerate() {
        if let Ok((_, _, mut node, _, mut bg_color, children)) = toast_query.get_mut(*entity) {
            // Calculate vertical offset
            let offset = 142.0 + (index as f32 * 80.0);
            node.top = Val::Px(offset);

            // Fade out animation in the last 1 second
            let fade_duration = 1.0;
            let time_remaining = duration - age;
            if time_remaining < fade_duration {
                // Calculate alpha from 1.0 to 0.0 as time_remaining goes from fade_duration to 0
                let alpha = (time_remaining / fade_duration).clamp(0.0, 1.0);

                // Fade background
                bg_color.0.set_alpha(alpha * 0.95); // Original alpha was 0.95

                // Fade all children (text and images)
                for child in children.iter() {
                    // Fade text
                    if let Ok(mut text_color) = text_query.get_mut(child) {
                        text_color.0.set_alpha(alpha);
                    }
                    // Fade images by modifying their color
                    if let Ok(mut image_node) = image_query.get_mut(child) {
                        image_node.color.set_alpha(alpha);
                    }
                }
            }
        }
    }
}

/// Handle toast clicks to dismiss
pub fn handle_toast_clicks(
    mut commands: Commands,
    toast_query: Query<Entity, With<ToastNotification>>,
    // NOTE: Only dismiss toasts if the toast itself is clicked.
    // We intentionally do NOT dismiss all toasts on any UI button click, because
    // demand toasts have action buttons and global dismiss makes them feel broken.
    interaction_query: Query<
        &Interaction,
        (Changed<Interaction>, With<Button>, With<ToastNotification>),
    >,
) {
    for interaction in &interaction_query {
        if *interaction == Interaction::Pressed {
            // Dismiss all toasts on toast click (simplified)
            for entity in &toast_query {
                commands.entity(entity).try_despawn();
            }
        }
    }
}
