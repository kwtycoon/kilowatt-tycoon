use bevy::ecs::hierarchy::ChildSpawnerCommands;
use bevy::prelude::*;

pub(crate) fn format_delta(val: f32) -> String {
    if val >= 0.0 {
        format!("+${val:.2}")
    } else {
        format!("-${:.2}", val.abs())
    }
}

pub(crate) fn format_int_delta(val: i32) -> String {
    if val >= 0 {
        format!("+{val}")
    } else {
        format!("{val}")
    }
}

pub(crate) fn spawn_stat_row(
    parent: &mut ChildSpawnerCommands,
    label: &str,
    value: &str,
    value_color: Color,
) {
    parent
        .spawn(Node {
            width: Val::Percent(100.0),
            justify_content: JustifyContent::SpaceBetween,
            ..default()
        })
        .with_children(|row| {
            row.spawn((
                Text::new(label),
                TextFont {
                    font_size: 18.0,
                    ..default()
                },
                TextColor(Color::srgb(0.7, 0.7, 0.7)),
            ));
            row.spawn((
                Text::new(value),
                TextFont {
                    font_size: 18.0,
                    ..default()
                },
                TextColor(value_color),
            ));
        });
}

pub(crate) fn spawn_prominent_stat_row(
    parent: &mut ChildSpawnerCommands,
    label: &str,
    value: &str,
    value_color: Color,
) {
    parent
        .spawn(Node {
            width: Val::Percent(100.0),
            justify_content: JustifyContent::SpaceBetween,
            ..default()
        })
        .with_children(|row| {
            row.spawn((
                Text::new(label),
                TextFont {
                    font_size: 20.0,
                    ..default()
                },
                TextColor(Color::srgb(0.85, 0.85, 0.85)),
            ));
            row.spawn((
                Text::new(value),
                TextFont {
                    font_size: 20.0,
                    ..default()
                },
                TextColor(value_color),
            ));
        });
}

pub(crate) fn spawn_indented_row(
    parent: &mut ChildSpawnerCommands,
    label: &str,
    value: &str,
    value_color: Color,
) {
    parent
        .spawn(Node {
            width: Val::Percent(100.0),
            justify_content: JustifyContent::SpaceBetween,
            ..default()
        })
        .with_children(|row| {
            row.spawn((
                Text::new(label),
                TextFont {
                    font_size: 15.0,
                    ..default()
                },
                TextColor(Color::srgb(0.6, 0.6, 0.6)),
            ));
            row.spawn((
                Text::new(value),
                TextFont {
                    font_size: 15.0,
                    ..default()
                },
                TextColor(value_color),
            ));
        });
}

pub(crate) fn spawn_stat_row_with_hint(
    parent: &mut ChildSpawnerCommands,
    label: &str,
    value: &str,
    value_color: Color,
    hint: Option<&str>,
) {
    parent
        .spawn(Node {
            width: Val::Percent(100.0),
            justify_content: JustifyContent::SpaceBetween,
            align_items: AlignItems::Baseline,
            ..default()
        })
        .with_children(|row| {
            row.spawn((
                Text::new(label),
                TextFont {
                    font_size: 18.0,
                    ..default()
                },
                TextColor(Color::srgb(0.7, 0.7, 0.7)),
            ));
            row.spawn(Node {
                flex_direction: FlexDirection::Row,
                column_gap: Val::Px(6.0),
                align_items: AlignItems::Baseline,
                flex_wrap: FlexWrap::Wrap,
                ..default()
            })
            .with_children(|val_row| {
                val_row.spawn((
                    Text::new(value),
                    TextFont {
                        font_size: 18.0,
                        ..default()
                    },
                    TextColor(value_color),
                ));
                if let Some(hint_text) = hint {
                    val_row.spawn((
                        Text::new(hint_text),
                        TextFont {
                            font_size: 12.0,
                            ..default()
                        },
                        TextColor(Color::srgb(0.5, 0.5, 0.5)),
                    ));
                }
            });
        });
}

pub(crate) fn spawn_insight_row(parent: &mut ChildSpawnerCommands, text: &str) {
    parent.spawn((
        Text::new(text),
        TextFont {
            font_size: 13.0,
            ..default()
        },
        TextColor(Color::srgb(0.6, 0.7, 0.8)),
        Node {
            margin: UiRect::top(Val::Px(4.0)),
            ..default()
        },
    ));
}

pub(crate) fn spawn_section_header(
    parent: &mut ChildSpawnerCommands,
    label: &str,
    icon: &str,
    color: Color,
) {
    parent
        .spawn(Node {
            width: Val::Percent(100.0),
            flex_direction: FlexDirection::Row,
            column_gap: Val::Px(6.0),
            align_items: AlignItems::Center,
            margin: UiRect::top(Val::Px(8.0)),
            ..default()
        })
        .with_children(|row| {
            row.spawn((
                Text::new(icon),
                TextFont {
                    font_size: 16.0,
                    ..default()
                },
                TextColor(color),
            ));
            row.spawn((
                Text::new(label),
                TextFont {
                    font_size: 16.0,
                    ..default()
                },
                TextColor(color),
            ));
        });
}

pub(crate) fn spawn_section_divider(parent: &mut ChildSpawnerCommands, color: Color) {
    parent.spawn((
        Node {
            width: Val::Percent(100.0),
            height: Val::Px(1.0),
            margin: UiRect::vertical(Val::Px(4.0)),
            ..default()
        },
        BackgroundColor(Color::srgba(
            color.to_srgba().red,
            color.to_srgba().green,
            color.to_srgba().blue,
            0.3,
        )),
    ));
}

/// Generate a dynamic day title and subtitle based on the day's performance.
pub(crate) fn day_title(
    day: u32,
    net_profit: f32,
    reputation_delta: i32,
    sessions: i32,
    charging_revenue: f32,
    energy_cost: f32,
    opex: f32,
    warranty_cost: f32,
    warranty_recovery: f32,
) -> (&'static str, &'static str) {
    if day == 1 {
        return ("Day One", "Every empire starts somewhere.");
    }
    if sessions == 0 {
        return ("Ghost Town", "Not a single EV in sight.");
    }
    if net_profit >= 0.0 && reputation_delta >= 0 {
        if warranty_recovery > warranty_cost * 5.0 && warranty_recovery > 500.0 {
            return (
                "Insurance Payday",
                "The warranty just earned its keep - and then some.",
            );
        }
        return ("Smooth Operations", "A good day at the station.");
    }
    if net_profit >= 0.0 && reputation_delta < 0 {
        return (
            "Profitable, But...",
            "The money's good. The reviews? Not so much.",
        );
    }
    if opex > energy_cost && opex > 0.01 {
        return ("Murphy's Law", "Everything that could break, did.");
    }
    if charging_revenue < energy_cost && energy_cost > 0.01 {
        return ("The Pricing Trap", "Selling electrons below cost.");
    }
    if warranty_cost > 0.01 && warranty_recovery < 0.01 && opex < 1.0 {
        return (
            "Quiet Shift",
            "Nothing broke. The warranty company sends their thanks.",
        );
    }
    ("A Rough Start", "Room for improvement...")
}

/// Generate a humorous, contextual pro-tip based on the day's biggest problem.
pub(crate) fn generate_pro_tip(
    char_name: &str,
    net_profit: f32,
    sessions: i32,
    charging_revenue: f32,
    energy_cost: f32,
    opex: f32,
    reputation_delta: i32,
    warranty_cost: f32,
    warranty_recovery: f32,
    destroyed_transformers: i32,
) -> String {
    let tip = if destroyed_transformers >= 2 {
        "Two transformers down in one day. At this rate, the fire department will name a wing after us."
    } else if destroyed_transformers == 1 {
        "One transformer caught fire today. On the bright side, we're on a first-name basis with the fire chief now."
    } else if sessions == 0 {
        "Zero sessions today. The tumbleweeds are charging for free though."
    } else if charging_revenue < energy_cost && energy_cost > 0.01 {
        "We're losing money on every electron we sell, but at least the technician bought a new boat with our repair fees!"
    } else if warranty_recovery > warranty_cost && warranty_recovery > 0.01 {
        "The extended warranty covered more than its premium today. Rare W for insurance."
    } else if opex > charging_revenue && opex > 0.01 && warranty_cost < 0.01 {
        "Our repair budget could fund a small space program. Ever heard of an extended warranty?"
    } else if opex > charging_revenue && opex > 0.01 {
        "Our repair budget could fund a small space program."
    } else if warranty_cost > 0.01 && opex > warranty_recovery && opex > 0.01 {
        "Consider the Premium warranty - it covers 80% of labor too."
    } else if reputation_delta < -20 {
        "The local EV forum has created a dedicated thread about us. It's not flattering."
    } else if reputation_delta < -5 {
        "Drivers are starting to leave 1-star reviews. Might want to check on those chargers."
    } else if net_profit >= 0.0 {
        "Not bad! Keep this up and we might actually afford that second coffee machine."
    } else {
        "Every great empire had rough patches. Ours just happen to be expensive."
    };
    format!("{char_name}'s Pro-Tip: \"{tip}\"")
}

pub(crate) fn energy_margin_insight(charging_revenue: f32, energy_cost: f32) -> &'static str {
    if energy_cost < 0.01 {
        "No energy consumed today. Solar power for the win?"
    } else if charging_revenue < energy_cost {
        "You are currently subsidizing your customers' commutes. Generous, but expensive!"
    } else if charging_revenue < energy_cost * 1.2 {
        "Margins are razor-thin. One bad hour of peak pricing could wipe out your profit."
    } else {
        "Energy margins are healthy. Keep an eye on peak demand charges though."
    }
}

pub(crate) fn operations_insight(
    opex: f32,
    cable_theft_cost: f32,
    dispatches: i32,
    warranty_recovery: f32,
) -> &'static str {
    if warranty_recovery > 500.0 {
        "The warranty just paid for itself. Sometimes insurance actually works out."
    } else if cable_theft_cost > 0.01 && opex > 0.01 {
        "Between the thieves and the breakdowns, it's been an eventful day."
    } else if cable_theft_cost > 0.01 {
        "Cable thieves struck again. Maybe invest in some security cameras?"
    } else if dispatches > 2 {
        "The technician is starting to recognize our parking lot. Not a great sign."
    } else if opex > 0.01 {
        "A charger had a bad day. These things happen... less often with better O&M."
    } else {
        "No operational issues today. The chargers are behaving themselves!"
    }
}

pub(crate) fn reputation_insight(
    reputation_delta: i32,
    sessions: i32,
    sessions_failed: i32,
    chargers_online: i32,
    chargers_total: i32,
) -> &'static str {
    if chargers_total > 0 && chargers_online < chargers_total / 2 {
        "Most of your chargers are offline. The local EV forum is roasting you."
    } else if sessions_failed > sessions && sessions_failed > 0 {
        "More angry drivers than happy ones. Time to figure out what's going wrong."
    } else if reputation_delta < -10 {
        "Drivers are losing patience. Broken chargers and long waits are taking their toll."
    } else if reputation_delta < 0 {
        "A few unhappy customers today. Could be worse, but could definitely be better."
    } else if reputation_delta > 5 {
        "Word is spreading -- drivers are starting to recommend your station!"
    } else if reputation_delta >= 0 && sessions > 0 {
        "Steady reputation. Consistent service keeps drivers coming back."
    } else {
        "No drivers to impress today. Build it and they will come... eventually."
    }
}

pub(crate) fn fleet_insight(
    vehicles_missed: u32,
    breaches_remaining: u32,
    terminated: bool,
) -> &'static str {
    if terminated {
        "Contract terminated. The fleet operator has moved on."
    } else if breaches_remaining <= 2 {
        "One or two more missed vehicles and this contract is gone."
    } else if vehicles_missed > 0 {
        "Missed fleet vehicles cost you money AND burn through breach allowance."
    } else {
        "Perfect fleet day. Keep it up and the operator may extend the contract."
    }
}

pub(crate) fn unit_economy_verdict(revenue_per_session: f32, cost_per_session: f32) -> String {
    let margin = revenue_per_session - cost_per_session;
    if margin < 0.0 {
        format!(
            "You earned ${:.2} per session but spent ${:.2} on electricity for that same session. Head to the Pricing menu!",
            revenue_per_session, cost_per_session
        )
    } else if margin < 2.0 {
        format!(
            "Just ${:.2} margin per session. One spike in grid prices could flip you to a loss.",
            margin
        )
    } else {
        format!(
            "Healthy ${:.2} margin per session. Now focus on getting more drivers through the door.",
            margin
        )
    }
}
