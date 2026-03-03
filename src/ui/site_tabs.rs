//! Site switcher tabs - UI for switching between owned sites

use bevy::ecs::hierarchy::ChildSpawnerCommands;
use bevy::prelude::*;

use crate::resources::{MultiSiteManager, SiteArchetype, SiteId};

/// Marker component for the site tabs container
#[derive(Component)]
pub struct SiteTabsContainer;

/// Component for individual site tab buttons
#[derive(Component)]
pub struct SiteTab {
    pub site_id: SiteId,
}

/// Component for sell site button
#[derive(Component)]
pub struct SellSiteButton {
    pub site_id: SiteId,
}

/// Spawn the site switcher tabs container
pub fn spawn_site_tabs(parent: &mut ChildSpawnerCommands) {
    parent
        .spawn((
            SiteTabsContainer,
            Node {
                width: Val::Percent(100.0),
                height: Val::Px(52.0),
                flex_direction: FlexDirection::Row,
                column_gap: Val::Px(4.0),
                padding: UiRect::all(Val::Px(8.0)),
                flex_shrink: 0.0,
                ..default()
            },
            BackgroundColor(Color::srgba(0.0, 0.0, 0.0, 0.3)),
        ))
        .with_children(|tabs_row| {
            // Tabs will be dynamically added when sites are rented
            tabs_row.spawn((
                Text::new("Sites will appear here"),
                TextFont {
                    font_size: 12.0,
                    ..default()
                },
                TextColor(Color::srgb(0.5, 0.5, 0.5)),
            ));
        });
}

/// Update site tabs when sites are added/removed
pub fn update_site_tabs(
    multi_site: Res<MultiSiteManager>,
    game_state: Res<crate::resources::GameState>,
    build_state: Res<crate::resources::BuildState>,
    mut commands: Commands,
    tabs_container: Query<Entity, With<SiteTabsContainer>>,
    _existing_tabs: Query<Entity, With<SiteTab>>,
    children_query: Query<&Children>,
) {
    if !multi_site.is_changed() && !game_state.is_changed() && !build_state.is_changed() {
        return;
    }

    let day_running = build_state.is_open;

    for container_entity in &tabs_container {
        // Clear existing tabs
        if let Ok(children) = children_query.get(container_entity) {
            for child in children.iter() {
                commands.entity(child).try_despawn();
            }
        }

        // Rebuild tabs
        commands.entity(container_entity).with_children(|tabs_row| {
            let owned_sites = multi_site.owned_sites_list();

            if owned_sites.is_empty() {
                tabs_row.spawn((
                    Text::new("No sites owned. Visit Rent tab to get started."),
                    TextFont {
                        font_size: 12.0,
                        ..default()
                    },
                    TextColor(Color::srgb(0.6, 0.6, 0.6)),
                ));
                return;
            }

            let can_sell = owned_sites.len() > 1;

            for (site_id, site_state) in owned_sites {
                let is_active = multi_site.viewed_site_id == Some(site_id);
                let archetype_color = get_archetype_color(site_state.archetype);

                // Container for tab + sell button
                tabs_row
                    .spawn(Node {
                        flex_direction: FlexDirection::Row,
                        column_gap: Val::Px(2.0),
                        align_items: AlignItems::Center,
                        ..default()
                    })
                    .with_children(|tab_row| {
                        let locked = day_running && !is_active;
                        let bg = if is_active {
                            archetype_color
                        } else if locked {
                            Color::srgba(0.15, 0.15, 0.15, 0.5)
                        } else {
                            Color::srgb(0.2, 0.2, 0.2)
                        };
                        let text_color = if locked {
                            Color::srgba(1.0, 1.0, 1.0, 0.35)
                        } else {
                            Color::WHITE
                        };

                        tab_row
                            .spawn((
                                Button,
                                SiteTab { site_id },
                                Node {
                                    padding: UiRect::all(Val::Px(8.0)),
                                    column_gap: Val::Px(6.0),
                                    align_items: AlignItems::Center,
                                    ..default()
                                },
                                BackgroundColor(bg),
                                BorderRadius::all(Val::Px(6.0)),
                            ))
                            .with_children(|tab| {
                                tab.spawn((
                                    Text::new(&site_state.name),
                                    TextFont {
                                        font_size: 13.0,
                                        ..default()
                                    },
                                    TextColor(text_color),
                                ));

                                // Revenue indicator (small) - only show for current day's site
                                let is_todays_site = game_state
                                    .daily_history
                                    .current_day
                                    .site_id
                                    .map(|id| id == site_id)
                                    .unwrap_or(false);
                                if is_todays_site
                                    && game_state.daily_history.current_day.total_revenue() > 0.0
                                {
                                    tab.spawn((
                                        Text::new(format!(
                                            "+${:.0}",
                                            game_state.daily_history.current_day.total_revenue()
                                        )),
                                        TextFont {
                                            font_size: 10.0,
                                            ..default()
                                        },
                                        TextColor(Color::srgb(0.4, 0.9, 0.4)),
                                    ));
                                }
                            });

                        // Sell button hidden - too easy to accidentally lose a site
                        // TODO: Re-enable with confirmation dialog
                        // if can_sell {
                        //     tab_row
                        //         .spawn((
                        //             Button,
                        //             SellSiteButton { site_id },
                        //             Node {
                        //                 width: Val::Px(24.0),
                        //                 height: Val::Px(24.0),
                        //                 justify_content: JustifyContent::Center,
                        //                 align_items: AlignItems::Center,
                        //                 ..default()
                        //             },
                        //             BackgroundColor(Color::srgb(0.6, 0.2, 0.2)),
                        //             BorderRadius::all(Val::Px(4.0)),
                        //         ))
                        //         .with_children(|btn| {
                        //             btn.spawn((
                        //                 Text::new("×"),
                        //                 TextFont {
                        //                     font_size: 16.0,
                        //                     ..default()
                        //                 },
                        //                 TextColor(Color::WHITE),
                        //             ));
                        //         });
                        // }
                        let _ = can_sell; // Suppress unused variable warning
                    });
            }
        });
    }
}

/// Get color for site archetype (for tab highlighting)
fn get_archetype_color(archetype: SiteArchetype) -> Color {
    match archetype {
        SiteArchetype::ParkingLot => Color::srgb(0.3, 0.5, 0.7), // Blue
        SiteArchetype::GasStation => Color::srgb(0.8, 0.5, 0.2), // Orange
        SiteArchetype::FleetDepot => Color::srgb(0.4, 0.4, 0.4), // Dark gray (industrial)
        SiteArchetype::ScooterHub => Color::srgb(0.2, 0.7, 0.35), // Green (urban mobility)
    }
}

/// Handle site tab clicks (disabled while the day is running)
pub fn handle_site_tab_clicks(
    build_state: Res<crate::resources::BuildState>,
    interaction_query: Query<(&Interaction, &SiteTab), (Changed<Interaction>, With<Button>)>,
    mut switch_events: MessageWriter<crate::events::SiteSwitchEvent>,
) {
    if build_state.is_open {
        return;
    }
    for (interaction, tab) in &interaction_query {
        if *interaction == Interaction::Pressed {
            info!("Switching to site {:?}", tab.site_id);
            switch_events.write(crate::events::SiteSwitchEvent {
                target_site_id: tab.site_id,
            });
        }
    }
}

/// Handle sell site button clicks
pub fn handle_sell_site_clicks(
    interaction_query: Query<(&Interaction, &SellSiteButton), (Changed<Interaction>, With<Button>)>,
    mut multi_site: ResMut<MultiSiteManager>,
    mut game_state: ResMut<crate::resources::GameState>,
    mut sold_events: MessageWriter<crate::events::SiteSoldEvent>,
) {
    for (interaction, sell_btn) in &interaction_query {
        if *interaction == Interaction::Pressed {
            info!("Attempting to sell site {:?}", sell_btn.site_id);

            match multi_site.sell_site(sell_btn.site_id) {
                Ok(refund_amount) => {
                    info!("Site {:?} sold for ${:.0}", sell_btn.site_id, refund_amount);

                    game_state.refund_site_sale(refund_amount);

                    // Trigger sold event for entity cleanup
                    sold_events.write(crate::events::SiteSoldEvent {
                        site_id: sell_btn.site_id,
                        refund_amount,
                    });
                }
                Err(e) => {
                    warn!("Failed to sell site {:?}: {}", sell_btn.site_id, e);
                }
            }
        }
    }
}
