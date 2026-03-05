use bevy::ecs::hierarchy::ChildSpawnerCommands;
use bevy::prelude::*;

use crate::states::KpiToggleButton;
use crate::states::day_end::helpers::{
    energy_margin_insight, format_delta, format_int_delta, operations_insight, reputation_insight,
    spawn_indented_row, spawn_insight_row, spawn_prominent_stat_row, spawn_section_divider,
    spawn_section_header, spawn_stat_row, spawn_stat_row_with_hint, unit_economy_verdict,
};
use crate::states::day_end::report::DayEndReport;
use crate::states::day_end::{KpiCollapsed, KpiExpanded};

pub(crate) fn spawn_header_section(parent: &mut ChildSpawnerCommands, report: &DayEndReport) {
    parent
        .spawn(Node {
            width: Val::Percent(100.0),
            flex_direction: FlexDirection::Column,
            align_items: AlignItems::Center,
            row_gap: Val::Px(4.0),
            ..default()
        })
        .with_children(|header| {
            header.spawn((
                Text::new(format!("DAY {} COMPLETE", report.day)),
                TextFont {
                    font_size: 28.0,
                    ..default()
                },
                TextColor(Color::srgb(0.4, 0.8, 1.0)),
            ));
            header.spawn((
                Text::new("\"Daily Operations Report\""),
                TextFont {
                    font_size: 14.0,
                    ..default()
                },
                TextColor(Color::srgb(0.6, 0.6, 0.6)),
            ));
        });
}

pub(crate) fn spawn_avatar_section(parent: &mut ChildSpawnerCommands, report: &DayEndReport) {
    parent
        .spawn(Node {
            width: Val::Percent(100.0),
            flex_direction: FlexDirection::Row,
            column_gap: Val::Px(16.0),
            align_items: AlignItems::Center,
            padding: UiRect::vertical(Val::Px(8.0)),
            ..default()
        })
        .with_children(|avatar_row| {
            avatar_row.spawn((
                ImageNode::new(report.avatar_handle.clone()),
                Node {
                    width: Val::Px(64.0),
                    height: Val::Px(64.0),
                    border: UiRect::all(Val::Px(2.0)),
                    ..default()
                },
                BorderColor::all(Color::srgb(0.3, 0.6, 0.8)),
                BorderRadius::all(Val::Px(4.0)),
            ));

            avatar_row
                .spawn(Node {
                    flex_direction: FlexDirection::Column,
                    row_gap: Val::Px(4.0),
                    ..default()
                })
                .with_children(|info| {
                    info.spawn((
                        Text::new(&report.char_name),
                        TextFont {
                            font_size: 20.0,
                            ..default()
                        },
                        TextColor(Color::WHITE),
                    ));
                    info.spawn((
                        Text::new(format!("Role: {}", report.char_role)),
                        TextFont {
                            font_size: 14.0,
                            ..default()
                        },
                        TextColor(Color::srgb(0.5, 0.7, 0.5)),
                    ));
                });
        });
}

pub(crate) fn spawn_kpi_header(parent: &mut ChildSpawnerCommands) {
    // Section header row with toggle button
    parent
        .spawn(Node {
            width: Val::Percent(100.0),
            flex_direction: FlexDirection::Row,
            justify_content: JustifyContent::SpaceBetween,
            align_items: AlignItems::Center,
            ..default()
        })
        .with_children(|kpi_header| {
            kpi_header.spawn((
                Text::new("KPI SNAPSHOT"),
                TextFont {
                    font_size: 14.0,
                    ..default()
                },
                TextColor(Color::srgb(0.5, 0.5, 0.5)),
            ));

            kpi_header
                .spawn((
                    Button,
                    Node {
                        padding: UiRect::new(
                            Val::Px(8.0),
                            Val::Px(8.0),
                            Val::Px(4.0),
                            Val::Px(4.0),
                        ),
                        ..default()
                    },
                    BackgroundColor(Color::srgba(1.0, 1.0, 1.0, 0.08)),
                    BorderRadius::all(Val::Px(4.0)),
                    KpiToggleButton,
                ))
                .with_child((
                    Text::new("Expand v"),
                    TextFont {
                        font_size: 12.0,
                        ..default()
                    },
                    TextColor(Color::srgb(0.5, 0.7, 0.9)),
                ));
        });

    // Thin separator under KPI header
    parent.spawn((
        Node {
            width: Val::Percent(100.0),
            height: Val::Px(1.0),
            ..default()
        },
        BackgroundColor(Color::srgb(0.3, 0.35, 0.4)),
    ));
}

pub(crate) fn spawn_kpi_area(parent: &mut ChildSpawnerCommands, report: &DayEndReport) {
    parent
        .spawn(Node {
            width: Val::Percent(100.0),
            flex_direction: FlexDirection::Column,
            ..default()
        })
        .with_children(|kpi_row| {
            spawn_kpi_expanded_view(kpi_row, report);
            spawn_kpi_collapsed_view(kpi_row, report);
        });
}

fn spawn_kpi_expanded_view(parent: &mut ChildSpawnerCommands, report: &DayEndReport) {
    parent
        .spawn((
            KpiExpanded,
            Node {
                width: Val::Percent(100.0),
                flex_direction: FlexDirection::Column,
                row_gap: Val::Px(4.0),
                display: Display::None,
                ..default()
            },
        ))
        .with_children(|section| {
            // A. Energy Margin
            let energy_color = Color::srgb(0.9, 0.7, 0.4);
            spawn_section_header(section, "Energy Margin", "[~]", energy_color);
            spawn_indented_row(
                section,
                "  Customer Price",
                &format!("${:.2} / kWh", report.avg_sell_price_kwh),
                Color::srgb(0.7, 0.7, 0.7),
            );
            spawn_indented_row(
                section,
                "  Grid Cost",
                &format!("${:.2} / kWh", report.avg_buy_price_kwh),
                Color::srgb(0.7, 0.7, 0.7),
            );
            spawn_indented_row(
                section,
                "  Charging Revenue",
                &format!("+${:.2}", report.charging_revenue),
                Color::srgb(0.4, 0.9, 0.4),
            );
            spawn_indented_row(
                section,
                "  Energy Costs",
                &format!("-${:.2}", report.energy_cost + report.demand_charge),
                Color::srgb(0.9, 0.7, 0.4),
            );
            let net_energy_margin =
                report.charging_revenue - report.energy_cost - report.demand_charge;
            let margin_color = if net_energy_margin >= 0.0 {
                Color::srgb(0.4, 0.9, 0.4)
            } else {
                Color::srgb(0.9, 0.4, 0.4)
            };
            spawn_section_divider(section, energy_color);
            spawn_indented_row(
                section,
                "  Net Energy Margin",
                &format_delta(net_energy_margin),
                margin_color,
            );
            spawn_insight_row(
                section,
                energy_margin_insight(report.charging_revenue, report.energy_cost),
            );

            // B. Operations
            let ops_color = Color::srgb(0.9, 0.4, 0.4);
            spawn_section_header(section, "Operations", "[*]", ops_color);
            if report.repair_parts > 0.01 {
                spawn_indented_row(
                    section,
                    "  Repair Parts",
                    &format!("-${:.2}", report.repair_parts),
                    ops_color,
                );
            }
            if report.repair_labor > 0.01 {
                spawn_indented_row(
                    section,
                    "  Repair Labor",
                    &format!("-${:.2}", report.repair_labor),
                    ops_color,
                );
            }
            if report.maintenance > 0.01 {
                spawn_indented_row(
                    section,
                    "  Maintenance",
                    &format!("-${:.2}", report.maintenance),
                    Color::srgb(0.9, 0.6, 0.4),
                );
            }
            if report.amenity > 0.01 {
                spawn_indented_row(
                    section,
                    "  Amenities",
                    &format!("-${:.2}", report.amenity),
                    Color::srgb(0.9, 0.6, 0.4),
                );
            }
            if report.cable_theft_cost > 0.01 {
                spawn_indented_row(
                    section,
                    "  Cable Theft (repaired)",
                    &format!("-${:.2}", report.cable_theft_cost),
                    Color::srgb(1.0, 0.2, 0.2),
                );
            }
            if report.pending_cable_thefts > 0 {
                spawn_indented_row(
                    section,
                    "  Cable Theft (pending)",
                    &format!(
                        "-${:.0} ({}x $2k)",
                        report.pending_cable_cost, report.pending_cable_thefts
                    ),
                    Color::srgb(1.0, 0.4, 0.1),
                );
            }
            if report.warranty_cost > 0.01 {
                spawn_indented_row(
                    section,
                    "  Warranty Premium",
                    &format!("-${:.2}", report.warranty_cost),
                    ops_color,
                );
            }
            if report.warranty_recovery > 0.01 {
                spawn_indented_row(
                    section,
                    "  Warranty Recovery",
                    &format!("+${:.2}", report.warranty_recovery),
                    Color::srgb(0.4, 0.9, 0.4),
                );
            }
            if report.refunds > 0.01 {
                spawn_indented_row(
                    section,
                    "  Refunds",
                    &format!("-${:.2}", report.refunds),
                    ops_color,
                );
            }
            if report.penalties > 0.01 {
                spawn_indented_row(
                    section,
                    "  Penalties",
                    &format!("-${:.2}", report.penalties),
                    ops_color,
                );
            }
            spawn_section_divider(section, ops_color);
            spawn_indented_row(
                section,
                "  Total OPEX",
                &format!("-${:.2}", report.total_opex),
                ops_color,
            );
            spawn_insight_row(
                section,
                operations_insight(
                    report.opex,
                    report.cable_theft_cost,
                    report.dispatches_delta,
                    report.warranty_recovery,
                ),
            );

            // B2. Fixed Costs
            if report.total_fixed > 0.01 {
                let fixed_color = Color::srgb(0.7, 0.55, 0.9);
                spawn_section_header(section, "Fixed Costs", "[=]", fixed_color);
                if report.rent > 0.01 {
                    spawn_indented_row(
                        section,
                        "  Site Rent",
                        &format!("-${:.2}", report.rent),
                        fixed_color,
                    );
                }
                if report.upgrades > 0.01 {
                    spawn_indented_row(
                        section,
                        "  Upgrades",
                        &format!("-${:.2}", report.upgrades),
                        fixed_color,
                    );
                }
            }

            // C. Reputation
            let rep_section_color = Color::srgb(0.4, 0.7, 0.9);
            spawn_section_header(section, "Reputation", "[#]", rep_section_color);
            spawn_indented_row(
                section,
                "  Successful Charges",
                &format!("+{}", report.sessions_delta),
                Color::srgb(0.4, 0.9, 0.4),
            );
            if report.sessions_failed_today > 0 {
                spawn_indented_row(
                    section,
                    "  Angry Drivers",
                    &format!("{}", report.sessions_failed_today),
                    Color::srgb(0.9, 0.4, 0.4),
                );
            }
            spawn_indented_row(
                section,
                "  Charger Availability",
                &format!(
                    "{}/{} online",
                    report.chargers_online, report.chargers_total
                ),
                if report.chargers_online < report.chargers_total {
                    Color::srgb(0.9, 0.7, 0.4)
                } else {
                    Color::srgb(0.4, 0.9, 0.4)
                },
            );
            spawn_section_divider(section, rep_section_color);
            spawn_indented_row(
                section,
                "  Net Change",
                &format_int_delta(report.reputation_delta),
                report.rep_color,
            );
            spawn_insight_row(
                section,
                reputation_insight(
                    report.reputation_delta,
                    report.sessions_delta,
                    report.sessions_failed_today,
                    report.chargers_online,
                    report.chargers_total,
                ),
            );

            // D. Unit Economy
            let unit_color = Color::srgb(0.85, 0.85, 0.85);
            spawn_section_header(section, "Unit Economy", "[$]", unit_color);
            if report.sessions_delta > 0 {
                spawn_indented_row(
                    section,
                    "  Revenue / Session",
                    &format!("${:.2}", report.revenue_per_session),
                    Color::srgb(0.4, 0.9, 0.4),
                );
                spawn_indented_row(
                    section,
                    "  Energy Cost / Session",
                    &format!("-${:.2}", report.cost_per_session),
                    Color::srgb(0.9, 0.7, 0.4),
                );
                let margin_per = report.revenue_per_session - report.cost_per_session;
                let margin_per_color = if margin_per >= 0.0 {
                    Color::srgb(0.4, 0.9, 0.4)
                } else {
                    Color::srgb(0.9, 0.4, 0.4)
                };
                spawn_section_divider(section, unit_color);
                spawn_indented_row(
                    section,
                    "  Margin / Session",
                    &format_delta(margin_per),
                    margin_per_color,
                );
                spawn_insight_row(
                    section,
                    &unit_economy_verdict(report.revenue_per_session, report.cost_per_session),
                );
            } else {
                spawn_insight_row(
                    section,
                    "No sessions to analyze. Build more chargers or check your pricing!",
                );
            }

            // E. Solar Export
            if report.has_solar {
                let solar_color = Color::srgb(1.0, 0.85, 0.1);
                spawn_section_header(section, "Solar Export", "[>]", solar_color);
                if report.solar_export_revenue > 0.01 {
                    spawn_indented_row(
                        section,
                        "  Grid Sellback",
                        &format!("+${:.2}", report.solar_export_revenue),
                        Color::srgb(0.4, 0.9, 0.4),
                    );
                    if report.grid_event_revenue > 0.01 {
                        spawn_indented_row(
                            section,
                            "  Event Revenue",
                            &format!("+${:.2}", report.grid_event_revenue),
                            Color::srgb(1.0, 0.78, 0.1),
                        );
                    }
                    if report.grid_event_import_surcharge > 0.01 {
                        spawn_indented_row(
                            section,
                            "  Event Surcharge",
                            &format!("-${:.2}", report.grid_event_import_surcharge),
                            Color::srgb(1.0, 0.4, 0.4),
                        );
                    }
                    if let Some((name, mult)) = report.best_spike {
                        spawn_indented_row(
                            section,
                            "  Best Event",
                            &format!("{name} ({mult:.1}x export)"),
                            Color::srgb(1.0, 0.78, 0.1),
                        );
                    }
                    spawn_insight_row(
                        section,
                        "Grid events temporarily raise import and export rates.",
                    );
                } else {
                    spawn_indented_row(
                        section,
                        "  Grid Sellback",
                        "+$0.00",
                        Color::srgb(0.7, 0.7, 0.7),
                    );
                    spawn_insight_row(
                        section,
                        "All solar consumed on-site -- no excess to export.",
                    );
                }
            }
        });
}

fn spawn_kpi_collapsed_view(parent: &mut ChildSpawnerCommands, report: &DayEndReport) {
    parent
        .spawn((
            KpiCollapsed,
            Node {
                width: Val::Percent(100.0),
                flex_direction: FlexDirection::Column,
                row_gap: Val::Px(8.0),
                ..default()
            },
        ))
        .with_children(|section| {
            section.spawn((
                Text::new(format!("DAY {}: \"{}\"", report.day, report.title_text)),
                TextFont {
                    font_size: 20.0,
                    ..default()
                },
                TextColor(Color::srgb(0.4, 0.8, 1.0)),
            ));
            section.spawn((
                Text::new(&report.subtitle_text),
                TextFont {
                    font_size: 13.0,
                    ..default()
                },
                TextColor(Color::srgb(0.5, 0.5, 0.5)),
                Node {
                    margin: UiRect::bottom(Val::Px(4.0)),
                    ..default()
                },
            ));

            spawn_stat_row_with_hint(
                section,
                "Total Revenue",
                &format!("+${:.2}", report.total_income),
                Color::srgb(0.4, 0.9, 0.4),
                report.revenue_hint.as_deref(),
            );
            spawn_stat_row_with_hint(
                section,
                "Total Expenses",
                &format!("-${:.2}", report.total_expenses),
                Color::srgb(0.9, 0.4, 0.4),
                report.expense_hint.as_deref(),
            );
            // Thin divider above Operating Profit
            section.spawn((
                Node {
                    width: Val::Percent(100.0),
                    height: Val::Px(1.0),
                    margin: UiRect::vertical(Val::Px(2.0)),
                    ..default()
                },
                BackgroundColor(Color::srgba(0.4, 0.45, 0.5, 0.4)),
            ));
            spawn_prominent_stat_row(
                section,
                "Operating Profit",
                &format_delta(report.operating_profit),
                report.profit_color,
            );
            spawn_stat_row(
                section,
                "Reputation",
                &format!(
                    "{} ({})",
                    report.reputation,
                    format_int_delta(report.reputation_delta)
                ),
                report.rep_color,
            );
            spawn_stat_row(
                section,
                "Station Status",
                &format!(
                    "{}/{} Online",
                    report.chargers_online, report.chargers_total
                ),
                if report.chargers_online < report.chargers_total {
                    Color::srgb(0.9, 0.7, 0.4)
                } else {
                    Color::srgb(0.4, 0.9, 0.4)
                },
            );

            // Pro-Tip callout box
            section
                .spawn((
                    Node {
                        width: Val::Percent(100.0),
                        flex_direction: FlexDirection::Column,
                        padding: UiRect::new(
                            Val::Px(12.0),
                            Val::Px(12.0),
                            Val::Px(10.0),
                            Val::Px(10.0),
                        ),
                        margin: UiRect::top(Val::Px(8.0)),
                        border: UiRect::left(Val::Px(3.0)),
                        ..default()
                    },
                    BackgroundColor(Color::srgba(0.2, 0.22, 0.28, 0.8)),
                    BorderColor::all(Color::srgb(0.4, 0.7, 0.9)),
                    BorderRadius::all(Val::Px(4.0)),
                ))
                .with_child((
                    Text::new(&report.pro_tip_text),
                    TextFont {
                        font_size: 13.0,
                        ..default()
                    },
                    TextColor(Color::srgb(0.7, 0.8, 0.9)),
                ));
        });
}

pub(crate) fn spawn_badge_section(parent: &mut ChildSpawnerCommands, report: &DayEndReport) {
    let Some((badge, icon_handle)) = &report.top_badge_data else {
        return;
    };

    // Divider
    parent.spawn((
        Node {
            width: Val::Percent(100.0),
            height: Val::Px(1.0),
            margin: UiRect::vertical(Val::Px(4.0)),
            ..default()
        },
        BackgroundColor(Color::srgb(0.3, 0.35, 0.4)),
    ));

    // Badge header
    parent
        .spawn((Node {
            width: Val::Percent(100.0),
            justify_content: JustifyContent::Center,
            ..default()
        },))
        .with_child((
            Text::new("BADGE UNLOCKED"),
            TextFont {
                font_size: 14.0,
                ..default()
            },
            TextColor(Color::srgb(0.5, 0.5, 0.5)),
        ));

    // Badge card
    parent
        .spawn((
            Node {
                width: Val::Percent(100.0),
                flex_direction: FlexDirection::Row,
                column_gap: Val::Px(12.0),
                align_items: AlignItems::Center,
                padding: UiRect::all(Val::Px(10.0)),
                ..default()
            },
            BackgroundColor(Color::srgba(0.2, 0.22, 0.28, 0.8)),
            BorderRadius::all(Val::Px(6.0)),
        ))
        .with_children(|badge_row| {
            badge_row.spawn((
                ImageNode::new(icon_handle.clone()),
                Node {
                    width: Val::Px(40.0),
                    height: Val::Px(40.0),
                    ..default()
                },
            ));

            badge_row
                .spawn(Node {
                    flex_direction: FlexDirection::Column,
                    row_gap: Val::Px(2.0),
                    ..default()
                })
                .with_children(|badge_info| {
                    badge_info.spawn((
                        Text::new(badge.name()),
                        TextFont {
                            font_size: 18.0,
                            ..default()
                        },
                        TextColor(badge.tier().color()),
                    ));
                    badge_info.spawn((
                        Text::new(format!("({})", badge.tier().short_label())),
                        TextFont {
                            font_size: 12.0,
                            ..default()
                        },
                        TextColor(Color::srgb(0.5, 0.5, 0.5)),
                    ));
                });
        });
}
