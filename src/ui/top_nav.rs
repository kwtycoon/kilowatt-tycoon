//! Top navigation bar - Lemonade Tycoon style icon-based panel switcher

use crate::audio::SoundEnabled;
use crate::events::SiteSwitchEvent;
use crate::resources::{
    BuildState, EnvironmentState, ImageAssets, MultiSiteManager, UnitSystem, WeatherType,
};
use crate::ui::sidebar::{NavigationState, PrimaryNav};
use bevy::ecs::hierarchy::ChildSpawnerCommands;
use bevy::prelude::*;
use bevy::ui::BorderColor;

// ============ Marker Components ============

#[derive(Component)]
pub struct TopNavRoot;

#[derive(Component)]
pub struct PrimaryNavButton {
    pub nav: PrimaryNav,
}

#[derive(Component)]
pub struct LeaderboardButton;

#[derive(Component)]
pub struct LedgerButton;

#[derive(Component)]
pub struct UnitToggleButton;

/// Text child inside the unit toggle button so we can update the label.
#[derive(Component)]
pub struct UnitToggleLabel;

#[derive(Component)]
pub struct SoundToggleButton;

/// Text child inside the sound toggle button so we can update the label.
#[derive(Component)]
pub struct SoundToggleLabel;

/// Icon child inside the sound toggle button so we can swap on/off images.
#[derive(Component)]
pub struct SoundToggleIcon;

#[derive(Component)]
pub struct WeatherForecastDisplay;

#[derive(Component)]
pub struct CurrentWeatherIcon;

#[derive(Component)]
pub struct ForecastIcon1;

#[derive(Component)]
pub struct ForecastIcon2;

#[derive(Component)]
pub struct TemperatureLabel;

#[derive(Component)]
pub struct TemperatureHoverArea;

#[derive(Component)]
pub struct TemperatureTooltip;

/// Timer for auto-hiding the temperature tooltip
#[derive(Component)]
pub struct TooltipAutoHideTimer {
    pub timer: Timer,
}

impl Default for TooltipAutoHideTimer {
    fn default() -> Self {
        Self {
            timer: Timer::from_seconds(5.0, TimerMode::Once),
        }
    }
}

#[derive(Component)]
pub struct DemandLabel;

#[derive(Component)]
pub struct NewsTickerText;

// ============ Colors ============

mod colors {
    use bevy::prelude::Color;

    pub const NAV_BG: Color = Color::srgb(0.15, 0.45, 0.15); // Dark green
    pub const TEXT_PRIMARY: Color = Color::WHITE;

    // Pixel-art beveled button colors
    pub const BUTTON_FACE: Color = Color::srgb(0.15, 0.4, 0.15); // Button face
    pub const BUTTON_ACTIVE_FACE: Color = Color::srgb(0.25, 0.65, 0.25); // Active button face
    pub const BUTTON_HIGHLIGHT: Color = Color::srgb(0.3, 0.6, 0.3); // Light edge (top/left)
    pub const BUTTON_SHADOW: Color = Color::srgb(0.05, 0.2, 0.05); // Dark edge (bottom/right)
}

// ============ Setup ============

/// Legacy standalone setup function - now delegates to spawn_top_nav_content
pub fn setup_top_nav(mut commands: Commands) {
    commands.insert_resource(NavigationState::default());
    // This is now handled by the unified HUD hierarchy
    // Keeping for backwards compatibility but the actual spawning is done via spawn_top_nav_content
}

/// Spawn the top navigation bar as a child of a parent container
pub fn spawn_top_nav_content(parent: &mut ChildSpawnerCommands, image_assets: &ImageAssets) {
    parent
        .spawn((
            Node {
                width: Val::Percent(100.0),
                height: Val::Px(50.0),
                flex_direction: FlexDirection::Row,
                align_items: AlignItems::Center,
                padding: UiRect::horizontal(Val::Px(12.0)),
                column_gap: Val::Px(6.0),
                display: Display::Flex,
                flex_shrink: 0.0,
                border: UiRect::bottom(Val::Px(2.0)),
                ..default()
            },
            BackgroundColor(colors::NAV_BG),
            BorderColor::all(Color::srgb(0.1, 0.3, 0.1)),
            TopNavRoot,
        ))
        .with_children(|nav_parent| {
            // Navigation Buttons with icons (Location, Build, Strategy, Stats)
            let nav_items: Vec<(&str, PrimaryNav, Handle<Image>)> = vec![
                (
                    "Location",
                    PrimaryNav::Rent,
                    image_assets.icon_briefcase.clone(),
                ),
                ("Build", PrimaryNav::Build, image_assets.icon_plug.clone()),
                (
                    "Strategy",
                    PrimaryNav::Strategy,
                    image_assets.icon_cash.clone(),
                ),
                (
                    "Stats",
                    PrimaryNav::Stats,
                    image_assets.icon_dashboard.clone(),
                ),
            ];

            for (label, nav, icon) in nav_items {
                nav_parent
                    .spawn((
                        Button,
                        Node {
                            padding: UiRect::new(
                                Val::Px(12.0),
                                Val::Px(12.0),
                                Val::Px(4.0),
                                Val::Px(4.0),
                            ),
                            height: Val::Px(38.0),
                            flex_direction: FlexDirection::Row,
                            column_gap: Val::Px(6.0),
                            justify_content: JustifyContent::Center,
                            align_items: AlignItems::Center,
                            border: UiRect::new(
                                Val::Px(2.0),
                                Val::Px(2.0),
                                Val::Px(2.0),
                                Val::Px(2.0),
                            ),
                            ..default()
                        },
                        // Beveled border: highlight on top/left, shadow on bottom/right
                        BeveledBorder::normal().to_border_color(),
                        BackgroundColor(colors::BUTTON_FACE),
                        PrimaryNavButton { nav },
                        BeveledBorder::normal(),
                    ))
                    .with_children(|btn| {
                        // Icon
                        btn.spawn((
                            ImageNode::new(icon),
                            Node {
                                width: Val::Px(18.0),
                                height: Val::Px(18.0),
                                ..default()
                            },
                        ));
                        // Text
                        btn.spawn((
                            Text::new(label),
                            TextFont {
                                font_size: 13.0,
                                ..default()
                            },
                            TextColor(colors::TEXT_PRIMARY),
                        ));
                    });
            }

            // Leaderboard button (opens modal instead of navigation)
            nav_parent
                .spawn((
                    Button,
                    Node {
                        padding: UiRect::new(
                            Val::Px(12.0),
                            Val::Px(12.0),
                            Val::Px(4.0),
                            Val::Px(4.0),
                        ),
                        height: Val::Px(38.0),
                        flex_direction: FlexDirection::Row,
                        column_gap: Val::Px(6.0),
                        justify_content: JustifyContent::Center,
                        align_items: AlignItems::Center,
                        border: UiRect::new(Val::Px(2.0), Val::Px(2.0), Val::Px(2.0), Val::Px(2.0)),
                        ..default()
                    },
                    BeveledBorder::normal().to_border_color(),
                    BackgroundColor(colors::BUTTON_FACE),
                    LeaderboardButton,
                    BeveledBorder::normal(),
                ))
                .with_children(|btn| {
                    // Icon
                    btn.spawn((
                        ImageNode::new(image_assets.icon_medal_gold.clone()),
                        Node {
                            width: Val::Px(18.0),
                            height: Val::Px(18.0),
                            ..default()
                        },
                    ));
                    // Text
                    btn.spawn((
                        Text::new("Leaderboard"),
                        TextFont {
                            font_size: 13.0,
                            ..default()
                        },
                        TextColor(colors::TEXT_PRIMARY),
                    ));
                });

            // Ledger button (opens modal instead of navigation)
            nav_parent
                .spawn((
                    Button,
                    Node {
                        padding: UiRect::new(
                            Val::Px(12.0),
                            Val::Px(12.0),
                            Val::Px(4.0),
                            Val::Px(4.0),
                        ),
                        height: Val::Px(38.0),
                        flex_direction: FlexDirection::Row,
                        column_gap: Val::Px(6.0),
                        justify_content: JustifyContent::Center,
                        align_items: AlignItems::Center,
                        border: UiRect::new(Val::Px(2.0), Val::Px(2.0), Val::Px(2.0), Val::Px(2.0)),
                        ..default()
                    },
                    BeveledBorder::normal().to_border_color(),
                    BackgroundColor(colors::BUTTON_FACE),
                    LedgerButton,
                    BeveledBorder::normal(),
                ))
                .with_children(|btn| {
                    btn.spawn((
                        ImageNode::new(image_assets.icon_ledger.clone()),
                        Node {
                            width: Val::Px(18.0),
                            height: Val::Px(18.0),
                            ..default()
                        },
                    ));
                    btn.spawn((
                        Text::new("Ledger"),
                        TextFont {
                            font_size: 13.0,
                            ..default()
                        },
                        TextColor(colors::TEXT_PRIMARY),
                    ));
                });

            // Unit toggle button (Imperial / Metric)
            nav_parent
                .spawn((
                    Button,
                    Node {
                        padding: UiRect::new(
                            Val::Px(12.0),
                            Val::Px(12.0),
                            Val::Px(4.0),
                            Val::Px(4.0),
                        ),
                        height: Val::Px(38.0),
                        flex_direction: FlexDirection::Row,
                        column_gap: Val::Px(6.0),
                        justify_content: JustifyContent::Center,
                        align_items: AlignItems::Center,
                        border: UiRect::new(Val::Px(2.0), Val::Px(2.0), Val::Px(2.0), Val::Px(2.0)),
                        ..default()
                    },
                    BeveledBorder::normal().to_border_color(),
                    BackgroundColor(colors::BUTTON_FACE),
                    UnitToggleButton,
                    BeveledBorder::normal(),
                ))
                .with_children(|btn| {
                    btn.spawn((
                        ImageNode::new(image_assets.icon_ruler.clone()),
                        Node {
                            width: Val::Px(18.0),
                            height: Val::Px(18.0),
                            ..default()
                        },
                    ));
                    btn.spawn((
                        Text::new("Imperial"),
                        TextFont {
                            font_size: 13.0,
                            ..default()
                        },
                        TextColor(colors::TEXT_PRIMARY),
                        UnitToggleLabel,
                    ));
                });

            // Sound toggle button (On / Off)
            nav_parent
                .spawn((
                    Button,
                    Node {
                        padding: UiRect::new(
                            Val::Px(12.0),
                            Val::Px(12.0),
                            Val::Px(4.0),
                            Val::Px(4.0),
                        ),
                        height: Val::Px(38.0),
                        flex_direction: FlexDirection::Row,
                        column_gap: Val::Px(6.0),
                        justify_content: JustifyContent::Center,
                        align_items: AlignItems::Center,
                        border: UiRect::new(Val::Px(2.0), Val::Px(2.0), Val::Px(2.0), Val::Px(2.0)),
                        ..default()
                    },
                    BeveledBorder::normal().to_border_color(),
                    BackgroundColor(colors::BUTTON_FACE),
                    SoundToggleButton,
                    BeveledBorder::normal(),
                ))
                .with_children(|btn| {
                    btn.spawn((
                        ImageNode::new(image_assets.icon_sound_on.clone()),
                        Node {
                            width: Val::Px(18.0),
                            height: Val::Px(18.0),
                            ..default()
                        },
                        SoundToggleIcon,
                    ));
                    btn.spawn((
                        Text::new("Sound On"),
                        TextFont {
                            font_size: 13.0,
                            ..default()
                        },
                        TextColor(colors::TEXT_PRIMARY),
                        SoundToggleLabel,
                    ));
                });

            // 2. Weather & News (on the right)
            nav_parent
                .spawn(Node {
                    flex_grow: 1.0,
                    flex_direction: FlexDirection::Row,
                    justify_content: JustifyContent::FlexEnd,
                    column_gap: Val::Px(15.0),
                    align_items: AlignItems::Center,
                    ..default()
                })
                .with_children(|right| {
                    // Weather (interactive for tooltip)
                    right
                        .spawn((
                            Button,
                            Node {
                                flex_direction: FlexDirection::Row,
                                column_gap: Val::Px(4.0),
                                align_items: AlignItems::Center,
                                padding: UiRect::all(Val::Px(4.0)),
                                ..default()
                            },
                            BackgroundColor(Color::NONE),
                            TemperatureHoverArea,
                        ))
                        .with_children(|weather| {
                            weather.spawn((
                                ImageNode::new(image_assets.icon_weather_sunny.clone()),
                                Node {
                                    width: Val::Px(20.0),
                                    height: Val::Px(20.0),
                                    ..default()
                                },
                                CurrentWeatherIcon,
                            ));
                            weather.spawn((
                                Text::new("75 F -"),
                                TextFont {
                                    font_size: 16.0,
                                    ..default()
                                },
                                TextColor(Color::WHITE),
                                TemperatureLabel,
                            ));
                            // Tooltip (hidden by default, auto-shows for cold sites)
                            weather.spawn((
                                Node {
                                    position_type: PositionType::Absolute,
                                    top: Val::Px(30.0),
                                    left: Val::Px(-60.0),
                                    padding: UiRect::all(Val::Px(8.0)),
                                    border: UiRect::all(Val::Px(1.0)),
                                    ..default()
                                },
                                BackgroundColor(Color::srgba(0.1, 0.1, 0.1, 0.95)),
                                BorderColor::all(Color::srgb(0.4, 0.4, 0.4)),
                                Visibility::Hidden,
                                Text::new("Charging power: 100%"),
                                TextFont {
                                    font_size: 13.0,
                                    ..default()
                                },
                                TextColor(Color::WHITE),
                                TemperatureTooltip,
                                TooltipAutoHideTimer::default(),
                            ));
                        });

                    // News
                    right.spawn((
                        Text::new("All systems operational."),
                        TextFont {
                            font_size: 16.0,
                            ..default()
                        },
                        TextColor(Color::WHITE),
                        NewsTickerText,
                    ));
                });
        });
}

// ============ Beveled Border Component ============

/// Component to track the beveled border state for pixel-art buttons
#[derive(Component, Clone, Copy)]
pub struct BeveledBorder {
    pub is_pressed: bool,
}

impl BeveledBorder {
    pub fn normal() -> Self {
        Self { is_pressed: false }
    }

    pub fn pressed() -> Self {
        Self { is_pressed: true }
    }

    /// Convert to BorderColor - creates beveled effect using a single color
    /// Normal: lighter appearance, Pressed: darker appearance
    pub fn to_border_color(&self) -> BorderColor {
        if self.is_pressed {
            // Pressed: dark shadow on all sides (sunken look)
            BorderColor::all(colors::BUTTON_SHADOW)
        } else {
            // Normal: light highlight on all sides (raised look)
            BorderColor::all(colors::BUTTON_HIGHLIGHT)
        }
    }
}

// ============ Update Systems ============

pub fn update_top_nav_visibility(
    _build_state: Res<BuildState>,
    mut top_nav: Query<&mut Node, With<TopNavRoot>>,
) {
    // Always show the top nav when it exists (HudRoot controls overall visibility)
    // This ensures weather/news info is visible during building phase
    for mut node in &mut top_nav {
        node.display = Display::Flex;
    }
}

pub fn handle_primary_nav_clicks(
    mut nav_state: ResMut<NavigationState>,
    mut interaction_query: Query<(&Interaction, &PrimaryNavButton), Changed<Interaction>>,
) {
    for (interaction, nav_btn) in &mut interaction_query {
        if *interaction == Interaction::Pressed {
            nav_state.set_primary(nav_btn.nav);
            info!("Switched to primary nav: {:?}", nav_btn.nav);
        }
    }
}

/// Handle leaderboard button clicks to open the modal
pub fn handle_leaderboard_button_click(
    mut leaderboard_modal_state: ResMut<crate::ui::LeaderboardModalState>,
    interaction_query: Query<&Interaction, (Changed<Interaction>, With<LeaderboardButton>)>,
) {
    for interaction in &interaction_query {
        if *interaction == Interaction::Pressed {
            leaderboard_modal_state.open();
            info!("Opening leaderboard modal");
        }
    }
}

/// Handle ledger button clicks to open the modal
pub fn handle_ledger_button_click(
    mut ledger_modal_state: ResMut<crate::ui::LedgerModalState>,
    interaction_query: Query<&Interaction, (Changed<Interaction>, With<LedgerButton>)>,
) {
    for interaction in &interaction_query {
        if *interaction == Interaction::Pressed {
            ledger_modal_state.toggle();
        }
    }
}

/// Handle unit toggle button clicks
pub fn handle_unit_toggle_button_click(
    mut unit_system: ResMut<UnitSystem>,
    interaction_query: Query<&Interaction, (Changed<Interaction>, With<UnitToggleButton>)>,
    mut label_query: Query<&mut Text, With<UnitToggleLabel>>,
) {
    for interaction in &interaction_query {
        if *interaction == Interaction::Pressed {
            unit_system.toggle();
            let new_label = unit_system.label();
            for mut text in &mut label_query {
                **text = new_label.to_string();
            }
            info!("Unit system toggled to {new_label}");
        }
    }
}

/// Handle sound toggle button clicks
pub fn handle_sound_toggle_button_click(
    mut sound_enabled: ResMut<SoundEnabled>,
    image_assets: Res<ImageAssets>,
    interaction_query: Query<&Interaction, (Changed<Interaction>, With<SoundToggleButton>)>,
    mut label_query: Query<&mut Text, With<SoundToggleLabel>>,
    mut icon_query: Query<&mut ImageNode, With<SoundToggleIcon>>,
) {
    for interaction in &interaction_query {
        if *interaction == Interaction::Pressed {
            sound_enabled.toggle();
            let new_label = sound_enabled.label();
            for mut text in &mut label_query {
                **text = new_label.to_string();
            }
            let new_icon = if sound_enabled.0 {
                image_assets.icon_sound_on.clone()
            } else {
                image_assets.icon_sound_off.clone()
            };
            for mut image_node in &mut icon_query {
                image_node.image = new_icon.clone();
            }
            info!("Sound toggled to {new_label}");
        }
    }
}

pub fn sync_primary_nav_button_colors(
    nav_state: Res<NavigationState>,
    mut button_query: Query<(
        &PrimaryNavButton,
        &mut BackgroundColor,
        &mut BorderColor,
        &mut BeveledBorder,
    )>,
) {
    for (nav_btn, mut bg, mut border, mut bevel) in &mut button_query {
        let is_active = nav_btn.nav == nav_state.primary;
        if is_active {
            *bg = BackgroundColor(colors::BUTTON_ACTIVE_FACE);
            bevel.is_pressed = true;
            *border = bevel.to_border_color();
        } else {
            *bg = BackgroundColor(colors::BUTTON_FACE);
            bevel.is_pressed = false;
            *border = bevel.to_border_color();
        }
    }
}

pub fn update_weather_display(
    environment: Res<EnvironmentState>,
    multi_site: Res<crate::resources::MultiSiteManager>,
    unit_system: Res<UnitSystem>,
    image_assets: Res<ImageAssets>,
    mut current_weather: Query<&mut ImageNode, With<CurrentWeatherIcon>>,
    mut temp_label: Query<&mut Text, With<TemperatureLabel>>,
) {
    // Current weather icon - update the image based on weather type
    let weather_icon = match environment.current_weather {
        WeatherType::Sunny => image_assets.icon_weather_sunny.clone(),
        WeatherType::Overcast => image_assets.icon_weather_cloudy.clone(),
        WeatherType::Rainy => image_assets.icon_weather_rainy.clone(),
        WeatherType::Heatwave => image_assets.icon_weather_heatwave.clone(),
        WeatherType::Cold => image_assets.icon_weather_cold.clone(),
    };
    for mut image_node in &mut current_weather {
        image_node.image = weather_icon.clone();
    }

    // Temperature - adjusted for current site's climate
    let site_temp = if let Some(site) = multi_site.active_site() {
        environment.temperature_for_site(site.archetype)
    } else {
        environment.temperature_f
    };

    for mut text in &mut temp_label {
        **text = format!("{} -", unit_system.format_temp(site_temp));
    }
}

pub fn handle_temperature_tooltip(
    environment: Res<EnvironmentState>,
    multi_site: Res<MultiSiteManager>,
    unit_system: Res<UnitSystem>,
    hover_query: Query<&Interaction, (Changed<Interaction>, With<TemperatureHoverArea>)>,
    mut tooltip_query: Query<
        (&mut Visibility, &mut Text, &mut TooltipAutoHideTimer),
        With<TemperatureTooltip>,
    >,
) {
    for interaction in &hover_query {
        for (mut visibility, mut text, mut timer) in &mut tooltip_query {
            match interaction {
                Interaction::Hovered | Interaction::Pressed => {
                    // Calculate cold penalty for current site
                    let (cold_mult, site_temp) = if let Some(site) = multi_site.active_site() {
                        let temp = environment.temperature_for_site(site.archetype);
                        (
                            site.archetype
                                .cold_charging_multiplier(environment.temperature_f),
                            temp,
                        )
                    } else {
                        (1.0, environment.temperature_f)
                    };

                    // Format tooltip text
                    let power_pct = (cold_mult * 100.0).round() as i32;
                    let temp_display = unit_system.format_temp(site_temp);
                    if cold_mult < 1.0 {
                        **text = format!(
                            "Cold weather penalty\nCharging power: {power_pct}%\n({temp_display} reduces battery charge rate)",
                        );
                    } else {
                        **text = "Charging power: 100%\n(No temperature penalty)".to_string();
                    }
                    *visibility = Visibility::Visible;
                    // Stop auto-hide timer when manually hovering
                    timer.timer.pause();
                }
                Interaction::None => {
                    *visibility = Visibility::Hidden;
                }
            }
        }
    }
}

/// Show tooltip automatically when switching to a cold weather site
pub fn show_tooltip_on_cold_site_switch(
    mut switch_events: MessageReader<SiteSwitchEvent>,
    environment: Res<EnvironmentState>,
    multi_site: Res<MultiSiteManager>,
    unit_system: Res<UnitSystem>,
    mut tooltip_query: Query<
        (
            &mut Visibility,
            &mut Text,
            &mut TooltipAutoHideTimer,
            &mut BackgroundColor,
            &mut TextColor,
        ),
        With<TemperatureTooltip>,
    >,
) {
    for _event in switch_events.read() {
        // Check if the new active site has a cold climate
        if let Some(site) = multi_site.active_site()
            && site.archetype.climate_warning().is_some()
        {
            // This is a cold site - show the tooltip automatically
            let site_temp = environment.temperature_for_site(site.archetype);
            let cold_mult = site
                .archetype
                .cold_charging_multiplier(environment.temperature_f);
            let power_pct = (cold_mult * 100.0).round() as i32;
            let temp_display = unit_system.format_temp(site_temp);
            let penalty_threshold = unit_system.format_temp(32.0);

            for (mut visibility, mut text, mut timer, mut bg, mut text_color) in &mut tooltip_query
            {
                // Set tooltip text
                if cold_mult < 1.0 {
                    **text = format!(
                        "Cold weather penalty\nCharging power: {power_pct}%\n({temp_display} reduces battery charge rate)",
                    );
                } else {
                    **text = format!(
                        "Cold climate site\nCharging power: 100%\n(Currently {temp_display} - penalty below {penalty_threshold})",
                    );
                }

                // Show tooltip and start auto-hide timer
                *visibility = Visibility::Visible;
                timer.timer.reset();
                timer.timer.unpause();

                // Reset colors to full opacity
                *bg = BackgroundColor(Color::srgba(0.1, 0.1, 0.1, 0.95));
                *text_color = TextColor(Color::WHITE);
            }
        }
    }
}

/// Update tooltip auto-hide timer and fade out
pub fn update_tooltip_auto_hide(
    time: Res<Time>,
    mut tooltip_query: Query<
        (
            &mut Visibility,
            &mut TooltipAutoHideTimer,
            &mut BackgroundColor,
            &mut TextColor,
        ),
        With<TemperatureTooltip>,
    >,
) {
    for (mut visibility, mut timer, mut bg, mut text_color) in &mut tooltip_query {
        if timer.timer.is_paused() {
            continue;
        }

        timer.timer.tick(time.delta());

        // Start fading in the last 1 second
        let remaining = timer.timer.remaining_secs();
        if remaining < 1.0 && remaining > 0.0 {
            let alpha = remaining; // 1.0 -> 0.0 over the last second
            *bg = BackgroundColor(Color::srgba(0.1, 0.1, 0.1, 0.95 * alpha));
            *text_color = TextColor(Color::srgba(1.0, 1.0, 1.0, alpha));
        }

        // Hide when timer finishes
        if timer.timer.is_finished() {
            *visibility = Visibility::Hidden;
            timer.timer.pause();
            // Reset colors for next time
            *bg = BackgroundColor(Color::srgba(0.1, 0.1, 0.1, 0.95));
            *text_color = TextColor(Color::WHITE);
        }
    }
}

pub fn update_news_ticker(
    environment: Res<EnvironmentState>,
    build_state: Res<BuildState>,
    multi_site: Res<crate::resources::MultiSiteManager>,
    mut ticker_query: Query<(&mut Text, &mut TextColor), With<NewsTickerText>>,
) {
    // Get active site grid for validation
    let validation_message = if let Some(site) = multi_site.active_site() {
        let validation = site.grid.validate_for_open();
        if !build_state.is_open {
            if !validation.can_open {
                // Show validation issue
                Some((
                    validation.issues.first().cloned().unwrap_or_default(),
                    Color::srgb(0.9, 0.6, 0.2), // Orange for issues
                ))
            } else {
                // Ready to start
                Some((
                    "Ready to open! Click START DAY to begin.".to_string(),
                    Color::srgb(0.3, 0.9, 0.3), // Green for ready
                ))
            }
        } else {
            None // Day is running, show normal news
        }
    } else {
        None
    };

    for (mut text, mut text_color) in &mut ticker_query {
        if let Some((message, color)) = &validation_message {
            **text = message.clone();
            *text_color = TextColor(*color);
        } else {
            // Normal news ticker
            **text = environment
                .active_news
                .clone()
                .unwrap_or_else(|| "All systems operational.".to_string());
            *text_color = TextColor(Color::WHITE);
        }
    }
}
