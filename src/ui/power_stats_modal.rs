//! Power stats modal -- a live 24h chart of site power draw vs the effective
//! site limit (sum of per-charger caps / rated power, clamped to grid). Opened
//! from Stats -> Power. Backed by `SiteState::power_history`.

use bevy::ecs::hierarchy::ChildSpawnerCommands;
use bevy::prelude::*;

use crate::components::BelongsToSite;
use crate::components::charger::Charger;
use crate::ocpp::charging_profiles::{ActiveProfileRow, ChargingProfileStore};
use crate::ocpp::queue::OcppMessageQueue;
use crate::resources::site_energy::PowerSample;
use crate::resources::{GameClock, MultiSiteManager};
use crate::systems::power_history::{LimitSource, site_limit_breakdown};

// ============ Components ============

#[derive(Component)]
pub struct PowerStatsModalUI;

#[derive(Component)]
pub struct PowerStatsCloseButton;

/// Container the chart columns are (re)built into.
#[derive(Component)]
pub struct PowerChartPlot;

/// A single chart column (despawned and rebuilt as history grows).
#[derive(Component)]
pub struct PowerChartColumn;

#[derive(Component)]
pub struct PowerYAxisMaxLabel;

#[derive(Component)]
pub struct PowerYAxisMidLabel;

#[derive(Component)]
pub struct PowerSummaryDrawLabel;

#[derive(Component)]
pub struct PowerSummaryLimitLabel;

#[derive(Component)]
pub struct PowerSummaryPeakDrawLabel;

#[derive(Component)]
pub struct PowerSummaryPeakLimitLabel;

/// Which tab of the modal is showing.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum PowerModalTab {
    #[default]
    Chart,
    Profiles,
}

/// Tab selector button.
#[derive(Component)]
pub struct PowerModalTabButton {
    pub tab: PowerModalTab,
}

/// Marks the root node of a tab's content (toggled via `Node.display`).
#[derive(Component)]
pub struct PowerTabRoot {
    pub tab: PowerModalTab,
}

/// A breakdown-strip value label; the field determines what value it shows.
#[derive(Component, Clone, Copy)]
pub enum PowerBreakdownField {
    Grid,
    Chargers,
    Ocpp,
    Effective,
}

/// The "Limited by: ..." source badge text.
#[derive(Component)]
pub struct PowerSourceBadge;

/// Container the per-charger profile rows are (re)built into.
#[derive(Component)]
pub struct PowerProfilesContainer;

/// A single profile-list row (despawned and rebuilt on change).
#[derive(Component)]
pub struct PowerProfileRow;

// ============ Resource ============

#[derive(Resource, Debug, Default)]
pub struct PowerStatsModalState {
    pub is_open: bool,
    pub active_tab: PowerModalTab,
    /// Sample count at last chart rebuild (rebuild when it changes).
    last_sample_count: usize,
    /// Profile count + tab signature at last profiles rebuild.
    last_profile_signature: (usize, PowerModalTab),
}

impl PowerStatsModalState {
    pub fn toggle(&mut self) {
        self.is_open = !self.is_open;
        if !self.is_open {
            self.force_rebuild();
        }
    }

    pub fn close(&mut self) {
        self.is_open = false;
        self.force_rebuild();
    }

    fn force_rebuild(&mut self) {
        self.last_sample_count = usize::MAX;
        self.last_profile_signature = (usize::MAX, PowerModalTab::Chart);
    }
}

// ============ Colors ============

const PANEL_BG: Color = Color::srgb(0.12, 0.14, 0.18);
const BORDER: Color = Color::srgb(0.3, 0.35, 0.4);
const GOLD: Color = Color::srgb(1.0, 0.84, 0.0);
const DIM_TEXT: Color = Color::srgb(0.6, 0.6, 0.6);
const BRIGHT_TEXT: Color = Color::srgb(0.9, 0.9, 0.9);
const PLOT_BG: Color = Color::srgba(0.0, 0.0, 0.0, 0.3);
const DRAW_COLOR: Color = Color::srgb(0.2, 0.7, 1.0);
const LIMIT_COLOR: Color = Color::srgba(1.0, 0.55, 0.2, 0.45);
const GRIDLINE: Color = Color::srgba(1.0, 1.0, 1.0, 0.08);
const TAB_ACTIVE: Color = Color::srgba(1.0, 1.0, 1.0, 0.15);
const TAB_INACTIVE: Color = Color::srgba(1.0, 1.0, 1.0, 0.05);
const SRC_GRID: Color = Color::srgb(1.0, 0.55, 0.2);
const SRC_OCPP: Color = Color::srgb(0.4, 0.8, 1.0);
const SRC_HARDWARE: Color = Color::srgb(0.6, 0.85, 0.5);
const ROW_BORDER: Color = Color::srgba(1.0, 1.0, 1.0, 0.06);

/// Max chart columns; history is downsampled to this width for cheap rendering.
const MAX_COLUMNS: usize = 240;
/// Plot height in logical pixels.
const PLOT_HEIGHT: f32 = 260.0;

// ============ Spawn / despawn ============

pub fn spawn_power_stats_modal(
    mut commands: Commands,
    modal_state: Res<PowerStatsModalState>,
    existing: Query<Entity, With<PowerStatsModalUI>>,
) {
    if !modal_state.is_open || !existing.is_empty() {
        return;
    }

    commands
        .spawn((
            PowerStatsModalUI,
            Node {
                width: Val::Percent(100.0),
                height: Val::Percent(100.0),
                position_type: PositionType::Absolute,
                justify_content: JustifyContent::Center,
                align_items: AlignItems::Center,
                ..default()
            },
            BackgroundColor(Color::srgba(0.0, 0.0, 0.0, 0.7)),
            GlobalZIndex(2000),
        ))
        .with_children(|overlay| {
            overlay
                .spawn((
                    Node {
                        width: Val::Px(820.0),
                        max_height: Val::Percent(90.0),
                        flex_direction: FlexDirection::Column,
                        padding: UiRect::all(Val::Px(24.0)),
                        row_gap: Val::Px(12.0),
                        ..default()
                    },
                    BackgroundColor(PANEL_BG),
                    BorderColor::all(BORDER),
                    BorderRadius::all(Val::Px(12.0)),
                ))
                .with_children(|modal| {
                    spawn_header(modal);
                    spawn_breakdown(modal);
                    spawn_tabs(modal);

                    // Chart tab content.
                    modal
                        .spawn((
                            PowerTabRoot {
                                tab: PowerModalTab::Chart,
                            },
                            Node {
                                width: Val::Percent(100.0),
                                flex_direction: FlexDirection::Column,
                                row_gap: Val::Px(12.0),
                                ..default()
                            },
                        ))
                        .with_children(|chart_root| {
                            spawn_legend(chart_root);
                            spawn_chart_area(chart_root);
                            spawn_x_axis(chart_root);
                            spawn_summary(chart_root);
                        });

                    // Profiles tab content (hidden until selected).
                    modal
                        .spawn((
                            PowerTabRoot {
                                tab: PowerModalTab::Profiles,
                            },
                            Node {
                                width: Val::Percent(100.0),
                                height: Val::Px(360.0),
                                flex_direction: FlexDirection::Column,
                                overflow: Overflow::clip_y(),
                                display: Display::None,
                                ..default()
                            },
                            BackgroundColor(PLOT_BG),
                            BorderRadius::all(Val::Px(6.0)),
                        ))
                        .with_child((
                            Node {
                                width: Val::Percent(100.0),
                                flex_direction: FlexDirection::Column,
                                padding: UiRect::all(Val::Px(10.0)),
                                row_gap: Val::Px(4.0),
                                ..default()
                            },
                            PowerProfilesContainer,
                        ));
                });
        });
}

fn spawn_header(modal: &mut ChildSpawnerCommands) {
    modal
        .spawn(Node {
            width: Val::Percent(100.0),
            justify_content: JustifyContent::SpaceBetween,
            align_items: AlignItems::Center,
            ..default()
        })
        .with_children(|header| {
            header.spawn((
                Text::new("POWER - 24H"),
                TextFont {
                    font_size: 22.0,
                    ..default()
                },
                TextColor(GOLD),
            ));
            header
                .spawn((
                    Button,
                    Node {
                        width: Val::Px(32.0),
                        height: Val::Px(32.0),
                        justify_content: JustifyContent::Center,
                        align_items: AlignItems::Center,
                        ..default()
                    },
                    BackgroundColor(Color::srgba(1.0, 1.0, 1.0, 0.1)),
                    BorderRadius::all(Val::Px(4.0)),
                    PowerStatsCloseButton,
                ))
                .with_child((
                    Text::new("X"),
                    TextFont {
                        font_size: 18.0,
                        ..default()
                    },
                    TextColor(DIM_TEXT),
                ));
        });
}

fn spawn_breakdown(modal: &mut ChildSpawnerCommands) {
    // Source badge.
    modal
        .spawn(Node {
            width: Val::Percent(100.0),
            align_items: AlignItems::Center,
            column_gap: Val::Px(8.0),
            ..default()
        })
        .with_children(|row| {
            row.spawn((
                Text::new("Limited by:"),
                TextFont {
                    font_size: 13.0,
                    ..default()
                },
                TextColor(DIM_TEXT),
            ));
            row.spawn((
                Text::new("--"),
                TextFont {
                    font_size: 13.0,
                    ..default()
                },
                TextColor(BRIGHT_TEXT),
                PowerSourceBadge,
            ));
        });

    // Component values.
    modal
        .spawn(Node {
            width: Val::Percent(100.0),
            justify_content: JustifyContent::SpaceBetween,
            ..default()
        })
        .with_children(|row| {
            spawn_breakdown_stat(row, "Grid limit", PowerBreakdownField::Grid);
            spawn_breakdown_stat(row, "Chargers", PowerBreakdownField::Chargers);
            spawn_breakdown_stat(row, "OCPP caps", PowerBreakdownField::Ocpp);
            spawn_breakdown_stat(row, "Effective", PowerBreakdownField::Effective);
        });

    // Divider under the breakdown.
    modal.spawn((
        Node {
            width: Val::Percent(100.0),
            height: Val::Px(2.0),
            ..default()
        },
        BackgroundColor(BORDER),
    ));
}

fn spawn_breakdown_stat(
    parent: &mut ChildSpawnerCommands,
    caption: &str,
    field: PowerBreakdownField,
) {
    parent
        .spawn(Node {
            flex_direction: FlexDirection::Column,
            row_gap: Val::Px(2.0),
            align_items: AlignItems::Center,
            ..default()
        })
        .with_children(|col| {
            col.spawn((
                Text::new(caption.to_string()),
                TextFont {
                    font_size: 11.0,
                    ..default()
                },
                TextColor(DIM_TEXT),
            ));
            col.spawn((
                Text::new("-- kW"),
                TextFont {
                    font_size: 15.0,
                    ..default()
                },
                TextColor(BRIGHT_TEXT),
                field,
            ));
        });
}

fn spawn_tabs(modal: &mut ChildSpawnerCommands) {
    modal
        .spawn(Node {
            width: Val::Percent(100.0),
            column_gap: Val::Px(6.0),
            ..default()
        })
        .with_children(|tabs| {
            spawn_tab_btn(tabs, "Chart", PowerModalTab::Chart);
            spawn_tab_btn(tabs, "Profiles", PowerModalTab::Profiles);
        });
}

fn spawn_tab_btn(parent: &mut ChildSpawnerCommands, label: &str, tab: PowerModalTab) {
    parent
        .spawn((
            Button,
            Node {
                padding: UiRect::axes(Val::Px(16.0), Val::Px(6.0)),
                ..default()
            },
            BackgroundColor(TAB_INACTIVE),
            BorderRadius::all(Val::Px(4.0)),
            PowerModalTabButton { tab },
        ))
        .with_child((
            Text::new(label.to_string()),
            TextFont {
                font_size: 13.0,
                ..default()
            },
            TextColor(DIM_TEXT),
        ));
}

fn spawn_legend(modal: &mut ChildSpawnerCommands) {
    modal
        .spawn(Node {
            width: Val::Percent(100.0),
            column_gap: Val::Px(20.0),
            align_items: AlignItems::Center,
            ..default()
        })
        .with_children(|legend| {
            spawn_legend_item(legend, DRAW_COLOR, "Actual draw");
            spawn_legend_item(legend, Color::srgb(1.0, 0.55, 0.2), "Site limit");
        });
}

fn spawn_legend_item(parent: &mut ChildSpawnerCommands, color: Color, label: &str) {
    parent
        .spawn(Node {
            column_gap: Val::Px(6.0),
            align_items: AlignItems::Center,
            ..default()
        })
        .with_children(|item| {
            item.spawn((
                Node {
                    width: Val::Px(14.0),
                    height: Val::Px(14.0),
                    ..default()
                },
                BackgroundColor(color),
                BorderRadius::all(Val::Px(3.0)),
            ));
            item.spawn((
                Text::new(label.to_string()),
                TextFont {
                    font_size: 12.0,
                    ..default()
                },
                TextColor(BRIGHT_TEXT),
            ));
        });
}

fn spawn_chart_area(modal: &mut ChildSpawnerCommands) {
    modal
        .spawn(Node {
            width: Val::Percent(100.0),
            height: Val::Px(PLOT_HEIGHT),
            flex_direction: FlexDirection::Row,
            column_gap: Val::Px(6.0),
            ..default()
        })
        .with_children(|area| {
            // Y-axis labels (max / mid / 0)
            area.spawn(Node {
                width: Val::Px(52.0),
                height: Val::Percent(100.0),
                flex_direction: FlexDirection::Column,
                justify_content: JustifyContent::SpaceBetween,
                align_items: AlignItems::End,
                ..default()
            })
            .with_children(|axis| {
                axis.spawn((
                    Text::new("-- kW"),
                    TextFont {
                        font_size: 11.0,
                        ..default()
                    },
                    TextColor(DIM_TEXT),
                    PowerYAxisMaxLabel,
                ));
                axis.spawn((
                    Text::new("-- kW"),
                    TextFont {
                        font_size: 11.0,
                        ..default()
                    },
                    TextColor(DIM_TEXT),
                    PowerYAxisMidLabel,
                ));
                axis.spawn((
                    Text::new("0 kW"),
                    TextFont {
                        font_size: 11.0,
                        ..default()
                    },
                    TextColor(DIM_TEXT),
                ));
            });

            // Plot area (columns fill this; a mid gridline for reference)
            area.spawn((
                Node {
                    flex_grow: 1.0,
                    height: Val::Percent(100.0),
                    flex_direction: FlexDirection::Row,
                    align_items: AlignItems::End,
                    column_gap: Val::Px(0.0),
                    overflow: Overflow::clip(),
                    ..default()
                },
                BackgroundColor(PLOT_BG),
                BorderRadius::all(Val::Px(6.0)),
                PowerChartPlot,
            ))
            .with_children(|plot| {
                // Horizontal mid gridline at 50%.
                plot.spawn((
                    Node {
                        width: Val::Percent(100.0),
                        height: Val::Px(1.0),
                        position_type: PositionType::Absolute,
                        bottom: Val::Percent(50.0),
                        ..default()
                    },
                    BackgroundColor(GRIDLINE),
                ));
            });
        });
}

fn spawn_x_axis(modal: &mut ChildSpawnerCommands) {
    modal
        .spawn(Node {
            width: Val::Percent(100.0),
            padding: UiRect::left(Val::Px(58.0)),
            justify_content: JustifyContent::SpaceBetween,
            ..default()
        })
        .with_children(|axis| {
            for label in ["0:00", "6:00", "12:00", "18:00", "24:00"] {
                axis.spawn((
                    Text::new(label),
                    TextFont {
                        font_size: 11.0,
                        ..default()
                    },
                    TextColor(DIM_TEXT),
                ));
            }
        });
}

fn spawn_summary(modal: &mut ChildSpawnerCommands) {
    modal.spawn((
        Node {
            width: Val::Percent(100.0),
            height: Val::Px(2.0),
            margin: UiRect::vertical(Val::Px(2.0)),
            ..default()
        },
        BackgroundColor(BORDER),
    ));
    modal
        .spawn(Node {
            width: Val::Percent(100.0),
            justify_content: JustifyContent::SpaceBetween,
            ..default()
        })
        .with_children(|row| {
            spawn_summary_stat(row, "Draw now", PowerSummaryDrawLabel);
            spawn_summary_stat(row, "Limit now", PowerSummaryLimitLabel);
            spawn_summary_stat(row, "Peak draw", PowerSummaryPeakDrawLabel);
            spawn_summary_stat(row, "Peak limit", PowerSummaryPeakLimitLabel);
        });
}

fn spawn_summary_stat<C: Component>(parent: &mut ChildSpawnerCommands, caption: &str, marker: C) {
    parent
        .spawn(Node {
            flex_direction: FlexDirection::Column,
            row_gap: Val::Px(2.0),
            align_items: AlignItems::Center,
            ..default()
        })
        .with_children(|col| {
            col.spawn((
                Text::new(caption.to_string()),
                TextFont {
                    font_size: 11.0,
                    ..default()
                },
                TextColor(DIM_TEXT),
            ));
            col.spawn((
                Text::new("-- kW"),
                TextFont {
                    font_size: 16.0,
                    ..default()
                },
                TextColor(BRIGHT_TEXT),
                marker,
            ));
        });
}

pub fn despawn_power_stats_modal(
    mut commands: Commands,
    modal_state: Res<PowerStatsModalState>,
    query: Query<Entity, With<PowerStatsModalUI>>,
) {
    if !modal_state.is_open {
        for entity in &query {
            commands.entity(entity).try_despawn();
        }
    }
}

// ============ Interaction ============

pub fn handle_power_stats_close_button(
    interaction: Query<&Interaction, (Changed<Interaction>, With<PowerStatsCloseButton>)>,
    mut modal_state: ResMut<PowerStatsModalState>,
) {
    for i in &interaction {
        if *i == Interaction::Pressed {
            modal_state.close();
        }
    }
}

pub fn handle_power_stats_keyboard(
    keys: Res<ButtonInput<KeyCode>>,
    mut modal_state: ResMut<PowerStatsModalState>,
) {
    if modal_state.is_open && keys.just_pressed(KeyCode::Escape) {
        modal_state.close();
    }
}

pub fn handle_power_stats_tab_buttons(
    interaction: Query<(&Interaction, &PowerModalTabButton), Changed<Interaction>>,
    mut modal_state: ResMut<PowerStatsModalState>,
) {
    for (i, btn) in &interaction {
        if *i == Interaction::Pressed {
            modal_state.active_tab = btn.tab;
        }
    }
}

/// Update tab button colors and toggle which tab's content is visible.
pub fn update_power_stats_tabs(
    modal_state: Res<PowerStatsModalState>,
    mut buttons: Query<(&PowerModalTabButton, &mut BackgroundColor)>,
    mut roots: Query<(&PowerTabRoot, &mut Node)>,
) {
    if !modal_state.is_open {
        return;
    }
    for (btn, mut bg) in &mut buttons {
        *bg = if btn.tab == modal_state.active_tab {
            BackgroundColor(TAB_ACTIVE)
        } else {
            BackgroundColor(TAB_INACTIVE)
        };
    }
    for (root, mut node) in &mut roots {
        node.display = if root.tab == modal_state.active_tab {
            Display::Flex
        } else {
            Display::None
        };
    }
}

/// Update the limit-breakdown strip (grid / chargers / ocpp / effective + source).
pub fn update_power_stats_breakdown(
    modal_state: Res<PowerStatsModalState>,
    multi_site: Res<MultiSiteManager>,
    chargers: Query<(&Charger, &BelongsToSite)>,
    mut fields: Query<(&PowerBreakdownField, &mut Text), Without<PowerSourceBadge>>,
    mut badge: Query<(&mut Text, &mut TextColor), With<PowerSourceBadge>>,
) {
    if !modal_state.is_open {
        return;
    }
    let Some(site) = multi_site.active_site() else {
        return;
    };
    let dispatch = site.dispatch_limit_kva();
    let items = chargers
        .iter()
        .filter(|(_, belongs)| belongs.site_id == site.id)
        .map(|(charger, _)| {
            (
                charger.is_disabled,
                charger.ocpp_limit_kw,
                charger.rated_power_kw,
            )
        });
    let breakdown = site_limit_breakdown(items, dispatch);

    for (field, mut text) in &mut fields {
        let value = match field {
            PowerBreakdownField::Grid => breakdown.grid_dispatch_kw,
            PowerBreakdownField::Chargers => breakdown.charger_sum_kw,
            PowerBreakdownField::Ocpp => breakdown.ocpp_capped_sum_kw,
            PowerBreakdownField::Effective => breakdown.effective_kw,
        };
        let s = match field {
            PowerBreakdownField::Ocpp => {
                format!("{value:.0} kW ({})", breakdown.ocpp_charger_count)
            }
            _ => format!("{value:.0} kW"),
        };
        *text = Text::new(s);
    }

    if let Ok((mut text, mut color)) = badge.single_mut() {
        *text = Text::new(breakdown.source.label());
        *color = TextColor(match breakdown.source {
            LimitSource::Grid => SRC_GRID,
            LimitSource::Ocpp => SRC_OCPP,
            LimitSource::Hardware => SRC_HARDWARE,
        });
    }
}

/// Rebuild the Profiles tab list when the stored profile count changes.
pub fn update_power_stats_profiles(
    mut commands: Commands,
    mut modal_state: ResMut<PowerStatsModalState>,
    store: Res<ChargingProfileStore>,
    queue: Res<OcppMessageQueue>,
    game_clock: Res<GameClock>,
    multi_site: Res<MultiSiteManager>,
    chargers: Query<(Entity, &Charger, &BelongsToSite)>,
    container: Query<Entity, With<PowerProfilesContainer>>,
    old_rows: Query<Entity, With<PowerProfileRow>>,
) {
    if !modal_state.is_open || modal_state.active_tab != PowerModalTab::Profiles {
        return;
    }
    let signature = (store.total_profile_count(), modal_state.active_tab);
    if signature == modal_state.last_profile_signature {
        return;
    }
    modal_state.last_profile_signature = signature;

    let Ok(container_entity) = container.single() else {
        return;
    };
    for entity in &old_rows {
        commands.entity(entity).try_despawn();
    }

    let Some(site) = multi_site.active_site() else {
        return;
    };
    let now = queue.game_time_to_utc(game_clock.total_game_time);

    let mut site_chargers: Vec<(Entity, &Charger)> = chargers
        .iter()
        .filter(|(_, _, belongs)| belongs.site_id == site.id)
        .map(|(entity, charger, _)| (entity, charger))
        .collect();
    site_chargers.sort_by(|a, b| a.1.id.cmp(&b.1.id));

    let mut any = false;
    for (entity, charger) in site_chargers {
        let tx_start = queue
            .charger_state
            .get(&entity)
            .and_then(|s| s.tx_start_total_game_time)
            .map(|t| queue.game_time_to_utc(t));
        let rows = store.profile_rows(&charger.id, now, tx_start);
        if rows.is_empty() {
            continue;
        }
        any = true;
        spawn_profile_charger_header(&mut commands, container_entity, charger);
        for row in &rows {
            spawn_profile_row(&mut commands, container_entity, row);
        }
    }

    if !any {
        let empty = commands
            .spawn((
                PowerProfileRow,
                Text::new("No charging profiles set - limit is grid/hardware-bound."),
                TextFont {
                    font_size: 13.0,
                    ..default()
                },
                TextColor(DIM_TEXT),
                Node {
                    margin: UiRect::all(Val::Px(8.0)),
                    ..default()
                },
            ))
            .id();
        commands.entity(container_entity).add_child(empty);
    }
}

fn purpose_short(purpose: &crate::ocpp::types::ChargingProfilePurposeType) -> &'static str {
    use crate::ocpp::types::ChargingProfilePurposeType as P;
    match purpose {
        P::ChargePointMaxProfile => "CP-Max",
        P::TxDefaultProfile => "TxDefault",
        P::TxProfile => "Tx",
    }
}

fn spawn_profile_charger_header(commands: &mut Commands, container: Entity, charger: &Charger) {
    let cap = match charger.ocpp_limit_kw {
        Some(kw) => format!("cap {kw:.0} kW"),
        None => "no cap".to_string(),
    };
    let row = commands
        .spawn((
            PowerProfileRow,
            Node {
                width: Val::Percent(100.0),
                justify_content: JustifyContent::SpaceBetween,
                padding: UiRect::axes(Val::Px(6.0), Val::Px(5.0)),
                margin: UiRect::top(Val::Px(6.0)),
                ..default()
            },
            BackgroundColor(Color::srgba(1.0, 1.0, 1.0, 0.06)),
            BorderRadius::all(Val::Px(4.0)),
        ))
        .with_children(|r| {
            r.spawn((
                Text::new(charger.id.clone()),
                TextFont {
                    font_size: 13.0,
                    ..default()
                },
                TextColor(GOLD),
            ));
            r.spawn((
                Text::new(cap),
                TextFont {
                    font_size: 13.0,
                    ..default()
                },
                TextColor(BRIGHT_TEXT),
            ));
        })
        .id();
    commands.entity(container).add_child(row);
}

fn spawn_profile_row(commands: &mut Commands, container: Entity, row: &ActiveProfileRow) {
    let active = match row.active_limit_kw {
        Some(kw) => format!("{kw:.0} kW"),
        None => "inactive".to_string(),
    };
    let (active_color, kind) = match row.active_limit_kw {
        Some(_) => (BRIGHT_TEXT, format!("{:?}", row.kind)),
        None => (DIM_TEXT, format!("{:?}", row.kind)),
    };
    let label = format!(
        "{}  stack {}  {}  #{}",
        purpose_short(&row.purpose),
        row.stack_level,
        kind,
        row.profile_id,
    );
    let entity = commands
        .spawn((
            PowerProfileRow,
            Node {
                width: Val::Percent(100.0),
                justify_content: JustifyContent::SpaceBetween,
                padding: UiRect::axes(Val::Px(12.0), Val::Px(3.0)),
                border: UiRect::bottom(Val::Px(1.0)),
                ..default()
            },
            BorderColor::all(ROW_BORDER),
        ))
        .with_children(|r| {
            r.spawn((
                Text::new(label),
                TextFont {
                    font_size: 12.0,
                    ..default()
                },
                TextColor(DIM_TEXT),
            ));
            r.spawn((
                Text::new(active),
                TextFont {
                    font_size: 12.0,
                    ..default()
                },
                TextColor(active_color),
            ));
        })
        .id();
    commands.entity(container).add_child(entity);
}

// ============ Chart rebuild + live summary ============

/// Downsample raw samples to at most `max_cols` columns by averaging buckets.
/// Returns `(draw_kw, limit_kw)` per column, oldest first.
fn downsample(samples: &[PowerSample], max_cols: usize) -> Vec<(f32, f32)> {
    if samples.is_empty() || max_cols == 0 {
        return Vec::new();
    }
    if samples.len() <= max_cols {
        return samples.iter().map(|s| (s.draw_kw, s.limit_kw)).collect();
    }
    let mut out = Vec::with_capacity(max_cols);
    for c in 0..max_cols {
        let start = c * samples.len() / max_cols;
        let end = ((c + 1) * samples.len() / max_cols).max(start + 1);
        let slice = &samples[start..end.min(samples.len())];
        let n = slice.len().max(1) as f32;
        let draw = slice.iter().map(|s| s.draw_kw).sum::<f32>() / n;
        let limit = slice.iter().map(|s| s.limit_kw).sum::<f32>() / n;
        out.push((draw, limit));
    }
    out
}

/// Rebuild the chart columns when the sample count changes while open.
pub fn update_power_stats_chart(
    mut commands: Commands,
    mut modal_state: ResMut<PowerStatsModalState>,
    multi_site: Res<MultiSiteManager>,
    plot: Query<Entity, With<PowerChartPlot>>,
    old_columns: Query<Entity, With<PowerChartColumn>>,
    mut y_max: Query<&mut Text, (With<PowerYAxisMaxLabel>, Without<PowerYAxisMidLabel>)>,
    mut y_mid: Query<&mut Text, (With<PowerYAxisMidLabel>, Without<PowerYAxisMaxLabel>)>,
) {
    if !modal_state.is_open {
        return;
    }
    let Some(site) = multi_site.active_site() else {
        return;
    };
    let sample_count = site.power_history.samples.len();
    if sample_count == modal_state.last_sample_count {
        return;
    }
    modal_state.last_sample_count = sample_count;

    let Ok(plot_entity) = plot.single() else {
        return;
    };

    // Clear existing columns (keep the gridline, which has no marker).
    for entity in &old_columns {
        commands.entity(entity).try_despawn();
    }

    let cols = downsample(&site.power_history.samples, MAX_COLUMNS);

    // Scale to the max of both series (round up to a tidy value, min 10 kW).
    let raw_max = cols
        .iter()
        .map(|(d, l)| d.max(*l))
        .fold(0.0_f32, f32::max)
        .max(site.power_history.peak_limit_kw);
    let max_kw = nice_ceiling(raw_max.max(10.0));

    if let Ok(mut t) = y_max.single_mut() {
        *t = Text::new(format!("{max_kw:.0} kW"));
    }
    if let Ok(mut t) = y_mid.single_mut() {
        *t = Text::new(format!("{:.0} kW", max_kw / 2.0));
    }

    for (draw, limit) in cols {
        let draw_pct = (draw / max_kw * 100.0).clamp(0.0, 100.0);
        let limit_pct = (limit / max_kw * 100.0).clamp(0.0, 100.0);
        let column = commands
            .spawn((
                PowerChartColumn,
                Node {
                    flex_grow: 1.0,
                    height: Val::Percent(100.0),
                    min_width: Val::Px(1.0),
                    position_type: PositionType::Relative,
                    ..default()
                },
            ))
            .with_children(|col| {
                // Limit bar (behind) then draw bar (in front), both anchored bottom.
                col.spawn((
                    Node {
                        width: Val::Percent(100.0),
                        height: Val::Percent(limit_pct),
                        position_type: PositionType::Absolute,
                        bottom: Val::Px(0.0),
                        ..default()
                    },
                    BackgroundColor(LIMIT_COLOR),
                ));
                col.spawn((
                    Node {
                        width: Val::Percent(100.0),
                        height: Val::Percent(draw_pct),
                        position_type: PositionType::Absolute,
                        bottom: Val::Px(0.0),
                        ..default()
                    },
                    BackgroundColor(DRAW_COLOR),
                ));
            })
            .id();
        commands.entity(plot_entity).add_child(column);
    }
}

/// Live-update the summary stat labels every frame while open.
pub fn update_power_stats_summary(
    modal_state: Res<PowerStatsModalState>,
    multi_site: Res<MultiSiteManager>,
    mut draw_label: Query<
        &mut Text,
        (
            With<PowerSummaryDrawLabel>,
            Without<PowerSummaryLimitLabel>,
            Without<PowerSummaryPeakDrawLabel>,
            Without<PowerSummaryPeakLimitLabel>,
        ),
    >,
    mut limit_label: Query<
        &mut Text,
        (
            With<PowerSummaryLimitLabel>,
            Without<PowerSummaryDrawLabel>,
            Without<PowerSummaryPeakDrawLabel>,
            Without<PowerSummaryPeakLimitLabel>,
        ),
    >,
    mut peak_draw_label: Query<
        &mut Text,
        (
            With<PowerSummaryPeakDrawLabel>,
            Without<PowerSummaryDrawLabel>,
            Without<PowerSummaryLimitLabel>,
            Without<PowerSummaryPeakLimitLabel>,
        ),
    >,
    mut peak_limit_label: Query<
        &mut Text,
        (
            With<PowerSummaryPeakLimitLabel>,
            Without<PowerSummaryDrawLabel>,
            Without<PowerSummaryLimitLabel>,
            Without<PowerSummaryPeakDrawLabel>,
        ),
    >,
) {
    if !modal_state.is_open {
        return;
    }
    let Some(site) = multi_site.active_site() else {
        return;
    };
    let history = &site.power_history;
    let (draw_now, limit_now) = history
        .samples
        .last()
        .map(|s| (s.draw_kw, s.limit_kw))
        .unwrap_or((0.0, 0.0));

    if let Ok(mut t) = draw_label.single_mut() {
        *t = Text::new(format!("{draw_now:.0} kW"));
    }
    if let Ok(mut t) = limit_label.single_mut() {
        *t = Text::new(format!("{limit_now:.0} kW"));
    }
    if let Ok(mut t) = peak_draw_label.single_mut() {
        *t = Text::new(format!("{:.0} kW", history.peak_draw_kw));
    }
    if let Ok(mut t) = peak_limit_label.single_mut() {
        *t = Text::new(format!("{:.0} kW", history.peak_limit_kw));
    }
}

/// Round up to a tidy axis maximum (1/2/5 x 10^n).
fn nice_ceiling(value: f32) -> f32 {
    if value <= 0.0 {
        return 10.0;
    }
    let exp = value.log10().floor();
    let pow = 10.0_f32.powf(exp);
    let frac = value / pow;
    let nice = if frac <= 1.0 {
        1.0
    } else if frac <= 2.0 {
        2.0
    } else if frac <= 5.0 {
        5.0
    } else {
        10.0
    };
    nice * pow
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample(draw: f32, limit: f32) -> PowerSample {
        PowerSample {
            game_time_secs: 0.0,
            draw_kw: draw,
            limit_kw: limit,
        }
    }

    #[test]
    fn downsample_passthrough_when_small() {
        let samples = vec![sample(1.0, 10.0), sample(2.0, 10.0)];
        let out = downsample(&samples, 240);
        assert_eq!(out.len(), 2);
        assert_eq!(out[0], (1.0, 10.0));
    }

    #[test]
    fn downsample_buckets_average_when_large() {
        let samples: Vec<PowerSample> = (0..1000).map(|i| sample(i as f32, 100.0)).collect();
        let out = downsample(&samples, 100);
        assert_eq!(out.len(), 100);
        // Each column's limit is constant 100.
        assert!(out.iter().all(|(_, l)| (*l - 100.0).abs() < 1e-3));
    }

    #[test]
    fn nice_ceiling_rounds_up() {
        assert_eq!(nice_ceiling(9.0), 10.0);
        assert_eq!(nice_ceiling(11.0), 20.0);
        assert_eq!(nice_ceiling(21.0), 50.0);
        assert_eq!(nice_ceiling(51.0), 100.0);
        assert_eq!(nice_ceiling(150.0), 200.0);
    }
}
