//! Radial action menu for chargers (tower defense style)
//!
//! Layout:
//! - Center: Charger icon with ID label and selection ring
//! - Top: Status + Health bar card
//! - Bottom: Power bar card
//! - Left: Circular Reboot button with icon
//! - Right: Circular Dispatch button with icon

use bevy::ecs::hierarchy::ChildSpawnerCommands;
use bevy::prelude::*;

use crate::components::charger::{Charger, ChargerState, ChargerType, FaultType, RemoteAction};
use crate::events::{RemoteActionRequestEvent, TechnicianDispatchEvent};
use crate::resources::{
    BuildState, BuildTool, GameState, ImageAssets, SelectedChargerEntity, TechnicianState,
};
use crate::systems::WorldCamera;
use crate::systems::build_input::PlacementCursor;

// ============ Constants ============

const MENU_RADIUS: f32 = 100.0;
const BUTTON_SIZE: f32 = 60.0;
const CARD_WIDTH: f32 = 160.0;
const CARD_HEIGHT: f32 = 50.0;
const CENTER_RING_SIZE: f32 = 80.0;
const ICON_SIZE: f32 = 32.0;

// Colors
const CARD_BG: Color = Color::srgba(0.1, 0.12, 0.15, 0.95);
const BUTTON_BG: Color = Color::srgba(0.2, 0.22, 0.25, 0.95);
const BUTTON_BG_HOVER: Color = Color::srgba(0.3, 0.32, 0.35, 0.95);
const BUTTON_BG_PRESSED: Color = Color::srgba(0.15, 0.5, 0.8, 0.95);
const BUTTON_BG_DISABLED: Color = Color::srgba(0.15, 0.15, 0.15, 0.8);
const BUTTON_BORDER: Color = Color::srgba(0.4, 0.42, 0.45, 0.9);
const BUTTON_BG_NEEDS_REBOOT: Color = Color::srgba(0.7, 0.55, 0.1, 0.95);
const BUTTON_BORDER_NEEDS_REBOOT: Color = Color::srgba(1.0, 0.8, 0.2, 1.0);
const BUTTON_BG_NEEDS_DISPATCH: Color = Color::srgba(0.6, 0.15, 0.15, 0.95);
const BUTTON_BORDER_NEEDS_DISPATCH: Color = Color::srgba(1.0, 0.3, 0.3, 1.0);
const CENTER_RING_BORDER: Color = Color::srgb(0.2, 0.6, 0.9);
const HEALTH_BAR_GREEN: Color = Color::srgb(0.2, 0.8, 0.3);
const POWER_BAR_ORANGE: Color = Color::srgb(0.9, 0.6, 0.2);
const BAR_TRACK: Color = Color::srgba(0.2, 0.2, 0.2, 0.8);
const BACKDROP_DIAMETER: f32 = 300.0;
const BACKDROP_BG: Color = Color::srgba(0.08, 0.08, 0.12, 0.7);
const LABEL_BG: Color = Color::srgba(0.1, 0.1, 0.12, 0.85);

// ============ Components ============

#[derive(Component)]
pub struct RadialMenu {
    pub charger_entity: Entity,
}

#[derive(Component)]
pub struct RadialMenuButton {
    pub action: RadialMenuAction,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RadialMenuAction {
    Reboot,
    Dispatch,
    UpgradeAntiTheft,
}

// Marker components for live updates
#[derive(Component)]
pub struct RadialStatusText;

#[derive(Component)]
pub struct RadialHealthBar;

#[derive(Component)]
pub struct RadialHealthText;

#[derive(Component)]
pub struct RadialPowerBar;

#[derive(Component)]
pub struct RadialPowerText;

#[derive(Component)]
pub struct RadialChargerId;

#[derive(Component)]
pub struct RadialKwhTodayText;

#[derive(Component)]
pub struct RadialKwhLifetimeText;

#[derive(Component)]
pub struct RadialCenterIcon;

#[derive(Component)]
pub struct RadialButtonIcon;

/// Full-screen invisible layer that dismisses the radial menu when clicked
#[derive(Component)]
pub struct RadialMenuDismissLayer;

/// Component for animating GIF frames (used for UI-based GIF playback)
#[derive(Component)]
pub struct GifAnimator {
    pub frames: Vec<Handle<Image>>,
    pub current_frame: usize,
    pub timer: Timer,
}

/// Resource holding pre-loaded GIF animation frames
#[derive(Resource, Default)]
pub struct GifAnimationFrames {
    pub dancing_banana: Vec<Handle<Image>>,
    pub dancing_banana_loaded: bool,
}

/// Tracks button press flash effect with a timer
#[derive(Component)]
pub struct ButtonPressFlash {
    pub timer: Timer,
    pub success: bool, // true = action executed, false = disabled/unavailable
}

impl ButtonPressFlash {
    pub fn new(success: bool) -> Self {
        Self {
            timer: Timer::from_seconds(0.3, TimerMode::Once),
            success,
        }
    }
}

/// Tracks action-needed pulse animation for buttons that should draw attention
#[derive(Component)]
pub struct ActionNeededPulse {
    pub timer: Timer,
    pub base_color: Color,
    pub highlight_color: Color,
}

impl ActionNeededPulse {
    pub fn new(base_color: Color, highlight_color: Color) -> Self {
        Self {
            timer: Timer::from_seconds(1.5, TimerMode::Repeating),
            base_color,
            highlight_color,
        }
    }
}

// ============ Menu Spawning ============

/// Spawn radial menu when charger is selected
pub fn spawn_radial_menu(
    mut commands: Commands,
    selected: Res<SelectedChargerEntity>,
    chargers: Query<(&Charger, &GlobalTransform)>,
    existing_menus: Query<Entity, With<RadialMenu>>,
    existing_dismiss_layers: Query<Entity, With<RadialMenuDismissLayer>>,
    cameras: Query<(&Camera, &GlobalTransform), With<WorldCamera>>,
    windows: Query<&Window>,
    images: Res<ImageAssets>,
    mut build_state: ResMut<BuildState>,
    placement_cursors: Query<Entity, With<PlacementCursor>>,
    tech_state: Res<TechnicianState>,
    game_state: Res<GameState>,
) {
    // Only proceed if a charger is selected
    let Some(charger_entity) = selected.0 else {
        return;
    };

    // Despawn existing menus and dismiss layers before spawning new one
    // (close_radial_menu_on_deselect handles the deselection case)
    for menu in &existing_menus {
        commands.entity(menu).try_despawn();
    }
    for layer in &existing_dismiss_layers {
        commands.entity(layer).try_despawn();
    }

    // Cancel any active build placement when showing radial menu
    for cursor_entity in &placement_cursors {
        commands.entity(cursor_entity).try_despawn();
    }
    build_state.selected_tool = BuildTool::Select;

    let Ok((charger, global_transform)) = chargers.get(charger_entity) else {
        return;
    };

    // Convert world position to screen position
    let Ok((camera, camera_transform)) = cameras.single() else {
        return;
    };
    let Ok(_window) = windows.single() else {
        return;
    };

    // Use GlobalTransform for correct world position (chargers are children of site root)
    let world_pos = global_transform.translation().truncate();

    let Some(screen_pos) = camera
        .world_to_viewport(camera_transform, world_pos.extend(0.0))
        .ok()
    else {
        return;
    };

    // Spawn full-screen dismiss layer (click anywhere outside menu to dismiss)
    commands.spawn((
        Button,
        Node {
            position_type: PositionType::Absolute,
            width: Val::Percent(100.0),
            height: Val::Percent(100.0),
            left: Val::Px(0.0),
            top: Val::Px(0.0),
            ..default()
        },
        BackgroundColor(Color::NONE),
        RadialMenuDismissLayer,
        ZIndex(999), // Below radial menu (1000) but above other UI
    ));

    // Spawn menu container (using screen coordinates)
    commands
        .spawn((
            Node {
                position_type: PositionType::Absolute,
                width: Val::Px(0.0),
                height: Val::Px(0.0),
                left: Val::Px(screen_pos.x),
                top: Val::Px(screen_pos.y),
                ..default()
            },
            RadialMenu { charger_entity },
            ZIndex(1000),
        ))
        .with_children(|parent| {
            // === Backdrop Circle ===
            // Use PickingBehavior::IGNORE so clicks pass through to the buttons
            parent.spawn((
                Node {
                    width: Val::Px(BACKDROP_DIAMETER),
                    height: Val::Px(BACKDROP_DIAMETER),
                    position_type: PositionType::Absolute,
                    left: Val::Px(-BACKDROP_DIAMETER / 2.0),
                    top: Val::Px(-BACKDROP_DIAMETER / 2.0),
                    ..default()
                },
                BackgroundColor(BACKDROP_BG),
                BorderRadius::all(Val::Px(BACKDROP_DIAMETER / 2.0)),
                Pickable::IGNORE,
            ));

            // === Center Ring with Charger Icon ===
            spawn_center_ring(parent, charger, &images);

            // === Top Card: Status + Health ===
            spawn_status_card(parent, charger);

            // === Bottom Card: Power ===
            spawn_power_card(parent, charger);

            // === Left Button: Reboot ===
            let reboot_enabled = is_action_enabled(
                RadialMenuAction::Reboot,
                charger,
                charger_entity,
                &tech_state,
                None,
            );
            let reboot_recommended = is_action_recommended(RadialMenuAction::Reboot, charger);
            spawn_action_button(
                parent,
                RadialMenuAction::Reboot,
                -MENU_RADIUS,
                0.0,
                reboot_enabled,
                reboot_recommended,
                images.icon_action_soft_reboot.clone(),
                "Reboot",
            );

            // === Right Button: Dispatch ===
            let dispatch_enabled = is_action_enabled(
                RadialMenuAction::Dispatch,
                charger,
                charger_entity,
                &tech_state,
                None as Option<&GameState>,
            );
            let dispatch_recommended = is_action_recommended(RadialMenuAction::Dispatch, charger);
            spawn_action_button(
                parent,
                RadialMenuAction::Dispatch,
                MENU_RADIUS,
                0.0,
                dispatch_enabled,
                dispatch_recommended,
                images.icon_action_dispatch.clone(),
                "Dispatch",
            );

            // === Bottom Button: Upgrade anti-theft cable or Sell ===
            let anti_theft_price = charger.anti_theft_cable_price();
            let upgrade_enabled = if charger.anti_theft_cable {
                true
            } else {
                game_state.can_afford_build(anti_theft_price)
            };
            let upgrade_label = anti_theft_button_label(charger.anti_theft_cable, anti_theft_price);
            let upgrade_icon = if charger.anti_theft_cable {
                images.icon_action_refund.clone()
            } else {
                images.icon_action_anti_theft.clone()
            };
            spawn_action_button(
                parent,
                RadialMenuAction::UpgradeAntiTheft,
                0.0,
                MENU_RADIUS,
                upgrade_enabled,
                false,
                upgrade_icon,
                upgrade_label,
            );
        });
}

/// Label for the anti-theft upgrade button (static strings to avoid allocation).
fn anti_theft_button_label(installed: bool, price: i32) -> &'static str {
    if installed {
        // Sell refund = 50% of price
        return match price {
            800 => "Sell Protection ($400)",
            3_200 => "Sell Protection ($1,600)",
            6_000 => "Sell Protection ($3,000)",
            10_000 => "Sell Protection ($5,000)",
            _ => "Sell Protection",
        };
    }
    match price {
        800 => "Anti-theft ($800)",
        3_200 => "Anti-theft ($3,200)",
        6_000 => "Anti-theft ($6,000)",
        10_000 => "Anti-theft ($10,000)",
        _ => "Anti-theft",
    }
}

fn spawn_center_ring(parent: &mut ChildSpawnerCommands, charger: &Charger, images: &ImageAssets) {
    // Outer ring with border
    parent
        .spawn((
            Node {
                width: Val::Px(CENTER_RING_SIZE),
                height: Val::Px(CENTER_RING_SIZE),
                position_type: PositionType::Absolute,
                left: Val::Px(-CENTER_RING_SIZE / 2.0),
                top: Val::Px(-CENTER_RING_SIZE / 2.0),
                justify_content: JustifyContent::Center,
                align_items: AlignItems::Center,
                flex_direction: FlexDirection::Column,
                border: UiRect::all(Val::Px(3.0)),
                ..default()
            },
            BackgroundColor(CARD_BG),
            BorderColor::all(CENTER_RING_BORDER),
            BorderRadius::all(Val::Px(CENTER_RING_SIZE / 2.0)),
        ))
        .with_children(|ring| {
            // Charger icon - preserve aspect ratio (288x560 original, ~1:2 ratio)
            let icon_handle = get_charger_icon(charger, images);
            ring.spawn((
                ImageNode::new(icon_handle),
                Node {
                    width: Val::Px(35.0),
                    height: Val::Px(68.0), // Preserve ~1:2 aspect ratio
                    ..default()
                },
                RadialCenterIcon,
            ));

            // Charger ID text
            ring.spawn((
                Text::new(charger.id.to_uppercase()),
                TextFont {
                    font_size: 11.0,
                    ..default()
                },
                TextColor(Color::srgb(0.7, 0.85, 1.0)),
                RadialChargerId,
            ));
        });
}

fn spawn_status_card(parent: &mut ChildSpawnerCommands, charger: &Charger) {
    let card_y = -MENU_RADIUS - 10.0;
    let card_height = CARD_HEIGHT + 24.0;

    parent
        .spawn((
            Node {
                width: Val::Px(CARD_WIDTH),
                height: Val::Px(card_height),
                position_type: PositionType::Absolute,
                left: Val::Px(-CARD_WIDTH / 2.0),
                top: Val::Px(card_y - card_height / 2.0),
                flex_direction: FlexDirection::Column,
                justify_content: JustifyContent::Center,
                align_items: AlignItems::Center,
                padding: UiRect::all(Val::Px(8.0)),
                row_gap: Val::Px(4.0),
                ..default()
            },
            BackgroundColor(CARD_BG),
            BorderRadius::all(Val::Px(8.0)),
        ))
        .with_children(|card| {
            // Status text
            let status_color = get_status_color(charger.state());
            card.spawn((
                Text::new(format!("Status: {}", charger.state().display_name())),
                TextFont {
                    font_size: 12.0,
                    ..default()
                },
                TextColor(status_color),
                RadialStatusText,
            ));

            // Health bar container
            card.spawn((Node {
                width: Val::Percent(100.0),
                height: Val::Px(8.0),
                flex_direction: FlexDirection::Row,
                align_items: AlignItems::Center,
                column_gap: Val::Px(6.0),
                ..default()
            },))
                .with_children(|row| {
                    row.spawn((
                        Node {
                            width: Val::Percent(70.0),
                            height: Val::Px(6.0),
                            ..default()
                        },
                        BackgroundColor(BAR_TRACK),
                        BorderRadius::all(Val::Px(3.0)),
                    ))
                    .with_children(|track| {
                        track.spawn((
                            Node {
                                width: Val::Percent(charger.health * 100.0),
                                height: Val::Percent(100.0),
                                ..default()
                            },
                            BackgroundColor(HEALTH_BAR_GREEN),
                            BorderRadius::all(Val::Px(3.0)),
                            RadialHealthBar,
                        ));
                    });

                    row.spawn((
                        Text::new(format!("{:.0}%", charger.health * 100.0)),
                        TextFont {
                            font_size: 10.0,
                            ..default()
                        },
                        TextColor(Color::srgb(0.7, 0.7, 0.7)),
                        RadialHealthText,
                    ));
                });

            card.spawn((
                Text::new(format!(
                    "Today: {:.1} kWh",
                    charger.energy_delivered_kwh_today
                )),
                TextFont {
                    font_size: 10.0,
                    ..default()
                },
                TextColor(Color::srgb(0.7, 0.85, 1.0)),
                RadialKwhTodayText,
            ));

            card.spawn((
                Text::new(format!(
                    "Lifetime: {:.1} kWh",
                    charger.total_energy_delivered_kwh
                )),
                TextFont {
                    font_size: 10.0,
                    ..default()
                },
                TextColor(Color::srgb(0.7, 0.85, 1.0)),
                RadialKwhLifetimeText,
            ));
        });
}

fn spawn_power_card(parent: &mut ChildSpawnerCommands, charger: &Charger) {
    let card_y = MENU_RADIUS + 10.0;

    parent
        .spawn((
            Node {
                width: Val::Px(CARD_WIDTH),
                height: Val::Px(CARD_HEIGHT),
                position_type: PositionType::Absolute,
                left: Val::Px(-CARD_WIDTH / 2.0),
                top: Val::Px(card_y - CARD_HEIGHT / 2.0),
                flex_direction: FlexDirection::Column,
                justify_content: JustifyContent::Center,
                align_items: AlignItems::Center,
                padding: UiRect::all(Val::Px(8.0)),
                row_gap: Val::Px(4.0),
                ..default()
            },
            BackgroundColor(CARD_BG),
            BorderRadius::all(Val::Px(8.0)),
        ))
        .with_children(|card| {
            // Power label
            card.spawn((
                Text::new("Power"),
                TextFont {
                    font_size: 11.0,
                    ..default()
                },
                TextColor(Color::srgb(0.6, 0.6, 0.6)),
            ));

            // Power bar container
            card.spawn((Node {
                width: Val::Percent(100.0),
                height: Val::Px(8.0),
                flex_direction: FlexDirection::Row,
                align_items: AlignItems::Center,
                column_gap: Val::Px(6.0),
                ..default()
            },))
                .with_children(|row| {
                    // Power bar track
                    let power_pct = if charger.rated_power_kw > 0.0 {
                        (charger.current_power_kw / charger.rated_power_kw * 100.0)
                            .clamp(0.0, 100.0)
                    } else {
                        0.0
                    };

                    row.spawn((
                        Node {
                            width: Val::Percent(50.0),
                            height: Val::Px(6.0),
                            ..default()
                        },
                        BackgroundColor(BAR_TRACK),
                        BorderRadius::all(Val::Px(3.0)),
                    ))
                    .with_children(|track| {
                        // Power bar fill
                        track.spawn((
                            Node {
                                width: Val::Percent(power_pct),
                                height: Val::Percent(100.0),
                                ..default()
                            },
                            BackgroundColor(POWER_BAR_ORANGE),
                            BorderRadius::all(Val::Px(3.0)),
                            RadialPowerBar,
                        ));
                    });

                    // Power text
                    row.spawn((
                        Text::new(format!(
                            "{:.0} / {:.0} kW",
                            charger.current_power_kw, charger.rated_power_kw
                        )),
                        TextFont {
                            font_size: 10.0,
                            ..default()
                        },
                        TextColor(Color::srgb(0.7, 0.7, 0.7)),
                        RadialPowerText,
                    ));
                });
        });
}

fn spawn_action_button(
    parent: &mut ChildSpawnerCommands,
    action: RadialMenuAction,
    x: f32,
    y: f32,
    enabled: bool,
    recommended: bool,
    icon: Handle<Image>,
    label: &str,
) {
    let (bg_color, border_color) = if recommended {
        // Use attention-grabbing colors when action is recommended
        match action {
            RadialMenuAction::Reboot => (BUTTON_BG_NEEDS_REBOOT, BUTTON_BORDER_NEEDS_REBOOT),
            RadialMenuAction::Dispatch => (BUTTON_BG_NEEDS_DISPATCH, BUTTON_BORDER_NEEDS_DISPATCH),
            RadialMenuAction::UpgradeAntiTheft => (BUTTON_BG, BUTTON_BORDER),
        }
    } else if enabled {
        (BUTTON_BG, BUTTON_BORDER)
    } else {
        (BUTTON_BG_DISABLED, Color::srgba(0.25, 0.25, 0.25, 0.6))
    };
    let text_alpha = if enabled { 1.0 } else { 0.5 };

    // Button container (includes label below). Fixed width so wide labels don't shift center.
    parent
        .spawn((Node {
            position_type: PositionType::Absolute,
            left: Val::Px(x - BUTTON_SIZE / 2.0),
            top: Val::Px(y - BUTTON_SIZE / 2.0),
            width: Val::Px(BUTTON_SIZE),
            flex_direction: FlexDirection::Column,
            align_items: AlignItems::Center,
            row_gap: Val::Px(4.0),
            ..default()
        },))
        .with_children(|container| {
            // Circular button
            let mut button = container.spawn((
                Button,
                Node {
                    width: Val::Px(BUTTON_SIZE),
                    height: Val::Px(BUTTON_SIZE),
                    justify_content: JustifyContent::Center,
                    align_items: AlignItems::Center,
                    border: UiRect::all(Val::Px(2.0)),
                    ..default()
                },
                BackgroundColor(bg_color),
                BorderColor::all(border_color),
                BorderRadius::all(Val::Px(BUTTON_SIZE / 2.0)),
                RadialMenuButton { action },
            ));

            // Add pulse animation if action is recommended
            if recommended {
                let highlight_color = match action {
                    RadialMenuAction::Reboot => BUTTON_BORDER_NEEDS_REBOOT,
                    RadialMenuAction::Dispatch => BUTTON_BORDER_NEEDS_DISPATCH,
                    RadialMenuAction::UpgradeAntiTheft => BUTTON_BORDER,
                };
                button.insert(ActionNeededPulse::new(bg_color, highlight_color));
            }

            button.with_children(|btn| {
                // Icon - dim when disabled for clearer visual feedback
                let icon_alpha = if enabled { 1.0 } else { 0.4 };
                btn.spawn((
                    ImageNode::new(icon).with_color(Color::srgba(1.0, 1.0, 1.0, icon_alpha)),
                    Node {
                        width: Val::Px(ICON_SIZE),
                        height: Val::Px(ICON_SIZE),
                        ..default()
                    },
                    RadialButtonIcon,
                ));
            });

            // Label below button (with pill background)
            container
                .spawn((
                    Node {
                        padding: UiRect::new(
                            Val::Px(8.0),
                            Val::Px(8.0),
                            Val::Px(3.0),
                            Val::Px(3.0),
                        ),
                        ..default()
                    },
                    BackgroundColor(LABEL_BG),
                    BorderRadius::all(Val::Px(8.0)),
                ))
                .with_child((
                    Text::new(label),
                    TextFont {
                        font_size: 11.0,
                        ..default()
                    },
                    TextColor(Color::srgba(0.8, 0.8, 0.8, text_alpha)),
                ));
        });
}

fn get_charger_icon(charger: &Charger, images: &ImageAssets) -> Handle<Image> {
    // Use the same state-based image selection as the in-world sprite
    match charger.charger_type {
        ChargerType::DcFast => {
            if charger.rated_power_kw <= 75.0 {
                // 50kW compact DCFC
                match charger.state() {
                    ChargerState::Available => images.charger_dcfc50_available.clone(),
                    ChargerState::Charging => images.charger_dcfc50_charging.clone(),
                    ChargerState::Warning => images.charger_dcfc50_warning.clone(),
                    ChargerState::Offline | ChargerState::Disabled => {
                        images.charger_dcfc50_offline.clone()
                    }
                }
            } else if charger.rated_power_kw <= 125.0 {
                // 100kW standard DCFC (built-in ad screen)
                match charger.state() {
                    ChargerState::Available => images.charger_dcfc100_available.clone(),
                    ChargerState::Charging => images.charger_dcfc100_charging.clone(),
                    ChargerState::Warning => images.charger_dcfc100_warning.clone(),
                    ChargerState::Offline | ChargerState::Disabled => {
                        images.charger_dcfc100_offline.clone()
                    }
                }
            } else if charger.rated_power_kw <= 200.0 {
                // 150kW standard DCFC
                match charger.state() {
                    ChargerState::Available => images.charger_dcfc150_available.clone(),
                    ChargerState::Charging => images.charger_dcfc150_charging.clone(),
                    ChargerState::Warning => images.charger_dcfc150_warning.clone(),
                    ChargerState::Offline | ChargerState::Disabled => {
                        images.charger_dcfc150_offline.clone()
                    }
                }
            } else {
                // 350kW premium DCFC
                match charger.state() {
                    ChargerState::Available => images.charger_dcfc350_available.clone(),
                    ChargerState::Charging => images.charger_dcfc350_charging.clone(),
                    ChargerState::Warning => images.charger_dcfc350_warning.clone(),
                    ChargerState::Offline | ChargerState::Disabled => {
                        images.charger_dcfc350_offline.clone()
                    }
                }
            }
        }
        ChargerType::AcLevel2 => match charger.state() {
            ChargerState::Available => images.charger_l2_available.clone(),
            ChargerState::Charging => images.charger_l2_charging.clone(),
            ChargerState::Warning => images.charger_l2_warning.clone(),
            ChargerState::Offline | ChargerState::Disabled => images.charger_l2_offline.clone(),
        },
    }
}

fn get_status_color(state: ChargerState) -> Color {
    match state {
        ChargerState::Available => Color::srgb(0.2, 0.8, 0.2),
        ChargerState::Charging => Color::srgb(0.2, 0.6, 1.0),
        ChargerState::Warning => Color::srgb(1.0, 0.8, 0.2),
        ChargerState::Offline => Color::srgb(1.0, 0.3, 0.3),
        ChargerState::Disabled => Color::srgb(0.5, 0.5, 0.5),
    }
}

fn is_action_enabled(
    action: RadialMenuAction,
    charger: &Charger,
    charger_entity: Entity,
    tech_state: &TechnicianState,
    game_state: Option<&GameState>,
) -> bool {
    match action {
        RadialMenuAction::Reboot => {
            matches!(
                charger.state(),
                ChargerState::Warning | ChargerState::Offline
            ) && charger.current_fault.is_some()
                && !charger
                    .current_fault
                    .map(|f| f.requires_technician())
                    .unwrap_or(false)
        }
        RadialMenuAction::Dispatch => {
            // Button is enabled if charger needs a technician AND not already queued/being serviced
            charger
                .current_fault
                .map(|f| f.requires_technician())
                .unwrap_or(false)
                && !tech_state.is_charger_queued(charger_entity)
        }
        RadialMenuAction::UpgradeAntiTheft => {
            if charger.anti_theft_cable {
                true
            } else {
                game_state
                    .map(|g| g.can_afford_build(charger.anti_theft_cable_price()))
                    .unwrap_or(false)
            }
        }
    }
}

fn is_action_recommended(action: RadialMenuAction, charger: &Charger) -> bool {
    match action {
        RadialMenuAction::Reboot => {
            charger.state() == ChargerState::Warning
                && charger
                    .current_fault
                    .map(|f| !f.requires_technician())
                    .unwrap_or(false)
        }
        RadialMenuAction::Dispatch => charger
            .current_fault
            .map(|f| f.requires_technician())
            .unwrap_or(false),
        RadialMenuAction::UpgradeAntiTheft => false,
    }
}

// ============ Menu Interactions ============

pub fn handle_radial_menu_buttons(
    mut commands: Commands,
    mut interaction_query: Query<
        (
            Entity,
            &Interaction,
            &RadialMenuButton,
            &mut BackgroundColor,
            Option<&ActionNeededPulse>,
        ),
        Changed<Interaction>,
    >,
    menu_query: Query<&RadialMenu>,
    mut chargers: Query<&mut Charger>,
    tech_state: Res<TechnicianState>,
    mut game_state: ResMut<GameState>,
    mut action_events: MessageWriter<RemoteActionRequestEvent>,
    mut dispatch_events: MessageWriter<TechnicianDispatchEvent>,
    mut selected: ResMut<SelectedChargerEntity>,
    mut tips_state: ResMut<crate::systems::gameplay_tips::GameplayTipsState>,
) {
    for (button_entity, interaction, button, mut bg_color, pulse_opt) in &mut interaction_query {
        // Get the charger entity from the menu
        let Ok(menu) = menu_query.single() else {
            continue;
        };

        let Ok(mut charger) = chargers.get_mut(menu.charger_entity) else {
            continue;
        };

        let enabled = is_action_enabled(
            button.action,
            &charger,
            menu.charger_entity,
            &tech_state,
            Some(&game_state),
        );

        // Handle interaction feedback and actions
        match *interaction {
            Interaction::Hovered if enabled => {
                *bg_color = BackgroundColor(BUTTON_BG_HOVER);
            }
            Interaction::None if enabled => {
                // If button has pulse animation, don't set static color - let pulse handle it
                if pulse_opt.is_none() {
                    *bg_color = BackgroundColor(BUTTON_BG);
                }
            }
            Interaction::None if !enabled => {
                // Disabled buttons never have pulse animation
                *bg_color = BackgroundColor(BUTTON_BG_DISABLED);
            }
            Interaction::Pressed => {
                // Track that the player attempted a radial menu action (for tip gating)
                if enabled {
                    match button.action {
                        RadialMenuAction::Reboot => tips_state.manual_reboots += 1,
                        RadialMenuAction::Dispatch => tips_state.manual_dispatches += 1,
                        RadialMenuAction::UpgradeAntiTheft => {}
                    }
                }

                // Start visual flash effect - insert flash component for timed animation
                let action_success = if enabled {
                    match button.action {
                        RadialMenuAction::Reboot => {
                            let action = if charger.current_fault == Some(FaultType::FirmwareFault)
                            {
                                RemoteAction::HardReboot
                            } else {
                                RemoteAction::SoftReboot
                            };

                            info!(
                                "Radial menu: Sending {:?} action to charger {}",
                                action, charger.id
                            );
                            action_events.write(RemoteActionRequestEvent {
                                charger_entity: menu.charger_entity,
                                action,
                            });
                            true
                        }
                        RadialMenuAction::Dispatch => {
                            info!(
                                "Radial menu: Dispatching technician to charger {}",
                                charger.id
                            );
                            dispatch_events.write(TechnicianDispatchEvent {
                                charger_entity: menu.charger_entity,
                                charger_id: charger.id.clone(),
                            });
                            true
                        }
                        RadialMenuAction::UpgradeAntiTheft => {
                            if charger.anti_theft_cable {
                                // Sell: refund 50% and remove upgrade
                                let refund = charger.anti_theft_cable_refund();
                                game_state.refund_build(charger.anti_theft_cable_price());
                                charger.anti_theft_cable = false;
                                info!(
                                    "Radial menu: Sold anti-theft cable from charger {} (refund ${})",
                                    charger.id, refund
                                );
                                true
                            } else {
                                // Buy: spend money and install upgrade
                                let price = charger.anti_theft_cable_price();
                                if game_state.try_spend_build(price) {
                                    charger.anti_theft_cable = true;
                                    info!(
                                        "Radial menu: Upgraded charger {} to anti-theft cable (${})",
                                        charger.id, price
                                    );
                                    true
                                } else {
                                    false
                                }
                            }
                        }
                    }
                } else {
                    info!(
                        "Radial menu: {:?} action not available for charger {} (state: {:?}, fault: {:?})",
                        button.action,
                        charger.id,
                        charger.state(),
                        charger.current_fault
                    );
                    false
                };

                // Set initial flash color and handle dismissal
                if action_success {
                    *bg_color = BackgroundColor(BUTTON_BG_PRESSED);
                    // Dismiss menu after successful action by deselecting charger.
                    // Note: We don't insert ButtonPressFlash here since the menu
                    // is being closed immediately and the button will be despawned.
                    selected.0 = None;
                } else {
                    // Only insert flash component for failed actions where the menu stays open
                    commands
                        .entity(button_entity)
                        .insert(ButtonPressFlash::new(action_success));
                    *bg_color = BackgroundColor(Color::srgba(0.5, 0.2, 0.2, 0.95));
                }
            }
            _ => {}
        }
    }
}

/// Animate button press flash effect over time
pub fn update_button_flash(
    mut commands: Commands,
    mut query: Query<(
        Entity,
        &mut ButtonPressFlash,
        &mut BackgroundColor,
        &RadialMenuButton,
        Option<&ActionNeededPulse>,
    )>,
    menu_query: Query<&RadialMenu>,
    chargers: Query<&Charger>,
    time: Res<Time>,
    tech_state: Res<TechnicianState>,
    game_state: Res<GameState>,
) {
    for (entity, mut flash, mut bg_color, button, pulse_opt) in &mut query {
        flash.timer.tick(time.delta());

        // Get current enabled state for proper reset color
        let enabled = menu_query
            .single()
            .ok()
            .and_then(|menu| {
                chargers
                    .get(menu.charger_entity)
                    .ok()
                    .map(|charger| (charger, menu.charger_entity))
            })
            .map(|(charger, charger_entity)| {
                is_action_enabled(
                    button.action,
                    charger,
                    charger_entity,
                    &tech_state,
                    Some(&game_state),
                )
            })
            .unwrap_or(false);

        let progress = flash.timer.fraction();

        if flash.timer.is_finished() {
            // Remove flash component and reset to normal color
            commands.entity(entity).try_remove::<ButtonPressFlash>();
            // If button has pulse animation, don't set static color - let pulse handle it
            if pulse_opt.is_none() {
                *bg_color = BackgroundColor(if enabled {
                    BUTTON_BG
                } else {
                    BUTTON_BG_DISABLED
                });
            }
        } else {
            // Interpolate from flash color back to normal
            let target_color = if enabled {
                BUTTON_BG
            } else {
                BUTTON_BG_DISABLED
            };
            let flash_color = if flash.success {
                BUTTON_BG_PRESSED
            } else {
                Color::srgba(0.5, 0.2, 0.2, 0.95)
            };

            // Ease out - flash is brightest at start, fades to normal
            let t = progress * progress; // Quadratic ease out
            let r = lerp_color_channel(flash_color.to_srgba().red, target_color.to_srgba().red, t);
            let g = lerp_color_channel(
                flash_color.to_srgba().green,
                target_color.to_srgba().green,
                t,
            );
            let b =
                lerp_color_channel(flash_color.to_srgba().blue, target_color.to_srgba().blue, t);
            let a = lerp_color_channel(
                flash_color.to_srgba().alpha,
                target_color.to_srgba().alpha,
                t,
            );

            *bg_color = BackgroundColor(Color::srgba(r, g, b, a));
        }
    }
}

fn lerp_color_channel(a: f32, b: f32, t: f32) -> f32 {
    a + (b - a) * t
}

/// Animate action-needed pulse effect for buttons requiring player attention
pub fn update_action_pulse(
    mut query: Query<(&mut ActionNeededPulse, &mut BackgroundColor)>,
    time: Res<Time>,
) {
    for (mut pulse, mut bg_color) in &mut query {
        pulse.timer.tick(time.delta());

        // Use sine wave for smooth pulsing: 0 -> 1 -> 0
        let progress = pulse.timer.fraction();
        let sine_wave = (progress * std::f32::consts::PI * 2.0).sin();
        // Map sine from [-1, 1] to [0, 1]
        let t = (sine_wave + 1.0) / 2.0;

        // Interpolate between base and highlight colors
        let base_srgba = pulse.base_color.to_srgba();
        let highlight_srgba = pulse.highlight_color.to_srgba();

        let r = lerp_color_channel(base_srgba.red, highlight_srgba.red, t);
        let g = lerp_color_channel(base_srgba.green, highlight_srgba.green, t);
        let b = lerp_color_channel(base_srgba.blue, highlight_srgba.blue, t);
        let a = lerp_color_channel(base_srgba.alpha, highlight_srgba.alpha, t);

        *bg_color = BackgroundColor(Color::srgba(r, g, b, a));
    }
}

// ============ Real-time Updates ============

pub fn update_radial_menu_data(
    menu_query: Query<&RadialMenu>,
    chargers: Query<&Charger>,
    mut status_text: Query<(&mut Text, &mut TextColor), With<RadialStatusText>>,
    mut health_bar: Query<&mut Node, (With<RadialHealthBar>, Without<RadialPowerBar>)>,
    mut health_text: Query<
        &mut Text,
        (
            With<RadialHealthText>,
            Without<RadialStatusText>,
            Without<RadialPowerText>,
            Without<RadialKwhTodayText>,
            Without<RadialKwhLifetimeText>,
        ),
    >,
    mut power_bar: Query<&mut Node, (With<RadialPowerBar>, Without<RadialHealthBar>)>,
    mut power_text: Query<
        &mut Text,
        (
            With<RadialPowerText>,
            Without<RadialStatusText>,
            Without<RadialHealthText>,
            Without<RadialKwhTodayText>,
            Without<RadialKwhLifetimeText>,
        ),
    >,
    mut kwh_today_text: Query<
        &mut Text,
        (
            With<RadialKwhTodayText>,
            Without<RadialStatusText>,
            Without<RadialHealthText>,
            Without<RadialPowerText>,
            Without<RadialKwhLifetimeText>,
        ),
    >,
    mut kwh_lifetime_text: Query<
        &mut Text,
        (
            With<RadialKwhLifetimeText>,
            Without<RadialStatusText>,
            Without<RadialHealthText>,
            Without<RadialPowerText>,
            Without<RadialKwhTodayText>,
        ),
    >,
    mut center_icon: Query<&mut ImageNode, With<RadialCenterIcon>>,
    images: Res<ImageAssets>,
) {
    let Ok(menu) = menu_query.single() else {
        return;
    };

    let Ok(charger) = chargers.get(menu.charger_entity) else {
        return;
    };

    for (mut text, mut color) in &mut status_text {
        **text = format!("Status: {}", charger.state().display_name());
        *color = TextColor(get_status_color(charger.state()));
    }

    for mut icon in &mut center_icon {
        icon.image = get_charger_icon(charger, &images);
    }

    for mut node in &mut health_bar {
        node.width = Val::Percent(charger.health * 100.0);
    }

    for mut text in &mut health_text {
        **text = format!("{:.0}%", charger.health * 100.0);
    }

    let power_pct = if charger.rated_power_kw > 0.0 {
        (charger.current_power_kw / charger.rated_power_kw * 100.0).clamp(0.0, 100.0)
    } else {
        0.0
    };
    for mut node in &mut power_bar {
        node.width = Val::Percent(power_pct);
    }

    for mut text in &mut power_text {
        **text = format!(
            "{:.0} / {:.0} kW",
            charger.current_power_kw, charger.rated_power_kw
        );
    }

    for mut text in &mut kwh_today_text {
        **text = format!("Today: {:.1} kWh", charger.energy_delivered_kwh_today);
    }

    for mut text in &mut kwh_lifetime_text {
        **text = format!("Lifetime: {:.1} kWh", charger.total_energy_delivered_kwh);
    }
}

/// Handle clicks on the dismiss layer to close the radial menu
pub fn handle_dismiss_layer_click(
    mut selected: ResMut<SelectedChargerEntity>,
    query: Query<&Interaction, (Changed<Interaction>, With<RadialMenuDismissLayer>)>,
) {
    for interaction in &query {
        if *interaction == Interaction::Pressed {
            selected.0 = None;
        }
    }
}

/// Close menu when clicking outside or pressing Escape
pub fn close_radial_menu_on_deselect(
    mut commands: Commands,
    mut selected: ResMut<SelectedChargerEntity>,
    menu_query: Query<Entity, With<RadialMenu>>,
    dismiss_layer_query: Query<Entity, With<RadialMenuDismissLayer>>,
    keyboard: Res<ButtonInput<KeyCode>>,
) {
    // Close on Escape key when menu is open
    if keyboard.just_pressed(KeyCode::Escape) && selected.0.is_some() {
        selected.0 = None;
    }

    // Despawn menu and dismiss layer when selection cleared
    if selected.is_changed() && selected.0.is_none() {
        for entity in &menu_query {
            commands.entity(entity).try_despawn();
        }
        for entity in &dismiss_layer_query {
            commands.entity(entity).try_despawn();
        }
    }
}

// ============ Ad Viewer Modal ============

/// Embedded GIF data (works on both native and WASM)
const DANCING_BANANA_GIF: &[u8] = include_bytes!("../../assets/ads/dancing-banana.gif");

/// Load GIF frames from embedded data and store in resource
pub fn load_gif_frames(
    mut gif_frames: ResMut<GifAnimationFrames>,
    mut images: ResMut<Assets<Image>>,
) {
    if gif_frames.dancing_banana_loaded {
        return;
    }

    // Decode GIF frames using the image crate from embedded bytes
    let Ok(decoder) = image::codecs::gif::GifDecoder::new(std::io::Cursor::new(DANCING_BANANA_GIF))
    else {
        warn!("Failed to decode embedded GIF");
        gif_frames.dancing_banana_loaded = true;
        return;
    };

    use image::AnimationDecoder;
    let frames_iter = decoder.into_frames();

    for frame_result in frames_iter {
        let Ok(frame) = frame_result else {
            continue;
        };

        let rgba_image = frame.into_buffer();
        let (width, height) = rgba_image.dimensions();

        // Create a Bevy Image from the frame
        let bevy_image = Image::new(
            bevy::render::render_resource::Extent3d {
                width,
                height,
                depth_or_array_layers: 1,
            },
            bevy::render::render_resource::TextureDimension::D2,
            rgba_image.into_raw(),
            bevy::render::render_resource::TextureFormat::Rgba8UnormSrgb,
            bevy::asset::RenderAssetUsages::RENDER_WORLD,
        );

        let handle = images.add(bevy_image);
        gif_frames.dancing_banana.push(handle);
    }

    info!(
        "Loaded {} frames from dancing banana GIF",
        gif_frames.dancing_banana.len()
    );
    gif_frames.dancing_banana_loaded = true;
}

/// Update GIF animations by cycling through frames
pub fn update_gif_animations(
    mut query: Query<(&mut ImageNode, &mut GifAnimator)>,
    time: Res<Time>,
) {
    for (mut image_node, mut animator) in &mut query {
        if animator.frames.is_empty() {
            continue;
        }

        animator.timer.tick(time.delta());

        if animator.timer.just_finished() {
            animator.current_frame = (animator.current_frame + 1) % animator.frames.len();
            image_node.image = animator.frames[animator.current_frame].clone();
        }
    }
}
