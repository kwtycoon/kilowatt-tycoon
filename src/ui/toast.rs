//! Toast notification system for game events
//!
//! All toasts are children of a single `ToastContainer` flex column so Bevy's
//! layout engine stacks them automatically with correct spacing regardless of
//! each toast's rendered height.

use bevy::prelude::*;

use crate::events::{AchievementUnlockedEvent, ChargerFaultEvent, RepairFailedEvent};
use crate::resources::{GameClock, ImageAssets};

// ============ Components ============

/// Flex-column container that holds all toast entities. Positioned once in the
/// top-right corner; individual toasts use relative (default) positioning.
#[derive(Component)]
pub struct ToastContainer;

#[derive(Component)]
pub struct ToastNotification {
    pub created_at: f32,
    pub duration: f32,
}

/// Real-time based toast notification (doesn't speed up with game time)
#[derive(Component)]
pub struct RealTimeToast {
    pub created_at_real: f32,
    pub duration_real: f32,
}

#[derive(Component)]
struct ToastText;

/// Marker on fault toasts carrying the charger ID for per-charger deduplication.
#[derive(Component)]
pub struct FaultToast(pub String);

/// Marker on the "SELL NOW" button inside a grid event toast.
#[derive(Component)]
pub struct SellNowButton;

// ============ Constants ============

const TOAST_DURATION_REAL: f32 = 5.0;
const MAX_VISIBLE_TOASTS: usize = 3;

// ============ Container Setup ============

pub fn setup_toast_container(
    mut commands: Commands,
    existing: Query<Entity, With<ToastContainer>>,
) {
    if !existing.is_empty() {
        return;
    }
    commands.spawn((
        ToastContainer,
        Node {
            position_type: PositionType::Absolute,
            top: Val::Px(142.0),
            right: Val::Px(20.0),
            flex_direction: FlexDirection::Column,
            row_gap: Val::Px(8.0),
            align_items: AlignItems::FlexEnd,
            ..default()
        },
        ZIndex(9999),
        GlobalZIndex(9999),
    ));
}

// ============ Spawning Systems ============

/// Spawn toast notifications for charger faults.
///
/// Skips auto-remediated faults (O&M cleared instantly) and deduplicates
/// per charger so only the most recent fault toast per charger is visible.
pub fn spawn_fault_toasts(
    mut commands: Commands,
    mut fault_events: MessageReader<ChargerFaultEvent>,
    game_clock: Res<GameClock>,
    time: Res<Time>,
    image_assets: Res<ImageAssets>,
    existing_fault_toasts: Query<(Entity, &FaultToast)>,
    container: Single<Entity, With<ToastContainer>>,
) {
    let container = *container;
    for event in fault_events.read() {
        if event.auto_remediated {
            continue;
        }

        for (entity, ft) in &existing_fault_toasts {
            if ft.0 == event.charger_id {
                commands.entity(entity).try_despawn();
            }
        }

        let message = format!(
            "Charger {} fault: {}",
            event.charger_id,
            event.fault_type.display_name()
        );

        let toast_entity = spawn_toast(
            &mut commands,
            message,
            game_clock.game_time,
            time.elapsed_secs(),
            image_assets.icon_fault.clone(),
        );
        commands
            .entity(toast_entity)
            .insert(FaultToast(event.charger_id.clone()));
        commands.entity(container).add_child(toast_entity);
    }
}

/// Spawn toast notifications for repair failures
pub fn spawn_repair_failed_toasts(
    mut commands: Commands,
    mut repair_failed_events: MessageReader<RepairFailedEvent>,
    game_clock: Res<GameClock>,
    time: Res<Time>,
    image_assets: Res<ImageAssets>,
    container: Single<Entity, With<ToastContainer>>,
) {
    let container = *container;
    for event in repair_failed_events.read() {
        let message = format!(
            "Repair failed on {}: {}",
            event.charger_id, event.failure_reason
        );

        let entity = spawn_toast_custom(
            &mut commands,
            message,
            game_clock.game_time,
            time.elapsed_secs(),
            image_assets.icon_fault.clone(),
            Color::srgba(0.95, 0.7, 0.2, 0.95),
        );
        commands.entity(container).add_child(entity);
    }
}

const ACHIEVEMENT_TOAST_DURATION_REAL: f32 = 6.0;

/// Spawn a celebratory toast when an achievement is unlocked.
/// Uses real-time duration so it doesn't fly by at 30x game speed.
pub fn spawn_achievement_toasts(
    mut commands: Commands,
    mut unlock_events: MessageReader<AchievementUnlockedEvent>,
    game_clock: Res<GameClock>,
    image_assets: Res<ImageAssets>,
    time: Res<Time>,
    container: Single<Entity, With<ToastContainer>>,
) {
    let container = *container;
    for event in unlock_events.read() {
        let kind = event.kind;

        let tier_color = kind.tier().color();
        let bg_color = {
            let [r, g, b, _] = tier_color.to_srgba().to_f32_array();
            Color::srgba(r * 0.6, g * 0.6, b * 0.6, 0.95)
        };

        let message = format!("Achievement Unlocked: {}\n{}", kind.name(), kind.quote());

        let entity = spawn_achievement_toast(
            &mut commands,
            message,
            game_clock.game_time,
            time.elapsed_secs(),
            image_assets.icon_star_filled.clone(),
            bg_color,
        );
        commands.entity(container).add_child(entity);
    }
}

fn spawn_achievement_toast(
    commands: &mut Commands,
    message: String,
    game_time: f32,
    real_time: f32,
    icon: Handle<Image>,
    bg_color: Color,
) -> Entity {
    commands
        .spawn((
            Node {
                width: Val::Px(340.0),
                padding: UiRect::all(Val::Px(15.0)),
                flex_direction: FlexDirection::Row,
                column_gap: Val::Px(10.0),
                align_items: AlignItems::Center,
                ..default()
            },
            BackgroundColor(bg_color),
            BorderRadius::all(Val::Px(8.0)),
            ToastNotification {
                created_at: game_time,
                duration: ACHIEVEMENT_TOAST_DURATION_REAL * 100.0,
            },
            RealTimeToast {
                created_at_real: real_time,
                duration_real: ACHIEVEMENT_TOAST_DURATION_REAL,
            },
        ))
        .with_children(|parent| {
            parent.spawn((
                ImageNode::new(icon),
                Node {
                    width: Val::Px(28.0),
                    height: Val::Px(28.0),
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
        })
        .id()
}

const GRID_EVENT_TOAST_DURATION_REAL: f32 = 8.0;

/// Spawn a prominent toast when a grid event starts.
/// Shows a fun headline plus import/export multipliers so the player sees opportunity and cost.
/// When `has_power_management` is true, includes a "SELL NOW" action button.
pub fn spawn_grid_event_toast(
    commands: &mut Commands,
    container: Entity,
    event: crate::resources::GridEventType,
    game_time: f32,
    real_time: f32,
    icon: Handle<Image>,
    has_power_management: bool,
) {
    let name = event.name();
    let headline = event.headline();
    let export_mult = event.export_multiplier();
    let import_mult = event.import_multiplier();
    let message =
        format!("{name}: {headline}\nExport {export_mult:.1}x | Import {import_mult:.1}x");

    let bg_color = Color::srgba(0.1, 0.55, 0.85, 0.95);

    let entity = commands
        .spawn((
            Node {
                width: Val::Px(320.0),
                padding: UiRect::all(Val::Px(15.0)),
                flex_direction: FlexDirection::Column,
                row_gap: Val::Px(8.0),
                ..default()
            },
            BackgroundColor(bg_color),
            BorderRadius::all(Val::Px(8.0)),
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
            parent
                .spawn(Node {
                    flex_direction: FlexDirection::Row,
                    column_gap: Val::Px(10.0),
                    align_items: AlignItems::Center,
                    ..default()
                })
                .with_children(|row| {
                    row.spawn((
                        ImageNode::new(icon),
                        Node {
                            width: Val::Px(24.0),
                            height: Val::Px(24.0),
                            ..default()
                        },
                    ));

                    row.spawn((
                        Text::new(message),
                        TextFont {
                            font_size: 14.0,
                            ..default()
                        },
                        TextColor(Color::srgb(1.0, 1.0, 1.0)),
                        ToastText,
                    ));
                });

            if has_power_management {
                parent
                    .spawn((
                        Button,
                        SellNowButton,
                        Node {
                            padding: UiRect::axes(Val::Px(16.0), Val::Px(6.0)),
                            justify_content: JustifyContent::Center,
                            align_items: AlignItems::Center,
                            align_self: AlignSelf::FlexEnd,
                            ..default()
                        },
                        BackgroundColor(Color::srgb(1.0, 0.78, 0.1)),
                        BorderRadius::all(Val::Px(4.0)),
                    ))
                    .with_child((
                        Text::new("SELL NOW"),
                        TextFont {
                            font_size: 13.0,
                            ..default()
                        },
                        TextColor(Color::srgb(0.1, 0.1, 0.1)),
                    ));
            }
        })
        .id();

    commands.entity(container).add_child(entity);
}

/// Spawn a toast summarising a grid event that just ended.
pub fn spawn_grid_event_end_toast(
    commands: &mut Commands,
    container: Entity,
    message: &str,
    game_time: f32,
    real_time: f32,
    icon: Handle<Image>,
) {
    let entity = spawn_toast_custom(
        commands,
        message.to_string(),
        game_time,
        real_time,
        icon,
        Color::srgba(0.15, 0.55, 0.35, 0.95),
    );
    commands.entity(container).add_child(entity);
}

fn spawn_toast(
    commands: &mut Commands,
    message: String,
    game_time: f32,
    real_time: f32,
    icon: Handle<Image>,
) -> Entity {
    spawn_toast_custom(
        commands,
        message,
        game_time,
        real_time,
        icon,
        Color::srgba(0.9, 0.4, 0.2, 0.95),
    )
}

fn spawn_toast_custom(
    commands: &mut Commands,
    message: String,
    game_time: f32,
    real_time: f32,
    icon: Handle<Image>,
    bg_color: Color,
) -> Entity {
    commands
        .spawn((
            Node {
                width: Val::Px(300.0),
                padding: UiRect::all(Val::Px(15.0)),
                flex_direction: FlexDirection::Row,
                column_gap: Val::Px(10.0),
                align_items: AlignItems::Center,
                ..default()
            },
            BackgroundColor(bg_color),
            BorderRadius::all(Val::Px(8.0)),
            ToastNotification {
                created_at: game_time,
                duration: TOAST_DURATION_REAL * 10.0,
            },
            RealTimeToast {
                created_at_real: real_time,
                duration_real: TOAST_DURATION_REAL,
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
        })
        .id()
}

// ============ Update System ============

/// Expire, cap, and fade toasts. Stacking is handled by the flex container.
pub fn update_toasts(
    mut commands: Commands,
    mut toast_query: Query<(
        Entity,
        &ToastNotification,
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

    for (entity, toast, real_time_toast, _, _) in &toast_query {
        let (age, duration) = if let Some(rt_toast) = real_time_toast {
            let age_real = time.elapsed_secs() - rt_toast.created_at_real;
            (age_real, rt_toast.duration_real)
        } else {
            let age_game = game_clock.game_time - toast.created_at;
            (age_game, toast.duration)
        };

        if age >= duration {
            commands.entity(entity).try_despawn();
        } else {
            active_toasts.push((entity, age, duration));
        }
    }

    // Sort by age (oldest first)
    active_toasts.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap());

    // Cap visible toasts -- drop the oldest when over the limit
    while active_toasts.len() > MAX_VISIBLE_TOASTS {
        let (entity, _, _) = active_toasts.remove(0);
        commands.entity(entity).try_despawn();
    }

    // Fade out animation in the last 1 second
    for &(entity, age, duration) in &active_toasts {
        let fade_duration = 1.0;
        let time_remaining = duration - age;
        if time_remaining < fade_duration {
            let alpha = (time_remaining / fade_duration).clamp(0.0, 1.0);

            if let Ok((_, _, _, mut bg_color, children)) = toast_query.get_mut(entity) {
                bg_color.0.set_alpha(alpha * 0.95);

                for child in children.iter() {
                    if let Ok(mut text_color) = text_query.get_mut(child) {
                        text_color.0.set_alpha(alpha);
                    }
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
    interaction_query: Query<
        &Interaction,
        (Changed<Interaction>, With<Button>, With<ToastNotification>),
    >,
) {
    for interaction in &interaction_query {
        if *interaction == Interaction::Pressed {
            for entity in &toast_query {
                commands.entity(entity).try_despawn();
            }
        }
    }
}

/// Handle SELL NOW button presses from grid event toasts.
/// Switches the active site to MaxExport + GridExport for maximum revenue.
/// Updates the button text/color to confirm the action was taken.
pub fn handle_sell_now_button(
    mut multi_site: ResMut<crate::resources::MultiSiteManager>,
    mut interaction_query: Query<
        (&Interaction, &Children, &mut BackgroundColor),
        (Changed<Interaction>, With<SellNowButton>),
    >,
    mut text_query: Query<(&mut Text, &mut TextColor)>,
) {
    for (interaction, children, mut bg) in &mut interaction_query {
        if *interaction == Interaction::Pressed {
            let Some(viewed_id) = multi_site.viewed_site_id else {
                continue;
            };
            let Some(site_state) = multi_site.owned_sites.get_mut(&viewed_id) else {
                continue;
            };
            site_state.service_strategy.solar_export_policy =
                crate::resources::SolarExportPolicy::MaxExport;
            site_state.bess_state.mode = crate::resources::site_energy::BessMode::GridExport;

            *bg = BackgroundColor(Color::srgb(0.2, 0.75, 0.3));
            for child in children.iter() {
                if let Ok((mut text, mut text_color)) = text_query.get_mut(child) {
                    **text = "[OK] SELLING".to_string();
                    *text_color = TextColor(Color::srgb(1.0, 1.0, 1.0));
                }
            }
        }
    }
}
