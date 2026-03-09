//! Leaderboard modal - displays global leaderboard rankings in a popup

use bevy::ecs::hierarchy::ChildSpawnerCommands;
use bevy::prelude::*;
use bevy::ui::widget::NodeImageMode;

use crate::resources::{GameState, LeaderboardData, LeaderboardEntry};

const CAPSULE_SLICE_CAP: f32 = 17.0;
const CAPSULE_SLICE_EDGE: f32 = 2.0;

struct CapsuleImageHandles {
    name: Handle<Image>,
    score: Handle<Image>,
    name_gold: Handle<Image>,
    score_gold: Handle<Image>,
    name_silver: Handle<Image>,
    score_silver: Handle<Image>,
    name_bronze: Handle<Image>,
    score_bronze: Handle<Image>,
}

// ============ Components ============

/// Marker component for leaderboard modal UI elements
#[derive(Component)]
pub struct LeaderboardModalUI;

/// Close button marker for leaderboard modal
#[derive(Component, Debug, Clone, Copy)]
pub struct LeaderboardCloseButton;

/// Marker for leaderboard entry rows (to be despawned on rebuild)
#[derive(Component)]
pub struct LeaderboardEntryRow;

/// Scrollable container for leaderboard entries
#[derive(Component)]
pub struct LeaderboardEntriesContainer;

/// Loading text component
#[derive(Component)]
pub struct LeaderboardLoadingText;

/// Error text component
#[derive(Component)]
pub struct LeaderboardErrorText;

/// Marker for the player's own score value text
#[derive(Component)]
pub struct YourScoreText;

const LEADERBOARD_MODAL_WIDTH: f32 = 640.0;
const LEADERBOARD_LIST_HEIGHT: f32 = 520.0;
const PODIUM_ROW_HEIGHT: f32 = 44.0;
const STANDARD_ROW_HEIGHT: f32 = 34.0;
const SCORE_PILL_WIDTH: f32 = 168.0;
const SCORE_TRACK_INSET: f32 = 3.0;
const DASH_CONNECTOR_WIDTH: f32 = 12.0;

#[derive(Clone, Copy)]
struct LeaderboardRowStyle {
    rank_text: Color,
    rank_bg: Color,
    rank_border: Color,
    name_border: Color,
    name_text: Color,
    progress_fill: Color,
    row_height: f32,
    rank_width: f32,
    score_font_size: f32,
    name_font_size: f32,
}

// ============ Resource ============

/// Resource to control leaderboard modal visibility
#[derive(Resource, Default, Debug)]
pub struct LeaderboardModalState {
    pub is_open: bool,
}

impl LeaderboardModalState {
    pub fn open(&mut self) {
        self.is_open = true;
    }

    pub fn close(&mut self) {
        self.is_open = false;
    }

    pub fn toggle(&mut self) {
        self.is_open = !self.is_open;
    }
}

/// Cached image handles for the hexagonal capsule 9-slice assets.
#[derive(Resource)]
pub struct CapsuleAssets {
    pub name_image: Handle<Image>,
    pub score_image: Handle<Image>,
    pub name_gold: Handle<Image>,
    pub score_gold: Handle<Image>,
    pub name_silver: Handle<Image>,
    pub score_silver: Handle<Image>,
    pub name_bronze: Handle<Image>,
    pub score_bronze: Handle<Image>,
}

impl CapsuleAssets {
    pub fn load(asset_server: &AssetServer) -> Self {
        Self {
            name_image: asset_server.load("ui/capsule_name.png"),
            score_image: asset_server.load("ui/capsule_score.png"),
            name_gold: asset_server.load("ui/capsule_name_gold.png"),
            score_gold: asset_server.load("ui/capsule_score_gold.png"),
            name_silver: asset_server.load("ui/capsule_name_silver.png"),
            score_silver: asset_server.load("ui/capsule_score_silver.png"),
            name_bronze: asset_server.load("ui/capsule_name_bronze.png"),
            score_bronze: asset_server.load("ui/capsule_score_bronze.png"),
        }
    }
}

fn capsule_slicer() -> TextureSlicer {
    TextureSlicer {
        border: BorderRect {
            left: CAPSULE_SLICE_CAP,
            right: CAPSULE_SLICE_CAP,
            top: CAPSULE_SLICE_EDGE,
            bottom: CAPSULE_SLICE_EDGE,
        },
        center_scale_mode: SliceScaleMode::Stretch,
        sides_scale_mode: SliceScaleMode::Stretch,
        max_corner_scale: 1.0,
    }
}

// ============ Systems ============

/// Spawn the leaderboard modal when it's opened
pub fn spawn_leaderboard_modal(
    mut commands: Commands,
    modal_state: Res<LeaderboardModalState>,
    existing_modal: Query<Entity, With<LeaderboardModalUI>>,
    game_state: Res<GameState>,
) {
    // Only spawn if open and doesn't already exist
    if !modal_state.is_open || !existing_modal.is_empty() {
        return;
    }

    // Spawn modal overlay (dim background)
    commands
        .spawn((
            LeaderboardModalUI,
            Node {
                width: Val::Percent(100.0),
                height: Val::Percent(100.0),
                position_type: PositionType::Absolute,
                justify_content: JustifyContent::Center,
                align_items: AlignItems::Center,
                ..default()
            },
            BackgroundColor(Color::srgba(0.0, 0.0, 0.0, 0.7)),
            GlobalZIndex(2000), // Above day-end modal
        ))
        .with_children(|overlay| {
            // Wrapper: column layout so the title sits above the modal
            overlay
                .spawn(Node {
                    flex_direction: FlexDirection::Column,
                    align_items: AlignItems::Center,
                    max_height: Val::Percent(90.0),
                    ..default()
                })
                .with_children(|wrapper| {
                    // Title box — sits above the modal window
                    wrapper
                        .spawn((
                            Node {
                                padding: UiRect::new(
                                    Val::Px(28.0),
                                    Val::Px(28.0),
                                    Val::Px(8.0),
                                    Val::Px(8.0),
                                ),
                                border: UiRect::all(Val::Px(3.0)),
                                justify_content: JustifyContent::Center,
                                align_items: AlignItems::Center,
                                margin: UiRect::bottom(Val::Px(-20.0)),
                                ..default()
                            },
                            BackgroundColor(Color::srgb(0.08, 0.06, 0.14)),
                            BorderColor::all(Color::srgb(0.83, 0.67, 0.19)),
                            BorderRadius::all(Val::Px(10.0)),
                            ZIndex(1),
                        ))
                        .with_child((
                            Text::new("LEADERBOARD"),
                            TextFont {
                                font_size: 28.0,
                                ..default()
                            },
                            TextColor(Color::srgb(1.0, 0.84, 0.0)),
                        ));

                    // Modal container
                    wrapper
                        .spawn((
                            Node {
                                width: Val::Px(LEADERBOARD_MODAL_WIDTH),
                                flex_direction: FlexDirection::Column,
                                padding: UiRect::new(
                                    Val::Px(16.0),
                                    Val::Px(16.0),
                                    Val::Px(32.0),
                                    Val::Px(16.0),
                                ),
                                row_gap: Val::Px(10.0),
                                border: UiRect::all(Val::Px(3.0)),
                                ..default()
                            },
                            BackgroundColor(Color::srgb(0.11, 0.08, 0.18)),
                            BorderColor::all(Color::srgb(0.83, 0.67, 0.19)),
                            BorderRadius::all(Val::Px(12.0)),
                        ))
                        .with_children(|modal| {
                            // Close button (absolute top-right)
                            modal
                                .spawn((
                                    Button,
                                    Node {
                                        width: Val::Px(24.0),
                                        height: Val::Px(24.0),
                                        justify_content: JustifyContent::Center,
                                        align_items: AlignItems::Center,
                                        position_type: PositionType::Absolute,
                                        top: Val::Px(6.0),
                                        right: Val::Px(6.0),
                                        ..default()
                                    },
                                    BackgroundColor(Color::srgba(1.0, 1.0, 1.0, 0.1)),
                                    BorderRadius::all(Val::Px(4.0)),
                                    LeaderboardCloseButton,
                                ))
                                .with_child((
                                    Text::new("X"),
                                    TextFont {
                                        font_size: 14.0,
                                        ..default()
                                    },
                                    TextColor(Color::srgb(0.7, 0.7, 0.7)),
                                ));

                            // Loading state text (Display::None so it doesn't reserve layout space)
                            modal.spawn((
                                Text::new("Loading leaderboard..."),
                                TextFont {
                                    font_size: 13.0,
                                    ..default()
                                },
                                TextColor(Color::srgb(0.7, 0.7, 0.7)),
                                Node {
                                    margin: UiRect::all(Val::Px(12.0)),
                                    display: Display::None,
                                    ..default()
                                },
                                LeaderboardLoadingText,
                            ));

                            // Error state text (Display::None so it doesn't reserve layout space)
                            modal.spawn((
                                Text::new("Failed to load leaderboard"),
                                TextFont {
                                    font_size: 13.0,
                                    ..default()
                                },
                                TextColor(Color::srgb(0.9, 0.3, 0.3)),
                                Node {
                                    margin: UiRect::all(Val::Px(12.0)),
                                    display: Display::None,
                                    ..default()
                                },
                                LeaderboardErrorText,
                            ));

                            // Scrollable container for entries
                            modal
                                .spawn((
                                    Node {
                                        width: Val::Percent(100.0),
                                        height: Val::Px(LEADERBOARD_LIST_HEIGHT),
                                        flex_direction: FlexDirection::Column,
                                        row_gap: Val::Px(4.0),
                                        padding: UiRect::all(Val::Px(4.0)),
                                        overflow: Overflow::clip_y(),
                                        ..default()
                                    },
                                    BackgroundColor(Color::srgba(0.02, 0.05, 0.08, 0.55)),
                                    BorderColor::all(Color::srgba(0.84, 0.69, 0.24, 0.45)),
                                    BorderRadius::all(Val::Px(10.0)),
                                    LeaderboardEntriesContainer,
                                ))
                                .with_children(|_scroll_container| {
                                    // Entries will be spawned/updated dynamically
                                });

                            // "Your Score" bar
                            let current_score = game_state.calculate_cumulative_score();
                            let score_color = if current_score >= 0 {
                                Color::srgb(0.4, 0.9, 0.4)
                            } else {
                                Color::srgb(0.9, 0.4, 0.4)
                            };

                            modal
                                .spawn((
                                    Node {
                                        width: Val::Percent(100.0),
                                        flex_direction: FlexDirection::Row,
                                        justify_content: JustifyContent::SpaceBetween,
                                        align_items: AlignItems::Center,
                                        min_height: Val::Px(62.0),
                                        padding: UiRect::all(Val::Px(6.0)),
                                        column_gap: Val::Px(10.0),
                                        margin: UiRect::top(Val::Px(2.0)),
                                        border: UiRect::all(Val::Px(2.0)),
                                        ..default()
                                    },
                                    BackgroundColor(Color::srgb(0.31, 0.21, 0.05)),
                                    BorderColor::all(Color::srgb(0.92, 0.75, 0.21)),
                                    BorderRadius::all(Val::Px(10.0)),
                                ))
                                .with_children(|row| {
                                    row.spawn((
                                        Node {
                                            flex_grow: 1.0,
                                            min_height: Val::Px(48.0),
                                            justify_content: JustifyContent::Center,
                                            padding: UiRect::horizontal(Val::Px(18.0)),
                                            border: UiRect::all(Val::Px(2.0)),
                                            ..default()
                                        },
                                        BackgroundColor(Color::srgb(0.95, 0.82, 0.25)),
                                        BorderColor::all(Color::srgb(0.99, 0.91, 0.55)),
                                        BorderRadius::all(Val::Px(8.0)),
                                    ))
                                    .with_child((
                                        Text::new("Your Score"),
                                        TextFont {
                                            font_size: 22.0,
                                            ..default()
                                        },
                                        TextColor(Color::srgb(0.24, 0.14, 0.03)),
                                    ));

                                    row.spawn((
                                        Node {
                                            width: Val::Px(170.0),
                                            min_height: Val::Px(48.0),
                                            justify_content: JustifyContent::Center,
                                            align_items: AlignItems::Center,
                                            padding: UiRect::horizontal(Val::Px(14.0)),
                                            border: UiRect::all(Val::Px(2.0)),
                                            ..default()
                                        },
                                        BackgroundColor(Color::srgb(0.34, 0.23, 0.07)),
                                        BorderColor::all(Color::srgb(0.92, 0.75, 0.21)),
                                        BorderRadius::all(Val::Px(8.0)),
                                    ))
                                    .with_child((
                                        Text::new(format_score(current_score)),
                                        TextFont {
                                            font_size: 22.0,
                                            ..default()
                                        },
                                        TextColor(score_color),
                                        YourScoreText,
                                    ));
                                });

                            // Help text
                            modal.spawn((
                                Text::new(
                                    "Your score is automatically submitted at the end of each day!",
                                ),
                                TextFont {
                                    font_size: 11.0,
                                    ..default()
                                },
                                TextColor(Color::srgb(0.6, 0.6, 0.6)),
                                Node {
                                    margin: UiRect::top(Val::Px(8.0)),
                                    ..default()
                                },
                            ));
                        });
                });
        });
}

/// Despawn the leaderboard modal when it's closed
pub fn despawn_leaderboard_modal(
    mut commands: Commands,
    modal_state: Res<LeaderboardModalState>,
    modal_query: Query<Entity, With<LeaderboardModalUI>>,
) {
    // Despawn if closed and exists
    if !modal_state.is_open {
        for entity in &modal_query {
            commands.entity(entity).try_despawn();
        }
    }
}

/// Handle close button clicks
pub fn handle_leaderboard_close_button(
    interaction_query: Query<&Interaction, (Changed<Interaction>, With<LeaderboardCloseButton>)>,
    mut modal_state: ResMut<LeaderboardModalState>,
) {
    for interaction in &interaction_query {
        if *interaction == Interaction::Pressed {
            modal_state.close();
        }
    }
}

/// Update the leaderboard entries display
pub fn update_leaderboard_modal_content(
    leaderboard_data: Res<LeaderboardData>,
    mut commands: Commands,
    container_query: Query<Entity, With<LeaderboardEntriesContainer>>,
    entry_rows: Query<Entity, With<LeaderboardEntryRow>>,
    mut loading_text: Query<
        &mut Node,
        (
            With<LeaderboardLoadingText>,
            Without<LeaderboardErrorText>,
            Without<YourScoreText>,
        ),
    >,
    mut error_text: Query<
        (&mut Node, &mut Text),
        (
            With<LeaderboardErrorText>,
            Without<LeaderboardLoadingText>,
            Without<YourScoreText>,
        ),
    >,
    mut your_score_text: Query<
        (&mut Text, &mut TextColor),
        (
            With<YourScoreText>,
            Without<LeaderboardLoadingText>,
            Without<LeaderboardErrorText>,
        ),
    >,
    game_state: Res<GameState>,
    time: Res<Time>,
    asset_server: Res<AssetServer>,
    capsule_assets: Option<Res<CapsuleAssets>>,
) {
    let now = time.elapsed_secs_f64();

    // Update loading/error display (use Display::None/Flex to avoid reserving layout space)
    for mut node in &mut loading_text {
        node.display = if leaderboard_data.is_loading {
            Display::Flex
        } else {
            Display::None
        };
    }

    for (mut node, mut text) in &mut error_text {
        if let Some(error) = &leaderboard_data.error {
            node.display = Display::Flex;

            // Build error message with retry info
            let mut error_msg = error.clone();
            if leaderboard_data.failed_attempts > 0 {
                let retry_in = leaderboard_data.retry_remaining_secs(now);
                if retry_in > 0.0 {
                    error_msg.push_str(&format!(
                        "\nRetrying in {:.0} seconds... (attempt {}/8)",
                        retry_in, leaderboard_data.failed_attempts
                    ));
                } else {
                    error_msg.push_str(&format!(
                        "\nRetrying... (attempt {}/8)",
                        leaderboard_data.failed_attempts
                    ));
                }
            }

            text.0 = error_msg;
        } else {
            node.display = Display::None;
        }
    }

    // Update "Your Score" display
    let current_score = game_state.calculate_cumulative_score();
    for (mut text, mut color) in &mut your_score_text {
        text.0 = format_score(current_score);
        *color = if current_score >= 0 {
            TextColor(Color::srgb(0.4, 0.9, 0.4))
        } else {
            TextColor(Color::srgb(0.9, 0.4, 0.4))
        };
    }

    // Rebuild entries when data changes or the container was freshly spawned with no
    // rows yet. This handles re-opening the modal with cached data: the UI is re-created
    // but LeaderboardData hasn't changed, so is_changed() alone would skip the rebuild
    // and leave the user looking at a blank leaderboard.
    let container_is_fresh = !container_query.is_empty() && entry_rows.is_empty();
    if !leaderboard_data.is_changed() && !container_is_fresh {
        return;
    }

    // Despawn all existing entry rows
    for entity in &entry_rows {
        commands.entity(entity).despawn();
    }

    // Lazily initialise capsule image handles
    let capsules = match capsule_assets {
        Some(res) => CapsuleImageHandles {
            name: res.name_image.clone(),
            score: res.score_image.clone(),
            name_gold: res.name_gold.clone(),
            score_gold: res.score_gold.clone(),
            name_silver: res.name_silver.clone(),
            score_silver: res.score_silver.clone(),
            name_bronze: res.name_bronze.clone(),
            score_bronze: res.score_bronze.clone(),
        },
        None => {
            let loaded = CapsuleAssets::load(&asset_server);
            let handles = CapsuleImageHandles {
                name: loaded.name_image.clone(),
                score: loaded.score_image.clone(),
                name_gold: loaded.name_gold.clone(),
                score_gold: loaded.score_gold.clone(),
                name_silver: loaded.name_silver.clone(),
                score_silver: loaded.score_silver.clone(),
                name_bronze: loaded.name_bronze.clone(),
                score_bronze: loaded.score_bronze.clone(),
            };
            commands.insert_resource(loaded);
            handles
        }
    };

    // Rebuild the entries container
    for container_entity in &container_query {
        commands.entity(container_entity).with_children(|parent| {
            if leaderboard_data.entries.is_empty() {
                parent.spawn((
                    Text::new("No entries yet"),
                    TextFont {
                        font_size: 14.0,
                        ..default()
                    },
                    TextColor(Color::srgb(0.6, 0.6, 0.6)),
                    Node {
                        margin: UiRect::all(Val::Px(16.0)),
                        ..default()
                    },
                    LeaderboardEntryRow,
                ));
            } else {
                let top_score = leaderboard_data
                    .entries
                    .first()
                    .map_or(0, |entry| entry.score);
                let min_score = leaderboard_data
                    .entries
                    .iter()
                    .map(|entry| entry.score)
                    .min()
                    .unwrap_or(top_score);

                for (index, entry) in leaderboard_data.entries.iter().enumerate() {
                    spawn_leaderboard_entry_row(
                        parent,
                        entry,
                        index + 1,
                        top_score,
                        min_score,
                        &capsules,
                    );
                }
            }
        });
    }
}

fn spawn_leaderboard_entry_row(
    parent: &mut ChildSpawnerCommands,
    entry: &LeaderboardEntry,
    rank: usize,
    top_score: i64,
    min_score: i64,
    capsules: &CapsuleImageHandles,
) {
    let style = leaderboard_row_style(rank);
    let progress = leaderboard_progress_fraction(entry.score, top_score, min_score);
    let score_color = score_text_color(entry.score);
    let score_fill_width = (progress * 100.0).clamp(0.0, 100.0);
    let rank_width = style.rank_width;

    let (name_img, score_img) = match rank {
        1 => (capsules.name_gold.clone(), capsules.score_gold.clone()),
        2 => (capsules.name_silver.clone(), capsules.score_silver.clone()),
        3 => (capsules.name_bronze.clone(), capsules.score_bronze.clone()),
        _ => (capsules.name.clone(), capsules.score.clone()),
    };

    parent
        .spawn((
            Node {
                width: Val::Percent(100.0),
                flex_direction: FlexDirection::Row,
                column_gap: Val::Px(8.0),
                align_items: AlignItems::Center,
                ..default()
            },
            LeaderboardEntryRow,
        ))
        .with_children(|row| {
            row.spawn((
                Node {
                    width: Val::Px(rank_width),
                    min_width: Val::Px(rank_width),
                    min_height: Val::Px(style.row_height),
                    justify_content: JustifyContent::Center,
                    align_items: AlignItems::Center,
                    padding: UiRect::horizontal(Val::Px(6.0)),
                    border: UiRect::all(Val::Px(2.0)),
                    ..default()
                },
                BackgroundColor(style.rank_bg),
                BorderColor::all(style.rank_border),
                BorderRadius::all(Val::Px(8.0)),
            ))
            .with_child((
                Text::new(format!("#{}", rank)),
                TextFont {
                    font_size: if rank <= 3 { 17.0 } else { 13.0 },
                    ..default()
                },
                TextColor(style.rank_text),
            ));

            spawn_standard_name_capsule(row, entry, style, name_img);
            spawn_dash_connector(row, style);
            spawn_standard_score_capsule(
                row,
                entry,
                style,
                score_fill_width,
                score_color,
                score_img,
            );
        });
}

fn leaderboard_row_style(rank: usize) -> LeaderboardRowStyle {
    match rank {
        1 => LeaderboardRowStyle {
            rank_text: Color::srgb(0.28, 0.18, 0.02),
            rank_bg: Color::srgb(0.95, 0.81, 0.22),
            rank_border: Color::srgb(1.0, 0.92, 0.62),
            name_border: Color::srgb(0.98, 0.91, 0.43),
            name_text: Color::srgb(0.97, 0.98, 0.91),
            progress_fill: Color::srgb(0.36, 0.82, 0.34),
            row_height: PODIUM_ROW_HEIGHT,
            rank_width: 56.0,
            score_font_size: 17.0,
            name_font_size: 15.0,
        },
        2 => LeaderboardRowStyle {
            rank_text: Color::srgb(0.16, 0.18, 0.24),
            rank_bg: Color::srgb(0.82, 0.85, 0.9),
            rank_border: Color::srgb(0.96, 0.98, 1.0),
            name_border: Color::srgb(0.75, 0.82, 0.91),
            name_text: Color::srgb(0.94, 0.97, 0.95),
            progress_fill: Color::srgb(0.28, 0.69, 0.3),
            row_height: PODIUM_ROW_HEIGHT,
            rank_width: 56.0,
            score_font_size: 17.0,
            name_font_size: 15.0,
        },
        3 => LeaderboardRowStyle {
            rank_text: Color::srgb(0.24, 0.11, 0.05),
            rank_bg: Color::srgb(0.78, 0.5, 0.28),
            rank_border: Color::srgb(0.96, 0.79, 0.64),
            name_border: Color::srgb(0.86, 0.59, 0.41),
            name_text: Color::srgb(0.94, 0.95, 0.91),
            progress_fill: Color::srgb(0.25, 0.63, 0.27),
            row_height: PODIUM_ROW_HEIGHT,
            rank_width: 56.0,
            score_font_size: 17.0,
            name_font_size: 15.0,
        },
        _ => LeaderboardRowStyle {
            rank_text: Color::srgb(0.96, 0.83, 0.35),
            rank_bg: Color::srgb(0.17, 0.14, 0.31),
            rank_border: Color::srgb(0.34, 0.3, 0.56),
            name_border: Color::srgb(0.18, 0.28, 0.5),
            name_text: Color::srgb(0.9, 0.94, 1.0),
            progress_fill: Color::srgb(0.29, 0.72, 0.31),
            row_height: STANDARD_ROW_HEIGHT,
            rank_width: 38.0,
            score_font_size: 13.0,
            name_font_size: 12.0,
        },
    }
}

fn spawn_standard_name_capsule(
    row: &mut ChildSpawnerCommands,
    entry: &LeaderboardEntry,
    style: LeaderboardRowStyle,
    capsule_image: Handle<Image>,
) {
    row.spawn((
        ImageNode {
            image: capsule_image,
            image_mode: NodeImageMode::Sliced(capsule_slicer()),
            ..default()
        },
        Node {
            flex_grow: 1.0,
            min_height: Val::Px(style.row_height),
            justify_content: JustifyContent::Center,
            align_items: AlignItems::Center,
            padding: UiRect::horizontal(Val::Px(22.0)),
            ..default()
        },
    ))
    .with_child((
        Text::new(&entry.player_name),
        TextFont {
            font_size: style.name_font_size,
            ..default()
        },
        TextColor(style.name_text),
    ));
}

fn spawn_dash_connector(row: &mut ChildSpawnerCommands, style: LeaderboardRowStyle) {
    row.spawn((
        Node {
            width: Val::Px(DASH_CONNECTOR_WIDTH),
            height: Val::Px(2.0),
            ..default()
        },
        BackgroundColor(style.name_border),
    ));
}

fn spawn_standard_score_capsule(
    row: &mut ChildSpawnerCommands,
    entry: &LeaderboardEntry,
    style: LeaderboardRowStyle,
    score_fill_width: f32,
    score_color: Color,
    capsule_image: Handle<Image>,
) {
    row.spawn((
        ImageNode {
            image: capsule_image,
            image_mode: NodeImageMode::Sliced(capsule_slicer()),
            ..default()
        },
        Node {
            width: Val::Px(SCORE_PILL_WIDTH),
            min_width: Val::Px(SCORE_PILL_WIDTH),
            min_height: Val::Px(style.row_height),
            padding: UiRect::all(Val::Px(SCORE_TRACK_INSET)),
            ..default()
        },
    ))
    .with_children(|score_box| {
        spawn_score_track(score_box, entry.score, style, score_fill_width, score_color);
    });
}

fn spawn_score_track(
    parent: &mut ChildSpawnerCommands,
    score: i64,
    style: LeaderboardRowStyle,
    score_fill_width: f32,
    score_color: Color,
) {
    parent
        .spawn((
            Node {
                width: Val::Percent(100.0),
                min_height: Val::Px(style.row_height - (SCORE_TRACK_INSET * 2.0)),
                justify_content: JustifyContent::FlexStart,
                overflow: Overflow::clip_x(),
                padding: UiRect::horizontal(Val::Px(CAPSULE_SLICE_CAP - SCORE_TRACK_INSET)),
                ..default()
            },
            BackgroundColor(Color::NONE),
        ))
        .with_children(|track| {
            track.spawn((
                Node {
                    width: Val::Percent(score_fill_width),
                    height: Val::Percent(100.0),
                    ..default()
                },
                BackgroundColor(style.progress_fill.with_alpha(0.5)),
            ));

            track
                .spawn(Node {
                    position_type: PositionType::Absolute,
                    left: Val::Px(0.0),
                    right: Val::Px(0.0),
                    top: Val::Px(0.0),
                    bottom: Val::Px(0.0),
                    justify_content: JustifyContent::Center,
                    align_items: AlignItems::Center,
                    ..default()
                })
                .with_child((
                    Text::new(format_score(score)),
                    TextFont {
                        font_size: style.score_font_size,
                        ..default()
                    },
                    TextColor(score_color),
                ));
        });
}

fn leaderboard_progress_fraction(score: i64, top_score: i64, min_score: i64) -> f32 {
    if top_score == min_score {
        return if score == top_score { 1.0 } else { 0.0 };
    }

    let numerator = (score - min_score) as f32;
    let denominator = (top_score - min_score) as f32;
    (numerator / denominator).clamp(0.0, 1.0)
}

fn score_text_color(score: i64) -> Color {
    if score >= 0 {
        Color::srgb(0.53, 0.96, 0.48)
    } else {
        Color::srgb(0.95, 0.47, 0.47)
    }
}

/// Format a score with dollar sign and thousand separators (e.g., $1,250,000)
fn format_score(score: i64) -> String {
    let abs_score = score.abs();
    let sign = if score < 0 { "-" } else { "" };

    if abs_score >= 1_000_000 {
        format!("{}${:.2}M", sign, abs_score as f64 / 1_000_000.0)
    } else if abs_score >= 1_000 {
        format!("{}${:.1}K", sign, abs_score as f64 / 1_000.0)
    } else {
        format!("{}${}", sign, abs_score)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // ---- LeaderboardModalState ----

    #[test]
    fn test_modal_state_default_is_closed() {
        let state = LeaderboardModalState::default();
        assert!(!state.is_open);
    }

    #[test]
    fn test_modal_state_open() {
        let mut state = LeaderboardModalState::default();
        state.open();
        assert!(state.is_open);
    }

    #[test]
    fn test_modal_state_close() {
        let mut state = LeaderboardModalState::default();
        state.open();
        state.close();
        assert!(!state.is_open);
    }

    #[test]
    fn test_modal_state_toggle() {
        let mut state = LeaderboardModalState::default();
        assert!(!state.is_open);

        state.toggle();
        assert!(state.is_open);

        state.toggle();
        assert!(!state.is_open);
    }

    // ---- format_score ----

    #[test]
    fn test_format_score_zero() {
        assert_eq!(format_score(0), "$0");
    }

    #[test]
    fn test_format_score_small_positive() {
        assert_eq!(format_score(42), "$42");
        assert_eq!(format_score(999), "$999");
    }

    #[test]
    fn test_format_score_thousands() {
        assert_eq!(format_score(1_000), "$1.0K");
        assert_eq!(format_score(2_100), "$2.1K");
        assert_eq!(format_score(999_999), "$1000.0K");
    }

    #[test]
    fn test_format_score_millions() {
        assert_eq!(format_score(1_000_000), "$1.00M");
        assert_eq!(format_score(1_500_000), "$1.50M");
        assert_eq!(format_score(12_345_678), "$12.35M");
    }

    #[test]
    fn test_format_score_negative_small() {
        assert_eq!(format_score(-23), "-$23");
        assert_eq!(format_score(-500), "-$500");
    }

    #[test]
    fn test_format_score_negative_thousands() {
        assert_eq!(format_score(-1_000), "-$1.0K");
        assert_eq!(format_score(-97_000), "-$97.0K");
    }

    #[test]
    fn test_format_score_negative_millions() {
        assert_eq!(format_score(-2_500_000), "-$2.50M");
    }

    #[test]
    fn test_progress_fraction_top_score_is_full_width() {
        assert!((leaderboard_progress_fraction(100, 100, 25) - 1.0).abs() < f32::EPSILON);
    }

    #[test]
    fn test_progress_fraction_uses_visible_score_range() {
        assert!((leaderboard_progress_fraction(50, 100, 0) - 0.5).abs() < f32::EPSILON);
    }

    #[test]
    fn test_progress_fraction_handles_negative_scores() {
        assert!((leaderboard_progress_fraction(-10, -10, -50) - 1.0).abs() < f32::EPSILON);
        assert!((leaderboard_progress_fraction(-50, -10, -50) - 0.0).abs() < f32::EPSILON);
    }

    #[test]
    fn test_progress_fraction_handles_all_equal_scores() {
        assert!((leaderboard_progress_fraction(0, 0, 0) - 1.0).abs() < f32::EPSILON);
    }
}
