//! Transformer fire toast notifications -- escalating warnings and fire alerts.
//! Also contains hacker attack/detection toast notifications.
//!
//! - Overload warnings show a "Shed Load" button only when the player has the
//!   Advanced Power Management upgrade. Without it, the toast tells the player
//!   to buy the upgrade (teaching moment).
//! - Fire-started toast announces the fine and firetruck dispatch.

use bevy::prelude::*;

use crate::components::hacker::HackerAttackType;
use crate::events::{
    HackerAttackEvent, HackerDetectedEvent, OverloadSeverity, TransformerFireEvent,
    TransformerOverloadWarningEvent,
};
use crate::resources::{GameClock, ImageAssets, MultiSiteManager};
use crate::ui::toast::{RealTimeToast, ToastContainer, ToastNotification};

#[derive(Component)]
pub struct OverloadWarningToast;

#[derive(Component)]
pub struct FireStartedToast;

#[derive(Component)]
pub struct ShedLoadButton;

#[derive(Component)]
pub struct DismissOverloadButton;

pub fn spawn_overload_warning_toast(
    mut commands: Commands,
    mut events: MessageReader<TransformerOverloadWarningEvent>,
    game_clock: Res<GameClock>,
    time: Res<Time>,
    image_assets: Res<ImageAssets>,
    existing: Query<Entity, With<OverloadWarningToast>>,
    container: Single<Entity, With<ToastContainer>>,
) {
    let container = *container;
    for event in events.read() {
        for entity in &existing {
            commands.entity(entity).try_despawn();
        }

        let real_duration = 8.0;
        let is_critical = event.severity == OverloadSeverity::Critical;

        let (title, message, bg_color) = if is_critical {
            if event.has_power_management {
                (
                    "FIRE IMMINENT",
                    "Reduce load immediately! Shed Load to drop power density to 50%.",
                    Color::srgba(0.85, 0.15, 0.1, 0.95),
                )
            } else {
                (
                    "FIRE IMMINENT",
                    "Add more capacity or reduce load with Power Management!",
                    Color::srgba(0.85, 0.15, 0.1, 0.95),
                )
            }
        } else if event.has_power_management {
            (
                "OVERLOAD WARNING",
                "Transformer overloaded! Shed load to prevent fire.",
                Color::srgba(0.95, 0.55, 0.1, 0.95),
            )
        } else {
            (
                "OVERLOAD WARNING",
                "Transformer overloaded! Buy Advanced Power Management to shed load.",
                Color::srgba(0.95, 0.55, 0.1, 0.95),
            )
        };

        let has_apm = event.has_power_management;

        let toast_entity = commands
            .spawn((
                Node {
                    width: Val::Px(340.0),
                    padding: UiRect::all(Val::Px(14.0)),
                    flex_direction: FlexDirection::Column,
                    row_gap: Val::Px(10.0),
                    ..default()
                },
                BackgroundColor(bg_color),
                BorderRadius::all(Val::Px(8.0)),
                ToastNotification {
                    created_at: game_clock.game_time,
                    duration: 15.0,
                },
                crate::ui::toast::RealTimeToast {
                    created_at_real: time.elapsed_secs(),
                    duration_real: real_duration,
                },
                OverloadWarningToast,
            ))
            .with_children(|parent| {
                parent
                    .spawn(Node {
                        flex_direction: FlexDirection::Row,
                        column_gap: Val::Px(8.0),
                        align_items: AlignItems::Center,
                        ..default()
                    })
                    .with_children(|header| {
                        header.spawn((
                            ImageNode::new(image_assets.icon_warning.clone()),
                            Node {
                                width: Val::Px(20.0),
                                height: Val::Px(20.0),
                                ..default()
                            },
                        ));
                        header.spawn((
                            Text::new(title),
                            TextFont {
                                font_size: 16.0,
                                ..default()
                            },
                            TextColor(Color::WHITE),
                        ));
                    });

                parent.spawn((
                    Text::new(message),
                    TextFont {
                        font_size: 13.0,
                        ..default()
                    },
                    TextColor(Color::srgb(1.0, 0.95, 0.9)),
                ));

                parent
                    .spawn(Node {
                        flex_direction: FlexDirection::Row,
                        column_gap: Val::Px(8.0),
                        margin: UiRect::top(Val::Px(2.0)),
                        ..default()
                    })
                    .with_children(|actions| {
                        if has_apm {
                            actions
                                .spawn((
                                    Button,
                                    Node {
                                        padding: UiRect::axes(Val::Px(12.0), Val::Px(6.0)),
                                        justify_content: JustifyContent::Center,
                                        align_items: AlignItems::Center,
                                        ..default()
                                    },
                                    BackgroundColor(Color::srgb(0.85, 0.25, 0.15)),
                                    BorderRadius::all(Val::Px(4.0)),
                                    ShedLoadButton,
                                ))
                                .with_child((
                                    Text::new("Shed Load"),
                                    TextFont {
                                        font_size: 13.0,
                                        ..default()
                                    },
                                    TextColor(Color::WHITE),
                                ));
                        }

                        actions
                            .spawn((
                                Button,
                                Node {
                                    padding: UiRect::axes(Val::Px(12.0), Val::Px(6.0)),
                                    justify_content: JustifyContent::Center,
                                    align_items: AlignItems::Center,
                                    ..default()
                                },
                                BackgroundColor(Color::srgba(0.0, 0.0, 0.0, 0.25)),
                                BorderRadius::all(Val::Px(4.0)),
                                DismissOverloadButton,
                            ))
                            .with_child((
                                Text::new("Dismiss"),
                                TextFont {
                                    font_size: 13.0,
                                    ..default()
                                },
                                TextColor(Color::WHITE),
                            ));
                    });
            })
            .id();
        commands.entity(container).add_child(toast_entity);
    }
}

pub fn spawn_fire_started_toast(
    mut commands: Commands,
    mut events: MessageReader<TransformerFireEvent>,
    game_clock: Res<GameClock>,
    time: Res<Time>,
    image_assets: Res<ImageAssets>,
    existing_overload: Query<Entity, With<OverloadWarningToast>>,
    existing_fire: Query<Entity, With<FireStartedToast>>,
    container: Single<Entity, With<ToastContainer>>,
) {
    let container = *container;
    for _event in events.read() {
        for entity in &existing_overload {
            commands.entity(entity).try_despawn();
        }
        for entity in &existing_fire {
            commands.entity(entity).try_despawn();
        }

        let real_duration = 10.0;

        let toast_entity = commands
            .spawn((
                Node {
                    width: Val::Px(340.0),
                    padding: UiRect::all(Val::Px(14.0)),
                    flex_direction: FlexDirection::Column,
                    row_gap: Val::Px(8.0),
                    ..default()
                },
                BackgroundColor(Color::srgba(0.75, 0.1, 0.05, 0.95)),
                BorderRadius::all(Val::Px(8.0)),
                ToastNotification {
                    created_at: game_clock.game_time,
                    duration: 15.0,
                },
                crate::ui::toast::RealTimeToast {
                    created_at_real: time.elapsed_secs(),
                    duration_real: real_duration,
                },
                FireStartedToast,
            ))
            .with_children(|parent| {
                parent
                    .spawn(Node {
                        flex_direction: FlexDirection::Row,
                        column_gap: Val::Px(8.0),
                        align_items: AlignItems::Center,
                        ..default()
                    })
                    .with_children(|header| {
                        header.spawn((
                            ImageNode::new(image_assets.icon_fault.clone()),
                            Node {
                                width: Val::Px(20.0),
                                height: Val::Px(20.0),
                                ..default()
                            },
                        ));
                        header.spawn((
                            Text::new("TRANSFORMER FIRE"),
                            TextFont {
                                font_size: 16.0,
                                ..default()
                            },
                            TextColor(Color::WHITE),
                        ));
                    });

                parent.spawn((
                    Text::new(
                        "$5,000 fine for poor planning. Reputation damaged. Firetruck dispatched.",
                    ),
                    TextFont {
                        font_size: 13.0,
                        ..default()
                    },
                    TextColor(Color::srgb(1.0, 0.9, 0.85)),
                ));

                parent.spawn((
                    Text::new("Transformer will be destroyed. Sell for $0 and rebuild."),
                    TextFont {
                        font_size: 12.0,
                        ..default()
                    },
                    TextColor(Color::srgb(1.0, 0.75, 0.7)),
                ));
            })
            .id();
        commands.entity(container).add_child(toast_entity);
    }
}

pub fn handle_shed_load_button(
    mut commands: Commands,
    mut multi_site: ResMut<MultiSiteManager>,
    interactions: Query<&Interaction, (Changed<Interaction>, With<ShedLoadButton>)>,
    mut toast_query: Query<Entity, With<OverloadWarningToast>>,
) {
    for interaction in &interactions {
        if *interaction != Interaction::Pressed {
            continue;
        }

        if let Some(site_state) = multi_site.active_site_mut() {
            site_state.service_strategy.target_power_density = 0.5;
            info!("Power density reduced to 50% via overload Shed Load button");
        }

        for toast_entity in &mut toast_query {
            commands.entity(toast_entity).try_despawn();
        }
    }
}

pub fn handle_dismiss_overload_button(
    mut commands: Commands,
    interactions: Query<&Interaction, (Changed<Interaction>, With<DismissOverloadButton>)>,
    mut toast_query: Query<Entity, With<OverloadWarningToast>>,
) {
    for interaction in &interactions {
        if *interaction != Interaction::Pressed {
            continue;
        }
        for toast_entity in &mut toast_query {
            commands.entity(toast_entity).try_despawn();
        }
    }
}

pub fn update_fire_toast_button_styles(
    mut buttons: Query<
        (
            &Interaction,
            Option<&ShedLoadButton>,
            Option<&DismissOverloadButton>,
            &mut BackgroundColor,
        ),
        Changed<Interaction>,
    >,
) {
    for (interaction, is_shed, is_dismiss, mut bg) in &mut buttons {
        if is_shed.is_none() && is_dismiss.is_none() {
            continue;
        }
        let base = if is_shed.is_some() {
            Color::srgb(0.85, 0.25, 0.15)
        } else {
            Color::srgba(0.0, 0.0, 0.0, 0.25)
        };

        *bg = match *interaction {
            Interaction::None => BackgroundColor(base),
            Interaction::Hovered => BackgroundColor(Color::srgba(1.0, 1.0, 1.0, 0.35)),
            Interaction::Pressed => BackgroundColor(Color::srgb(0.18, 0.18, 0.18)),
        };
    }
}

// ============ Hacker Attack Toasts ============

#[derive(Component)]
pub struct HackerAttackToast;

pub fn spawn_hacker_attack_toast(
    mut commands: Commands,
    mut events: MessageReader<HackerAttackEvent>,
    game_clock: Res<GameClock>,
    time: Res<Time>,
    image_assets: Res<ImageAssets>,
    container: Single<Entity, With<ToastContainer>>,
) {
    let container = *container;
    for event in events.read() {
        let (title, message, bg_color) = match event.attack_type {
            HackerAttackType::TransformerOverload => (
                "CYBER ATTACK",
                "Transformer overload in progress! Chargers forced to max power.",
                Color::srgba(0.85, 0.15, 0.1, 0.95),
            ),
            HackerAttackType::PriceSlash => (
                "HACK DETECTED",
                "Price slashed to $0.01/kWh! Revenue tanking.",
                Color::srgba(0.8, 0.1, 0.7, 0.95),
            ),
        };

        let real_time = time.elapsed_secs();
        let entity = commands
            .spawn((
                Node {
                    width: Val::Px(320.0),
                    padding: UiRect::all(Val::Px(15.0)),
                    flex_direction: FlexDirection::Column,
                    row_gap: Val::Px(6.0),
                    ..default()
                },
                BackgroundColor(bg_color),
                BorderRadius::all(Val::Px(8.0)),
                ToastNotification {
                    created_at: game_clock.total_game_time,
                    duration: 600.0,
                },
                RealTimeToast {
                    created_at_real: real_time,
                    duration_real: 8.0,
                },
                HackerAttackToast,
            ))
            .with_children(|parent| {
                parent.spawn((
                    ImageNode::new(image_assets.vfx_light_pulse_yellow.clone()),
                    Node {
                        width: Val::Px(24.0),
                        height: Val::Px(24.0),
                        ..default()
                    },
                ));
                parent.spawn((
                    Text::new(title),
                    TextFont {
                        font_size: 16.0,
                        ..default()
                    },
                    TextColor(Color::WHITE),
                ));
                parent.spawn((
                    Text::new(message),
                    TextFont {
                        font_size: 12.0,
                        ..default()
                    },
                    TextColor(Color::srgb(1.0, 1.0, 0.9)),
                ));
            })
            .id();

        commands.entity(container).add_child(entity);
    }
}

pub fn spawn_hacker_detected_toast(
    mut commands: Commands,
    mut events: MessageReader<HackerDetectedEvent>,
    game_clock: Res<GameClock>,
    time: Res<Time>,
    image_assets: Res<ImageAssets>,
    container: Single<Entity, With<ToastContainer>>,
) {
    let container = *container;
    for event in events.read() {
        let label = if event.auto_blocked {
            "AUTO-BLOCKED"
        } else {
            "BLOCKED"
        };

        let attack_name = match event.attack_type {
            HackerAttackType::TransformerOverload => "Transformer Overload",
            HackerAttackType::PriceSlash => "Price Slash",
        };

        let message = format!("{label}: {attack_name} attempt neutralized!");
        let bg_color = Color::srgba(0.1, 0.65, 0.3, 0.95);

        let real_time = time.elapsed_secs();
        let entity = commands
            .spawn((
                Node {
                    width: Val::Px(320.0),
                    padding: UiRect::all(Val::Px(15.0)),
                    flex_direction: FlexDirection::Column,
                    row_gap: Val::Px(6.0),
                    ..default()
                },
                BackgroundColor(bg_color),
                BorderRadius::all(Val::Px(8.0)),
                ToastNotification {
                    created_at: game_clock.total_game_time,
                    duration: 600.0,
                },
                RealTimeToast {
                    created_at_real: real_time,
                    duration_real: 6.0,
                },
            ))
            .with_children(|parent| {
                parent.spawn((
                    ImageNode::new(image_assets.vfx_light_pulse_yellow.clone()),
                    Node {
                        width: Val::Px(24.0),
                        height: Val::Px(24.0),
                        ..default()
                    },
                ));
                parent.spawn((
                    Text::new(message),
                    TextFont {
                        font_size: 13.0,
                        ..default()
                    },
                    TextColor(Color::WHITE),
                ));
            })
            .id();

        commands.entity(container).add_child(entity);
    }
}
