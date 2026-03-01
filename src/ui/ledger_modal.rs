//! Ledger modal -- displays the double-entry accounting journal and account
//! balance summary in a scrollable popup.

use bevy::ecs::hierarchy::ChildSpawnerCommands;
use bevy::prelude::*;

use crate::resources::game_state::GameState;
use crate::resources::ledger::Account;

// ============ Components ============

#[derive(Component)]
pub struct LedgerModalUI;

#[derive(Component)]
pub struct LedgerCloseButton;

#[derive(Component)]
pub struct LedgerTabButton {
    pub tab: LedgerTab,
}

#[derive(Component)]
pub struct LedgerContentContainer;

#[derive(Component)]
pub struct LedgerEntryRow;

// ============ Resource ============

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LedgerTab {
    Journal,
    Balances,
}

#[derive(Resource, Debug)]
pub struct LedgerModalState {
    pub is_open: bool,
    pub active_tab: LedgerTab,
    last_txn_count: usize,
    last_tab: LedgerTab,
}

impl Default for LedgerModalState {
    fn default() -> Self {
        Self {
            is_open: false,
            active_tab: LedgerTab::Journal,
            last_txn_count: 0,
            last_tab: LedgerTab::Journal,
        }
    }
}

impl LedgerModalState {
    pub fn toggle(&mut self) {
        self.is_open = !self.is_open;
    }

    pub fn close(&mut self) {
        self.is_open = false;
    }
}

// ============ Colors ============

const GREEN: Color = Color::srgb(0.2, 0.85, 0.4);
const RED: Color = Color::srgb(0.9, 0.3, 0.3);
const GOLD: Color = Color::srgb(1.0, 0.84, 0.0);
const DIM_TEXT: Color = Color::srgb(0.6, 0.6, 0.6);
const BRIGHT_TEXT: Color = Color::srgb(0.9, 0.9, 0.9);
const PANEL_BG: Color = Color::srgb(0.12, 0.14, 0.18);
const BORDER: Color = Color::srgb(0.3, 0.35, 0.4);
const ROW_BG: Color = Color::srgba(0.0, 0.0, 0.0, 0.2);
const TAB_ACTIVE: Color = Color::srgba(1.0, 1.0, 1.0, 0.15);
const TAB_INACTIVE: Color = Color::srgba(1.0, 1.0, 1.0, 0.05);

// ============ Spawn ============

pub fn spawn_ledger_modal(
    mut commands: Commands,
    modal_state: Res<LedgerModalState>,
    existing: Query<Entity, With<LedgerModalUI>>,
) {
    if !modal_state.is_open || !existing.is_empty() {
        return;
    }

    commands
        .spawn((
            LedgerModalUI,
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
                        width: Val::Px(700.0),
                        max_height: Val::Percent(85.0),
                        flex_direction: FlexDirection::Column,
                        padding: UiRect::all(Val::Px(24.0)),
                        row_gap: Val::Px(10.0),
                        ..default()
                    },
                    BackgroundColor(PANEL_BG),
                    BorderColor::all(BORDER),
                    BorderRadius::all(Val::Px(12.0)),
                ))
                .with_children(|modal| {
                    // Header
                    modal
                        .spawn(Node {
                            width: Val::Percent(100.0),
                            justify_content: JustifyContent::SpaceBetween,
                            align_items: AlignItems::Center,
                            margin: UiRect::bottom(Val::Px(4.0)),
                            ..default()
                        })
                        .with_children(|header| {
                            header.spawn((
                                Text::new("GENERAL LEDGER"),
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
                                    LedgerCloseButton,
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

                    // Tab row
                    modal
                        .spawn(Node {
                            width: Val::Percent(100.0),
                            column_gap: Val::Px(6.0),
                            margin: UiRect::bottom(Val::Px(4.0)),
                            ..default()
                        })
                        .with_children(|tabs| {
                            spawn_tab_btn(tabs, "Journal", LedgerTab::Journal);
                            spawn_tab_btn(tabs, "Balances", LedgerTab::Balances);
                        });

                    // Divider
                    modal.spawn((
                        Node {
                            width: Val::Percent(100.0),
                            height: Val::Px(2.0),
                            ..default()
                        },
                        BackgroundColor(BORDER),
                    ));

                    // Scrollable content container
                    modal
                        .spawn((
                            Node {
                                width: Val::Percent(100.0),
                                height: Val::Px(500.0),
                                flex_direction: FlexDirection::Column,
                                overflow: Overflow::clip_y(),
                                ..default()
                            },
                            BackgroundColor(Color::srgba(0.0, 0.0, 0.0, 0.3)),
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
                            LedgerContentContainer,
                        ));
                });
        });
}

fn spawn_tab_btn(parent: &mut ChildSpawnerCommands, label: &str, tab: LedgerTab) {
    parent
        .spawn((
            Button,
            Node {
                padding: UiRect::axes(Val::Px(16.0), Val::Px(8.0)),
                ..default()
            },
            BackgroundColor(TAB_INACTIVE),
            BorderRadius::all(Val::Px(4.0)),
            LedgerTabButton { tab },
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

// ============ Despawn ============

pub fn despawn_ledger_modal(
    mut commands: Commands,
    modal_state: Res<LedgerModalState>,
    query: Query<Entity, With<LedgerModalUI>>,
) {
    if !modal_state.is_open {
        for entity in &query {
            commands.entity(entity).try_despawn();
        }
    }
}

// ============ Close button ============

pub fn handle_ledger_close_button(
    interaction: Query<&Interaction, (Changed<Interaction>, With<LedgerCloseButton>)>,
    mut modal_state: ResMut<LedgerModalState>,
) {
    for i in &interaction {
        if *i == Interaction::Pressed {
            modal_state.close();
        }
    }
}

// ============ Tab switching ============

pub fn handle_ledger_tab_buttons(
    interaction: Query<(&Interaction, &LedgerTabButton), Changed<Interaction>>,
    mut modal_state: ResMut<LedgerModalState>,
) {
    for (i, tab_btn) in &interaction {
        if *i == Interaction::Pressed {
            modal_state.active_tab = tab_btn.tab;
        }
    }
}

/// Update tab button visual state.
pub fn update_tab_visuals(
    modal_state: Res<LedgerModalState>,
    mut tabs: Query<(&LedgerTabButton, &mut BackgroundColor)>,
) {
    if !modal_state.is_open {
        return;
    }
    for (tab_btn, mut bg) in &mut tabs {
        *bg = if tab_btn.tab == modal_state.active_tab {
            BackgroundColor(TAB_ACTIVE)
        } else {
            BackgroundColor(TAB_INACTIVE)
        };
    }
}

// ============ Keyboard shortcut ============

pub fn handle_ledger_keyboard(
    keys: Res<ButtonInput<KeyCode>>,
    mut modal_state: ResMut<LedgerModalState>,
) {
    if keys.just_pressed(KeyCode::KeyL) {
        modal_state.toggle();
    }
}

// ============ Content update ============

pub fn update_ledger_content(
    mut commands: Commands,
    game_state: Res<GameState>,
    mut modal_state: ResMut<LedgerModalState>,
    container: Query<Entity, With<LedgerContentContainer>>,
    old_rows: Query<Entity, With<LedgerEntryRow>>,
) {
    if !modal_state.is_open {
        return;
    }

    let txn_count = game_state.ledger.transaction_count();
    let tab = modal_state.active_tab;

    if txn_count == modal_state.last_txn_count && tab == modal_state.last_tab {
        return;
    }
    modal_state.last_txn_count = txn_count;
    modal_state.last_tab = tab;

    // Despawn old rows
    for entity in &old_rows {
        commands.entity(entity).try_despawn();
    }

    let Ok(container_entity) = container.single() else {
        return;
    };

    match tab {
        LedgerTab::Journal => build_journal_view(&mut commands, container_entity, &game_state),
        LedgerTab::Balances => build_balances_view(&mut commands, container_entity, &game_state),
    }
}

// ============ Journal view ============

fn build_journal_view(commands: &mut Commands, container: Entity, game_state: &GameState) {
    let ledger = &game_state.ledger;

    // Header row
    let header = commands
        .spawn((
            LedgerEntryRow,
            Node {
                width: Val::Percent(100.0),
                justify_content: JustifyContent::SpaceBetween,
                padding: UiRect::axes(Val::Px(8.0), Val::Px(6.0)),
                ..default()
            },
        ))
        .with_children(|row| {
            let cols = [
                ("Date", 80.0),
                ("Description", 280.0),
                ("Amount", 100.0),
                ("Balance", 100.0),
            ];
            for (label, w) in cols {
                row.spawn((
                    Text::new(label.to_string()),
                    TextFont {
                        font_size: 11.0,
                        ..default()
                    },
                    TextColor(DIM_TEXT),
                    Node {
                        width: Val::Px(w),
                        ..default()
                    },
                ));
            }
        })
        .id();
    commands.entity(container).add_child(header);

    // Compute running balance per transaction
    let mut running_balance = 0.0_f64;
    struct TxnRow {
        date: String,
        narration: String,
        cash_amount: f64,
        balance_after: f64,
    }
    let mut rows: Vec<TxnRow> = Vec::with_capacity(ledger.journal.len());

    for txn in &ledger.journal {
        let mut cash_delta = 0.0_f64;
        for posting in &txn.postings {
            if posting.account.as_ref() == Account::Cash.beancount_name()
                && let Some(amt) = posting.amount()
            {
                use rust_decimal::prelude::ToPrimitive;
                cash_delta += amt.number.to_f64().unwrap_or(0.0);
            }
        }
        running_balance += cash_delta;
        rows.push(TxnRow {
            date: txn.date.format("%m/%d").to_string(),
            narration: txn.narration.as_str().to_string(),
            cash_amount: cash_delta,
            balance_after: running_balance,
        });
    }

    // Display most recent first
    for txn_row in rows.iter().rev() {
        let row_entity = commands
            .spawn((
                LedgerEntryRow,
                Node {
                    width: Val::Percent(100.0),
                    justify_content: JustifyContent::SpaceBetween,
                    padding: UiRect::axes(Val::Px(8.0), Val::Px(4.0)),
                    border: UiRect::bottom(Val::Px(1.0)),
                    ..default()
                },
                BorderColor::all(Color::srgba(1.0, 1.0, 1.0, 0.05)),
                BackgroundColor(ROW_BG),
            ))
            .with_children(|row| {
                // Date
                row.spawn((
                    Text::new(txn_row.date.clone()),
                    TextFont {
                        font_size: 12.0,
                        ..default()
                    },
                    TextColor(DIM_TEXT),
                    Node {
                        width: Val::Px(80.0),
                        ..default()
                    },
                ));
                // Description
                row.spawn((
                    Text::new(txn_row.narration.clone()),
                    TextFont {
                        font_size: 12.0,
                        ..default()
                    },
                    TextColor(BRIGHT_TEXT),
                    Node {
                        width: Val::Px(280.0),
                        ..default()
                    },
                ));
                // Amount
                let amount_color = if txn_row.cash_amount >= 0.0 {
                    GREEN
                } else {
                    RED
                };
                let sign = if txn_row.cash_amount >= 0.0 { "+" } else { "" };
                row.spawn((
                    Text::new(format!("{}${:.0}", sign, txn_row.cash_amount)),
                    TextFont {
                        font_size: 12.0,
                        ..default()
                    },
                    TextColor(amount_color),
                    Node {
                        width: Val::Px(100.0),
                        ..default()
                    },
                ));
                // Balance
                row.spawn((
                    Text::new(format!("${:.0}", txn_row.balance_after)),
                    TextFont {
                        font_size: 12.0,
                        ..default()
                    },
                    TextColor(BRIGHT_TEXT),
                    Node {
                        width: Val::Px(100.0),
                        ..default()
                    },
                ));
            })
            .id();
        commands.entity(container).add_child(row_entity);
    }
}

// ============ Balances view ============

fn build_balances_view(commands: &mut Commands, container: Entity, game_state: &GameState) {
    let ledger = &game_state.ledger;

    // Assets section
    spawn_section_header(commands, container, "ASSETS");
    spawn_balance_row(
        commands,
        container,
        "Cash",
        ledger.cash_balance_f32(),
        BRIGHT_TEXT,
    );
    spawn_balance_row(
        commands,
        container,
        "Equipment",
        decimal_to_f32(ledger.equipment_balance()),
        BRIGHT_TEXT,
    );

    // Income section
    spawn_section_header(commands, container, "INCOME");
    for &acct in Account::ALL_INCOME {
        let bal = ledger.account_balance(acct);
        let val = decimal_to_f32(bal).abs();
        if val > 0.01 {
            spawn_balance_row(commands, container, acct.display_label(), val, GREEN);
        }
    }

    // Expenses section
    spawn_section_header(commands, container, "EXPENSES");
    for &acct in Account::ALL_EXPENSES {
        let bal = ledger.account_balance(acct);
        let val = decimal_to_f32(bal);
        let color = if acct.is_contra() { GREEN } else { RED };
        if val.abs() > 0.01 {
            spawn_balance_row(commands, container, acct.display_label(), val.abs(), color);
        }
    }

    // Verification line
    let balanced = ledger.is_balanced();
    let verify_text = if balanced {
        "Debits = Credits (balanced)"
    } else {
        "WARNING: Debits != Credits"
    };
    let verify_color = if balanced { GREEN } else { RED };

    let verify = commands
        .spawn((
            LedgerEntryRow,
            Node {
                width: Val::Percent(100.0),
                justify_content: JustifyContent::Center,
                padding: UiRect::vertical(Val::Px(10.0)),
                margin: UiRect::top(Val::Px(8.0)),
                border: UiRect::top(Val::Px(1.0)),
                ..default()
            },
            BorderColor::all(BORDER),
        ))
        .with_child((
            Text::new(verify_text.to_string()),
            TextFont {
                font_size: 13.0,
                ..default()
            },
            TextColor(verify_color),
        ))
        .id();
    commands.entity(container).add_child(verify);
}

fn spawn_section_header(commands: &mut Commands, container: Entity, label: &str) {
    let entity = commands
        .spawn((
            LedgerEntryRow,
            Node {
                width: Val::Percent(100.0),
                padding: UiRect::new(Val::Px(8.0), Val::Px(8.0), Val::Px(10.0), Val::Px(4.0)),
                ..default()
            },
        ))
        .with_child((
            Text::new(label.to_string()),
            TextFont {
                font_size: 12.0,
                ..default()
            },
            TextColor(GOLD),
        ))
        .id();
    commands.entity(container).add_child(entity);
}

fn spawn_balance_row(
    commands: &mut Commands,
    container: Entity,
    label: &str,
    value: f32,
    color: Color,
) {
    let entity = commands
        .spawn((
            LedgerEntryRow,
            Node {
                width: Val::Percent(100.0),
                justify_content: JustifyContent::SpaceBetween,
                padding: UiRect::axes(Val::Px(16.0), Val::Px(4.0)),
                ..default()
            },
        ))
        .with_children(|row| {
            row.spawn((
                Text::new(label.to_string()),
                TextFont {
                    font_size: 12.0,
                    ..default()
                },
                TextColor(BRIGHT_TEXT),
            ));
            row.spawn((
                Text::new(format!("${:.2}", value)),
                TextFont {
                    font_size: 12.0,
                    ..default()
                },
                TextColor(color),
            ));
        })
        .id();
    commands.entity(container).add_child(entity);
}

fn decimal_to_f32(value: rust_decimal::Decimal) -> f32 {
    use rust_decimal::prelude::ToPrimitive;
    value.to_f32().unwrap_or(0.0)
}
