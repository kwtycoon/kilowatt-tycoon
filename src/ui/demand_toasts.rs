//! Demand charge toast notifications - player-facing alerts for demand charge management.
//!
//! Design goals:
//! - Single toast per type (no stacking spam)
//! - Focus on *cost impact* and *what player can do*
//! - Use existing `ImageAssets` icons only (no emoji)

use bevy::prelude::*;

use crate::events::DemandBurdenEvent;
use crate::resources::{GameClock, ImageAssets, MultiSiteManager};
use crate::ui::toast::ToastNotification;

// ============ Components ============

/// Single-instance toast: demand charges are becoming a meaningful slice of margin.
#[derive(Component)]
pub struct DemandBurdenToast;

/// Button to reduce power density (quick action)
#[derive(Component)]
pub struct ReduceLoadButton;

/// Button to dismiss warning
#[derive(Component)]
pub struct DismissWarningButton;

/// Marker for toast action buttons (for styling)
#[derive(Component)]
pub struct ToastActionButton;

// ============ Toast Spawning ============

/// Spawn (or replace) the demand burden toast.
pub fn spawn_demand_burden_toast(
    mut commands: Commands,
    mut events: MessageReader<DemandBurdenEvent>,
    game_clock: Res<GameClock>,
    time: Res<Time>,
    image_assets: Res<ImageAssets>,
    existing: Query<Entity, With<DemandBurdenToast>>,
) {
    for event in events.read() {
        // Single-instance: remove existing toast before spawning.
        for entity in &existing {
            commands.entity(entity).try_despawn();
        }

        let real_duration = 10.0; // real seconds
        let share_pct = (event.demand_share * 100.0).round() as i32;

        commands
            .spawn((
                Node {
                    position_type: PositionType::Absolute,
                    top: Val::Px(142.0),
                    right: Val::Px(20.0),
                    width: Val::Px(340.0),
                    padding: UiRect::all(Val::Px(14.0)),
                    flex_direction: FlexDirection::Column,
                    row_gap: Val::Px(10.0),
                    ..default()
                },
                BackgroundColor(Color::srgba(0.95, 0.4, 0.2, 0.95)),
                BorderRadius::all(Val::Px(8.0)),
                ZIndex(9999),
                ToastNotification {
                    created_at: game_clock.game_time,
                    duration: 15.0, // compatibility
                },
                crate::ui::toast::RealTimeToast {
                    created_at_real: time.elapsed_secs(),
                    duration_real: real_duration,
                },
                DemandBurdenToast,
            ))
            .with_children(|parent| {
                // Header
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
                            Text::new("DEMAND CHARGES HIGH"),
                            TextFont {
                                font_size: 16.0,
                                ..default()
                            },
                            TextColor(Color::WHITE),
                        ));
                    });

                // Line 1: money
                parent
                    .spawn(Node {
                        flex_direction: FlexDirection::Row,
                        column_gap: Val::Px(8.0),
                        align_items: AlignItems::Center,
                        ..default()
                    })
                    .with_children(|row| {
                        row.spawn((
                            ImageNode::new(image_assets.icon_cash.clone()),
                            Node {
                                width: Val::Px(18.0),
                                height: Val::Px(18.0),
                                ..default()
                            },
                        ));
                        row.spawn((
                            Text::new(format!(
                                "Projected demand: ${:.0}/day  ({}% of revenue)",
                                event.demand_charge, share_pct
                            )),
                            TextFont {
                                font_size: 14.0,
                                ..default()
                            },
                            TextColor(Color::WHITE),
                        ));
                    });

                // Line 2: context
                parent
                    .spawn(Node {
                        flex_direction: FlexDirection::Row,
                        column_gap: Val::Px(8.0),
                        align_items: AlignItems::Center,
                        ..default()
                    })
                    .with_children(|row| {
                        row.spawn((
                            ImageNode::new(image_assets.icon_power.clone()),
                            Node {
                                width: Val::Px(18.0),
                                height: Val::Px(18.0),
                                ..default()
                            },
                        ));
                        row.spawn((
                            Text::new(format!(
                                "Grid: {:.0} kVA  Peak: {:.0} kW  Rate: ${:.0}/kW",
                                event.grid_kva, event.peak_kw, event.demand_rate
                            )),
                            TextFont {
                                font_size: 12.0,
                                ..default()
                            },
                            TextColor(Color::srgb(1.0, 0.9, 0.85)),
                        ));
                    });

                // Actions
                parent
                    .spawn(Node {
                        flex_direction: FlexDirection::Row,
                        column_gap: Val::Px(8.0),
                        margin: UiRect::top(Val::Px(2.0)),
                        ..default()
                    })
                    .with_children(|actions| {
                        actions
                            .spawn((
                                Button,
                                Node {
                                    padding: UiRect::axes(Val::Px(12.0), Val::Px(6.0)),
                                    justify_content: JustifyContent::Center,
                                    align_items: AlignItems::Center,
                                    ..default()
                                },
                                BackgroundColor(Color::srgb(0.25, 0.55, 0.85)),
                                BorderRadius::all(Val::Px(4.0)),
                                ReduceLoadButton,
                                ToastActionButton,
                            ))
                            .with_child((
                                Text::new("Reduce Power"),
                                TextFont {
                                    font_size: 13.0,
                                    ..default()
                                },
                                TextColor(Color::WHITE),
                            ));

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
                                DismissWarningButton,
                                ToastActionButton,
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
            });
    }
}

/// Keep toast action buttons visible and responsive.
pub fn update_toast_action_button_styles(
    mut buttons: Query<
        (
            &Interaction,
            Option<&ReduceLoadButton>,
            Option<&DismissWarningButton>,
            &mut BackgroundColor,
        ),
        (Changed<Interaction>, With<ToastActionButton>),
    >,
) {
    for (interaction, is_primary, is_secondary, mut bg) in &mut buttons {
        let base = if is_primary.is_some() {
            Color::srgb(0.25, 0.55, 0.85)
        } else if is_secondary.is_some() {
            Color::srgba(0.0, 0.0, 0.0, 0.25)
        } else {
            Color::srgba(1.0, 1.0, 1.0, 0.2)
        };

        *bg = match *interaction {
            Interaction::None => BackgroundColor(base),
            Interaction::Hovered => BackgroundColor(Color::srgba(1.0, 1.0, 1.0, 0.35)),
            Interaction::Pressed => BackgroundColor(Color::srgb(0.18, 0.18, 0.18)),
        };
    }
}

// ============ Button Handlers ============

pub fn handle_reduce_load_button(
    mut commands: Commands,
    mut multi_site: ResMut<MultiSiteManager>,
    interactions: Query<&Interaction, (Changed<Interaction>, With<ReduceLoadButton>)>,
    mut toast_query: Query<Entity, With<DemandBurdenToast>>,
) {
    for interaction in &interactions {
        if *interaction != Interaction::Pressed {
            continue;
        }

        if let Some(site_state) = multi_site.active_site_mut() {
            site_state.service_strategy.target_power_density = 0.8;
            info!("Power density reduced to 80% via demand toast action");
        }

        for toast_entity in &mut toast_query {
            commands.entity(toast_entity).try_despawn();
        }
    }
}

pub fn handle_dismiss_warning_button(
    mut commands: Commands,
    interactions: Query<&Interaction, (Changed<Interaction>, With<DismissWarningButton>)>,
    mut toast_query: Query<Entity, With<DemandBurdenToast>>,
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
