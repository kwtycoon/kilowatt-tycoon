use bevy::prelude::*;

use crate::states::day_end::report::DayEndReport;
use crate::states::day_end::sections::{
    spawn_avatar_section, spawn_badge_section, spawn_header_section, spawn_kpi_area,
    spawn_kpi_header,
};
use crate::states::day_end::{DayEndModalContainer, LinkedInShareButton};
use crate::states::{DayEndContinueButton, DayEndScrollBody, DayEndUI};

/// Spawn the day-end overlay, modal shell, and all content sections.
///
/// Reads the precomputed [`DayEndReport`] resource and delegates to
/// per-section spawner functions so each section is independently testable.
pub(crate) fn spawn_day_end_ui(mut commands: Commands, report: Res<DayEndReport>) {
    commands
        .spawn((
            DayEndUI,
            Node {
                width: Val::Percent(100.0),
                height: Val::Percent(100.0),
                position_type: PositionType::Absolute,
                justify_content: JustifyContent::Center,
                align_items: AlignItems::Center,
                ..default()
            },
            BackgroundColor(Color::srgba(0.0, 0.0, 0.0, 0.7)),
            GlobalZIndex(1000),
        ))
        .with_children(|overlay| {
            overlay
                .spawn((
                    DayEndModalContainer,
                    Node {
                        width: Val::Px(480.0),
                        max_height: Val::Percent(85.0),
                        flex_direction: FlexDirection::Column,
                        ..default()
                    },
                    BackgroundColor(Color::srgb(0.12, 0.14, 0.18)),
                    BorderColor::all(Color::srgb(0.3, 0.35, 0.4)),
                    BorderRadius::all(Val::Px(12.0)),
                ))
                .with_children(|modal_outer| {
                    // Scrollable body
                    modal_outer
                        .spawn((
                            DayEndScrollBody,
                            Node {
                                width: Val::Percent(100.0),
                                flex_direction: FlexDirection::Column,
                                flex_grow: 1.0,
                                overflow: Overflow::scroll_y(),
                                ..default()
                            },
                            ScrollPosition::default(),
                        ))
                        .with_children(|scroll_outer| {
                            scroll_outer
                                .spawn(Node {
                                    width: Val::Percent(100.0),
                                    flex_direction: FlexDirection::Column,
                                    padding: UiRect::all(Val::Px(24.0)),
                                    row_gap: Val::Px(12.0),
                                    ..default()
                                })
                                .with_children(|modal| {
                                    spawn_header_section(modal, &report);
                                    spawn_avatar_section(modal, &report);
                                    spawn_kpi_header(modal);
                                    spawn_kpi_area(modal, &report);
                                    spawn_badge_section(modal, &report);
                                });
                        });

                    // Button footer (pinned outside scroll)
                    spawn_button_footer(modal_outer);
                });
        });
}

fn spawn_button_footer(parent: &mut bevy::ecs::hierarchy::ChildSpawnerCommands) {
    parent
        .spawn((
            Node {
                width: Val::Percent(100.0),
                flex_direction: FlexDirection::Row,
                justify_content: JustifyContent::Center,
                column_gap: Val::Px(12.0),
                padding: UiRect::new(Val::Px(16.0), Val::Px(16.0), Val::Px(12.0), Val::Px(12.0)),
                border: UiRect::top(Val::Px(1.0)),
                ..default()
            },
            BorderColor::all(Color::srgb(0.3, 0.35, 0.4)),
        ))
        .with_children(|button_row| {
            // Share on LinkedIn
            button_row
                .spawn((
                    Button,
                    Node {
                        padding: UiRect::new(
                            Val::Px(16.0),
                            Val::Px(16.0),
                            Val::Px(10.0),
                            Val::Px(10.0),
                        ),
                        justify_content: JustifyContent::Center,
                        align_items: AlignItems::Center,
                        border: UiRect::all(Val::Px(1.0)),
                        ..default()
                    },
                    BackgroundColor(Color::srgb(0.04, 0.4, 0.76)),
                    BorderColor::all(Color::srgb(0.06, 0.5, 0.86)),
                    BorderRadius::all(Val::Px(4.0)),
                    LinkedInShareButton,
                ))
                .with_child((
                    Text::new("Share on LinkedIn"),
                    TextFont {
                        font_size: 14.0,
                        ..default()
                    },
                    TextColor(Color::WHITE),
                ));

            // Continue to Next Day
            button_row
                .spawn((
                    Button,
                    Node {
                        padding: UiRect::new(
                            Val::Px(16.0),
                            Val::Px(16.0),
                            Val::Px(10.0),
                            Val::Px(10.0),
                        ),
                        justify_content: JustifyContent::Center,
                        align_items: AlignItems::Center,
                        border: UiRect::all(Val::Px(2.0)),
                        ..default()
                    },
                    BackgroundColor(Color::srgb(0.2, 0.6, 0.3)),
                    BorderColor::all(Color::srgb(0.25, 0.7, 0.35)),
                    BorderRadius::all(Val::Px(4.0)),
                    DayEndContinueButton,
                ))
                .with_child((
                    Text::new("Continue"),
                    TextFont {
                        font_size: 14.0,
                        ..default()
                    },
                    TextColor(Color::WHITE),
                ));
        });
}
