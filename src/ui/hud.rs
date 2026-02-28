//! HUD (Heads-Up Display) components and systems
//!
//! Provides the unified UI hierarchy for the game, including:
//! - Top bar (cash, revenue, time, reputation, speed controls)
//! - Top navigation bar (panel switcher)
//! - Middle area (strategy sidebar + game view + right toolbar)
//! - Bottom bar (power/utility info)

use bevy::prelude::*;

use crate::resources::{
    AchievementState, CharacterKind, GameClock, GameSpeed, GameState, ImageAssets,
    MultiSiteManager, PlayerProfile,
};
use crate::ui::sidebar::{spawn_sidebar_content, spawn_start_day_button};
use crate::ui::site_tabs::spawn_site_tabs;
use crate::ui::top_nav::spawn_top_nav_content;

// ============ Marker Components ============

#[derive(Component)]
pub struct HudRoot;

#[derive(Component)]
pub struct PlayerNameLabel;

#[derive(Component)]
pub struct PlayerScoreLabel;

#[derive(Component)]
pub struct PlayerAvatarImage;

#[derive(Component)]
pub struct AchievementBadgeButton;

#[derive(Component)]
pub struct AchievementCountLabel;

#[derive(Component)]
pub struct CashLabel;

#[derive(Component)]
pub struct EffectivePriceBadge;

#[derive(Component)]
pub struct PowerTotalLabel;

#[derive(Component)]
pub struct SpotPriceBadge;

#[derive(Component)]
pub struct ReputationLabel;

#[derive(Component)]
pub struct ChargerStatusLabel;

#[derive(Component)]
pub struct TimeLabel;

#[derive(Component)]
pub struct SpeedButton(pub GameSpeed);

#[derive(Component)]
pub struct TicketListContainer;

#[derive(Component)]
pub struct PowerPanelToggleButton;

#[derive(Component)]
pub struct SidebarRoot;

#[derive(Component)]
pub struct MiddleAreaRoot;

#[derive(Component)]
pub struct BottomBarRoot;

// ============ Setup ============

pub fn setup_hud(
    mut commands: Commands,
    image_assets: Res<ImageAssets>,
    player_profile: Res<PlayerProfile>,
    existing_hud: Query<Entity, With<HudRoot>>,
) {
    // Only spawn the HUD once - skip if it already exists (e.g. Day 2+ re-entering Playing)
    if !existing_hud.is_empty() {
        return;
    }

    // Clone handles for use in closures
    let icon_cash = image_assets.icon_cash.clone();
    let icon_power = image_assets.icon_power.clone();
    let icon_reputation = image_assets.icon_reputation.clone();
    let icon_fault = image_assets.icon_fault.clone();
    let icon_medal_gold = image_assets.icon_medal_gold.clone();

    // Get the character avatar
    let avatar_handle = match player_profile.character {
        Some(CharacterKind::Ant) => image_assets.character_main_ant.clone(),
        Some(CharacterKind::Mallard) => image_assets.character_main_mallard.clone(),
        Some(CharacterKind::Raccoon) => image_assets.character_main_raccoon.clone(),
        None => image_assets.character_main_ant.clone(), // fallback
    };

    // Root container - unified hierarchical layout
    commands
        .spawn((
            Node {
                width: Val::Percent(100.0),
                height: Val::Percent(100.0),
                flex_direction: FlexDirection::Column,
                ..default()
            },
            HudRoot,
            Visibility::Hidden, // Hide until template is selected
            Pickable::IGNORE,   // Allow clicks to pass through to game world
        ))
        .with_children(|root| {
            // ============ TOP BAR ============
            root.spawn((
                Node {
                    width: Val::Percent(100.0),
                    height: Val::Px(64.0),
                    padding: UiRect::horizontal(Val::Px(12.0)),
                    column_gap: Val::Px(20.0),
                    align_items: AlignItems::Center,
                    flex_shrink: 0.0,
                    border: UiRect::vertical(Val::Px(2.0)),
                    ..default()
                },
                BackgroundColor(Color::srgb(0.1, 0.4, 0.1)), // Darker green top bar
                BorderColor::all(Color::srgb(0.2, 0.7, 0.2)), // Tycoon green border
            ))
            .with_children(|bar| {
                // ============ PLAYER IDENTITY GROUP ============
                bar.spawn(Node {
                    flex_direction: FlexDirection::Row,
                    column_gap: Val::Px(12.0),
                    align_items: AlignItems::Center,
                    ..default()
                })
                .with_children(|identity| {
                    // Character avatar icon
                    identity.spawn((
                        ImageNode::new(avatar_handle.clone()),
                        Node {
                            width: Val::Px(48.0),
                            height: Val::Px(48.0),
                            border: UiRect::all(Val::Px(2.0)),
                            ..default()
                        },
                        BorderColor::all(Color::srgb(0.2, 0.7, 0.2)),
                        PlayerAvatarImage,
                    ));

                    // Name + Score column
                    identity
                        .spawn(Node {
                            flex_direction: FlexDirection::Column,
                            row_gap: Val::Px(2.0),
                            justify_content: JustifyContent::Center,
                            ..default()
                        })
                        .with_children(|col| {
                            // Player name
                            col.spawn((
                                Text::new(player_profile.name.clone()),
                                TextFont {
                                    font_size: 16.0,
                                    ..default()
                                },
                                TextColor(Color::WHITE),
                                PlayerNameLabel,
                            ));

                            // Score (cash value)
                            col.spawn((
                                Text::new("$0"),
                                TextFont {
                                    font_size: 14.0,
                                    ..default()
                                },
                                TextColor(Color::srgb(1.0, 1.0, 0.4)),
                                PlayerScoreLabel,
                            ));
                        });

                    // Badge count button
                    identity
                        .spawn((
                            Button,
                            Node {
                                flex_direction: FlexDirection::Row,
                                column_gap: Val::Px(6.0),
                                align_items: AlignItems::Center,
                                padding: UiRect::axes(Val::Px(10.0), Val::Px(6.0)),
                                border: UiRect::all(Val::Px(2.0)),
                                ..default()
                            },
                            BackgroundColor(Color::srgb(0.15, 0.45, 0.15)),
                            BorderColor::all(Color::srgb(0.2, 0.7, 0.2)),
                            AchievementBadgeButton,
                        ))
                        .with_children(|btn| {
                            // Medal icon
                            btn.spawn((
                                ImageNode::new(icon_medal_gold.clone()),
                                Node {
                                    width: Val::Px(20.0),
                                    height: Val::Px(20.0),
                                    ..default()
                                },
                            ));
                            // Count text
                            btn.spawn((
                                Text::new("0"),
                                TextFont {
                                    font_size: 16.0,
                                    ..default()
                                },
                                TextColor(Color::WHITE),
                                AchievementCountLabel,
                            ));
                        });
                });

                // Spacer between player identity and stats
                bar.spawn(Node {
                    width: Val::Px(20.0),
                    ..default()
                });
                // 1. Cash
                bar.spawn(Node {
                    flex_direction: FlexDirection::Row,
                    column_gap: Val::Px(6.0),
                    align_items: AlignItems::Center,
                    ..default()
                })
                .with_children(|group| {
                    group.spawn((
                        ImageNode::new(icon_cash.clone()),
                        Node {
                            width: Val::Px(20.0),
                            height: Val::Px(20.0),
                            ..default()
                        },
                    ));
                    group.spawn((
                        Text::new("0"),
                        TextFont {
                            font_size: 18.0,
                            ..default()
                        },
                        TextColor(Color::srgb(1.0, 1.0, 0.4)),
                        CashLabel,
                    ));
                });

                // 2. Power Total
                bar.spawn(Node {
                    flex_direction: FlexDirection::Row,
                    column_gap: Val::Px(6.0),
                    align_items: AlignItems::Center,
                    ..default()
                })
                .with_children(|group| {
                    group.spawn((
                        ImageNode::new(icon_power.clone()),
                        Node {
                            width: Val::Px(20.0),
                            height: Val::Px(20.0),
                            ..default()
                        },
                    ));
                    group.spawn((
                        Text::new("0"),
                        TextFont {
                            font_size: 18.0,
                            ..default()
                        },
                        TextColor(Color::srgb(1.0, 1.0, 0.4)),
                        PowerTotalLabel,
                    ));
                });

                // 2b. Effective Price Badge (visible only with Dynamic Pricing upgrade)
                bar.spawn(Node {
                    flex_direction: FlexDirection::Row,
                    column_gap: Val::Px(4.0),
                    align_items: AlignItems::Center,
                    padding: UiRect::axes(Val::Px(6.0), Val::Px(2.0)),
                    display: Display::None,
                    ..default()
                })
                .with_children(|group| {
                    group.spawn((
                        Text::new("$0.45/kWh"),
                        TextFont {
                            font_size: 14.0,
                            ..default()
                        },
                        TextColor(Color::srgb(0.3, 0.9, 0.3)),
                        EffectivePriceBadge,
                    ));
                });

                // 2c. Spot Price Badge (visible only for level 2+ sites)
                bar.spawn(Node {
                    flex_direction: FlexDirection::Row,
                    column_gap: Val::Px(4.0),
                    align_items: AlignItems::Center,
                    padding: UiRect::axes(Val::Px(6.0), Val::Px(2.0)),
                    display: Display::None,
                    ..default()
                })
                .with_children(|group| {
                    group.spawn((
                        Text::new("SPOT $0.06"),
                        TextFont {
                            font_size: 14.0,
                            ..default()
                        },
                        TextColor(Color::srgb(0.4, 0.8, 1.0)),
                        SpotPriceBadge,
                    ));
                });

                // 3. Reputation
                bar.spawn(Node {
                    flex_direction: FlexDirection::Row,
                    column_gap: Val::Px(6.0),
                    align_items: AlignItems::Center,
                    ..default()
                })
                .with_children(|group| {
                    group.spawn((
                        ImageNode::new(icon_reputation.clone()),
                        Node {
                            width: Val::Px(20.0),
                            height: Val::Px(20.0),
                            ..default()
                        },
                    ));
                    group.spawn((
                        Text::new("50/100"),
                        TextFont {
                            font_size: 18.0,
                            ..default()
                        },
                        TextColor(Color::srgb(1.0, 1.0, 0.4)),
                        ReputationLabel,
                    ));
                });

                // 4. Chargers (Problems / Total)
                bar.spawn(Node {
                    flex_direction: FlexDirection::Row,
                    column_gap: Val::Px(6.0),
                    align_items: AlignItems::Center,
                    ..default()
                })
                .with_children(|group| {
                    group.spawn((
                        ImageNode::new(icon_fault.clone()),
                        Node {
                            width: Val::Px(20.0),
                            height: Val::Px(20.0),
                            ..default()
                        },
                    ));
                    group.spawn((
                        Text::new("0/0"),
                        TextFont {
                            font_size: 18.0,
                            ..default()
                        },
                        TextColor(Color::srgb(1.0, 1.0, 0.4)),
                        ChargerStatusLabel,
                    ));
                });

                // Spacer
                bar.spawn(Node {
                    flex_grow: 1.0,
                    ..default()
                });

                // Right side: Start Day, Date/Time, Speed buttons
                bar.spawn(Node {
                    flex_direction: FlexDirection::Row,
                    column_gap: Val::Px(10.0),
                    align_items: AlignItems::Center,
                    ..default()
                })
                .with_children(|right| {
                    // 1. Start Day button
                    spawn_start_day_button(right, &image_assets);

                    // 2. Date/Time
                    right.spawn((
                        Text::new("Year 1 - Day 1 00:00"),
                        TextFont {
                            font_size: 16.0,
                            ..default()
                        },
                        TextColor(Color::WHITE),
                        TimeLabel,
                    ));

                    // 3. Speed buttons (1x and 10x)
                    right
                        .spawn((
                            Button,
                            Node {
                                padding: UiRect::axes(Val::Px(10.0), Val::Px(4.0)),
                                justify_content: JustifyContent::Center,
                                align_items: AlignItems::Center,
                                ..default()
                            },
                            BackgroundColor(Color::srgb(0.2, 0.3, 0.2)),
                            SpeedButton(GameSpeed::Normal),
                        ))
                        .with_child((
                            Text::new("1x"),
                            TextFont {
                                font_size: 14.0,
                                ..default()
                            },
                            TextColor(Color::WHITE),
                        ));

                    right
                        .spawn((
                            Button,
                            Node {
                                padding: UiRect::axes(Val::Px(10.0), Val::Px(4.0)),
                                justify_content: JustifyContent::Center,
                                align_items: AlignItems::Center,
                                ..default()
                            },
                            BackgroundColor(Color::srgb(0.7, 0.5, 0.1)), // Gold/yellow for active
                            SpeedButton(GameSpeed::Fast),
                        ))
                        .with_child((
                            Text::new("10x"),
                            TextFont {
                                font_size: 14.0,
                                ..default()
                            },
                            TextColor(Color::WHITE),
                        ));
                });
            });

            // ============ TOP NAV (via helper) ============
            spawn_top_nav_content(root, &image_assets);

            // ============ SITE TABS ============
            spawn_site_tabs(root);

            // ============ MIDDLE AREA ============
            root.spawn((
                Node {
                    width: Val::Percent(100.0),
                    flex_grow: 1.0,
                    flex_direction: FlexDirection::Row,
                    ..default()
                },
                MiddleAreaRoot,
            ))
            .with_children(|middle| {
                // Left: Sidebar (strategy + build panels)
                spawn_sidebar_content(middle, &image_assets);

                // Center: Game view spacer (transparent, allows clicking through to game world)
                middle.spawn((
                    Node {
                        flex_grow: 1.0,
                        ..default()
                    },
                    Pickable::IGNORE, // Don't block clicks to the game world
                ));

                // Right: Toolbar (removed power toggle button)
            });

            // ============ FLOATING ELEMENTS ============

            // Tickets panel - right side absolute positioning
            root.spawn((
                Node {
                    position_type: PositionType::Absolute,
                    right: Val::Px(65.0),
                    top: Val::Px(120.0),
                    width: Val::Px(220.0),
                    height: Val::Auto,
                    flex_direction: FlexDirection::Column,
                    padding: UiRect::all(Val::Px(10.0)),
                    row_gap: Val::Px(4.0),
                    ..default()
                },
                BackgroundColor(Color::srgba(0.1, 0.12, 0.15, 0.9)),
                TicketListContainer,
                Visibility::Hidden,
            ))
            .with_children(|list| {
                list.spawn((
                    Text::new("TICKETS"),
                    TextFont {
                        font_size: 16.0,
                        ..default()
                    },
                    TextColor(Color::srgb(0.7, 0.7, 0.7)),
                ));
            });
        });
}

// ============ Update Systems ============

pub fn update_hud(
    game_state: Res<GameState>,
    game_clock: Res<GameClock>,
    multi_site: Res<MultiSiteManager>,
    achievement_state: Res<AchievementState>,
    chargers_q: Query<&crate::components::charger::Charger>,
    mut hud_visibility: Query<&mut Visibility, With<HudRoot>>,
    mut player_score_q: Query<&mut Text, With<PlayerScoreLabel>>,
    mut achievement_count_q: Query<
        &mut Text,
        (With<AchievementCountLabel>, Without<PlayerScoreLabel>),
    >,
    mut cash_q: Query<
        &mut Text,
        (
            With<CashLabel>,
            Without<PlayerScoreLabel>,
            Without<AchievementCountLabel>,
        ),
    >,
    mut power_q: Query<
        &mut Text,
        (
            With<PowerTotalLabel>,
            Without<CashLabel>,
            Without<PlayerScoreLabel>,
            Without<AchievementCountLabel>,
        ),
    >,
    mut rep_q: Query<
        &mut Text,
        (
            With<ReputationLabel>,
            Without<PowerTotalLabel>,
            Without<CashLabel>,
            Without<PlayerScoreLabel>,
            Without<AchievementCountLabel>,
        ),
    >,
    mut charger_status_q: Query<
        &mut Text,
        (
            With<ChargerStatusLabel>,
            Without<ReputationLabel>,
            Without<PowerTotalLabel>,
            Without<CashLabel>,
            Without<PlayerScoreLabel>,
            Without<AchievementCountLabel>,
        ),
    >,
    mut time_q: Query<
        &mut Text,
        (
            With<TimeLabel>,
            Without<ChargerStatusLabel>,
            Without<ReputationLabel>,
            Without<PowerTotalLabel>,
            Without<CashLabel>,
            Without<PlayerScoreLabel>,
            Without<AchievementCountLabel>,
        ),
    >,
) {
    // Ensure HUD is visible
    for mut vis in &mut hud_visibility {
        *vis = Visibility::Inherited;
    }

    // Player Score (formatted with $ and commas, same as cash)
    for mut text in &mut player_score_q {
        **text = format!("${}", format_with_commas(game_state.cash as i64));
    }

    // Achievement Count
    for mut text in &mut achievement_count_q {
        **text = format!("{}", achievement_state.unlocked_count());
    }

    // Cash (formatted with $ and commas)
    for mut text in &mut cash_q {
        **text = format!("${}", format_with_commas(game_state.cash as i64));
    }

    // Power (Total load across all sites)
    let total_load: f32 = multi_site
        .owned_sites
        .values()
        .map(|s| s.phase_loads.total_load())
        .sum();
    for mut text in &mut power_q {
        **text = format!("{total_load:.0} kW");
    }

    // Reputation
    for mut text in &mut rep_q {
        **text = format!("{}/100", game_state.reputation);
    }

    // Chargers (Problems / Total)
    let mut total_chargers = 0;
    let mut problem_chargers = 0;
    for charger in &chargers_q {
        total_chargers += 1;
        if matches!(
            charger.state(),
            crate::components::charger::ChargerState::Warning
                | crate::components::charger::ChargerState::Offline
        ) {
            problem_chargers += 1;
        }
    }
    for mut text in &mut charger_status_q {
        **text = format!("{problem_chargers}/{total_chargers}");
    }

    // Time
    for mut text in &mut time_q {
        **text = format!(
            "{} {}",
            game_clock.formatted_date(),
            game_clock.time_of_day_12h()
        );
    }
}

pub fn handle_speed_buttons(
    mut game_clock: ResMut<GameClock>,
    speed_buttons: Query<(&Interaction, &SpeedButton), Changed<Interaction>>,
) {
    // Speed buttons (1x and 10x)
    for (interaction, speed_btn) in &speed_buttons {
        if *interaction == Interaction::Pressed {
            game_clock.set_speed(speed_btn.0);
            info!("Speed set to {:?}", speed_btn.0);
        }
    }
}

/// Sync speed button colors to reflect current game speed
/// Format a number with comma separators (e.g., 1234567 -> "1,234,567")
fn format_with_commas(n: i64) -> String {
    let is_negative = n < 0;
    let n = n.abs();
    let s = n.to_string();
    let mut result = String::new();
    for (i, c) in s.chars().rev().enumerate() {
        if i > 0 && i % 3 == 0 {
            result.push(',');
        }
        result.push(c);
    }
    let formatted: String = result.chars().rev().collect();
    if is_negative {
        format!("-{formatted}")
    } else {
        formatted
    }
}

pub fn sync_speed_button_colors(
    game_clock: Res<GameClock>,
    mut speed_btns: Query<(&SpeedButton, &mut BackgroundColor)>,
) {
    // Sync speed buttons - highlight the active speed in gold/yellow
    for (btn, mut bg) in &mut speed_btns {
        if game_clock.speed == btn.0 {
            *bg = BackgroundColor(Color::srgb(0.7, 0.5, 0.1)); // Gold/yellow for active
        } else {
            *bg = BackgroundColor(Color::srgb(0.2, 0.3, 0.2)); // Dark green for inactive
        }
    }
}

/// Update the effective price badge in the HUD top bar.
/// Only visible when the Dynamic Pricing Engine upgrade is purchased.
pub fn update_effective_price_badge(
    multi_site: Res<crate::resources::MultiSiteManager>,
    game_clock: Res<GameClock>,
    mut badge_q: Query<(&mut Text, &mut TextColor, &ChildOf), With<EffectivePriceBadge>>,
    mut parent_q: Query<&mut Node, Without<EffectivePriceBadge>>,
) {
    let Some(site) = multi_site.active_site() else {
        return;
    };

    let has_upgrade = site.site_upgrades.has_dynamic_pricing();

    for (mut text, mut text_color, parent) in &mut badge_q {
        // Toggle parent container visibility
        if let Ok(mut parent_node) = parent_q.get_mut(parent.parent()) {
            parent_node.display = if has_upgrade {
                Display::Flex
            } else {
                Display::None
            };
        }

        if !has_upgrade {
            continue;
        }

        let effective = site.service_strategy.pricing.effective_price(
            game_clock.game_time,
            &site.site_energy_config,
            site.charger_utilization,
        );
        let utility_rate = site.site_energy_config.current_rate(game_clock.game_time);
        let margin = effective - utility_rate;

        **text = format!("${effective:.2}/kWh");

        // Color-code by margin health
        *text_color = if margin >= 0.15 {
            TextColor(Color::srgb(0.3, 0.9, 0.3)) // Green - healthy margin
        } else if margin >= 0.0 {
            TextColor(Color::srgb(0.9, 0.9, 0.3)) // Yellow - thin margin
        } else {
            TextColor(Color::srgb(0.9, 0.3, 0.3)) // Red - selling below cost
        };
    }
}

/// Update the wholesale spot price badge in the HUD top bar.
/// Only visible when the viewed site has `challenge_level >= 2`.
pub fn update_spot_price_badge(
    multi_site: Res<crate::resources::MultiSiteManager>,
    mut badge_q: Query<(&mut Text, &mut TextColor, &ChildOf), With<SpotPriceBadge>>,
    mut parent_q: Query<&mut Node, Without<SpotPriceBadge>>,
) {
    let Some(site) = multi_site.active_site() else {
        return;
    };

    let show = site.challenge_level >= 2;

    for (mut text, mut text_color, parent) in &mut badge_q {
        if let Ok(mut parent_node) = parent_q.get_mut(parent.parent()) {
            parent_node.display = if show { Display::Flex } else { Display::None };
        }

        if !show {
            continue;
        }

        let price = site.spot_market.current_price_per_kwh;

        // Show grid event name when active, otherwise "SPOT"
        let label = if let Some(ref event) = site.spot_market.grid_event {
            event.name
        } else {
            "SPOT"
        };

        **text = format!("{label} ${price:.2}");

        // Color-code: green = low, yellow = moderate, red = spike
        *text_color = if price >= 0.50 {
            TextColor(Color::srgb(1.0, 0.2, 0.2)) // Red - price spike
        } else if price >= 0.15 {
            TextColor(Color::srgb(1.0, 0.8, 0.2)) // Yellow - elevated
        } else {
            TextColor(Color::srgb(0.4, 0.8, 1.0)) // Blue - normal
        };
    }
}

/// Handle achievement badge button clicks to open the achievement modal
pub fn handle_achievement_badge_click(
    mut achievement_modal_state: ResMut<crate::ui::AchievementModalState>,
    interaction_query: Query<&Interaction, (Changed<Interaction>, With<AchievementBadgeButton>)>,
) {
    for interaction in &interaction_query {
        if *interaction == Interaction::Pressed {
            achievement_modal_state.toggle();
        }
    }
}

/// Keep the HUD player-name label in sync with the profile (e.g. after
/// the character-setup screen assigns a placeholder name on start).
pub fn sync_player_name_label(
    profile: Res<PlayerProfile>,
    mut name_q: Query<&mut Text, With<PlayerNameLabel>>,
) {
    if !profile.is_changed() {
        return;
    }
    for mut text in &mut name_q {
        **text = profile.name.clone();
    }
}

/// Keep the HUD avatar image in sync with the selected character.
pub fn sync_player_avatar_image(
    profile: Res<PlayerProfile>,
    image_assets: Res<ImageAssets>,
    mut avatar_q: Query<&mut ImageNode, With<PlayerAvatarImage>>,
) {
    if !profile.is_changed() {
        return;
    }
    let handle = match profile.character {
        Some(CharacterKind::Ant) => image_assets.character_main_ant.clone(),
        Some(CharacterKind::Mallard) => image_assets.character_main_mallard.clone(),
        Some(CharacterKind::Raccoon) => image_assets.character_main_raccoon.clone(),
        None => return,
    };
    for mut image_node in &mut avatar_q {
        image_node.image = handle.clone();
    }
}
