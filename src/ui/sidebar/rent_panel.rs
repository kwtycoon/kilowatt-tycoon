//! Rent panel UI - Location carousel for renting new sites

use bevy::ecs::hierarchy::ChildSpawnerCommands;
use bevy::prelude::*;

use super::panel::spawn_panel_container;
use super::{ActivePanel, NavigationState, PrimaryNav};
use crate::events::SiteSwitchEvent;
use crate::resources::{GameState, ImageAssets, MultiSiteManager};

/// Carousel state for browsing available sites
#[derive(Resource, Debug, Clone, Default)]
pub struct RentCarouselState {
    pub current_index: usize,
}

/// Marker component for rent panel
#[derive(Component)]
pub struct RentPanel;

/// Carousel navigation buttons
#[derive(Component, Debug, Clone, Copy)]
pub enum CarouselButton {
    Previous,
    Next,
}

/// Rent button component
#[derive(Component)]
pub struct RentSiteButton {
    pub listing_index: usize,
}

/// Spawn the rent panel
pub fn spawn_rent_panel(parent: &mut ChildSpawnerCommands) {
    spawn_panel_container(
        parent,
        ActivePanel::Rent,
        RentPanel,
        false, // Not visible by default
    )
    .with_children(|content| {
        // Panel will be populated by update system
        content.spawn((
            Text::new("Loading locations..."),
            TextFont {
                font_size: 14.0,
                ..default()
            },
            TextColor(Color::srgb(0.7, 0.7, 0.7)),
        ));
    });
}

/// Track whether the rent panel content is stale and needs a rebuild
/// the next time the panel becomes visible.
#[derive(Resource, Default)]
pub struct RentPanelDirty(pub bool);

/// Update the rent panel content when data changes and the panel is visible.
///
/// Children are only spawned while the panel has `Display::Flex`.
/// Bevy's taffy layout reports zero-size for nodes first created under a
/// `Display::None` parent and never recomputes them, so we defer content
/// creation until the panel is actually visible. When data changes while
/// hidden, we set a dirty flag and rebuild on the next frame the panel
/// is shown.
pub fn update_rent_panel(
    multi_site: Res<MultiSiteManager>,
    carousel: Res<RentCarouselState>,
    game_state: Res<GameState>,
    image_assets: Res<ImageAssets>,
    panel_query: Query<Entity, With<RentPanel>>,
    children_query: Query<&Children>,
    mut dirty: ResMut<RentPanelDirty>,
    mut commands: Commands,
) {
    if multi_site.is_changed() || carousel.is_changed() || game_state.is_changed() {
        dirty.0 = true;
    }

    if !dirty.0 {
        return;
    }
    dirty.0 = false;

    for entity in &panel_query {
        if let Ok(children) = children_query.get(entity) {
            for child in children.iter() {
                commands.entity(child).try_despawn();
            }
        }

        commands.entity(entity).with_children(|panel| {
            render_rent_panel_content(panel, &multi_site, &carousel, &game_state, &image_assets);
        });
    }
}

/// Render the rent panel content
fn render_rent_panel_content(
    panel: &mut ChildSpawnerCommands,
    multi_site: &MultiSiteManager,
    carousel: &RentCarouselState,
    game_state: &GameState,
    image_assets: &ImageAssets,
) {
    // Title
    panel.spawn((
        Text::new("Locations"),
        TextFont {
            font_size: 18.0,
            ..default()
        },
        TextColor(Color::srgb(0.4, 0.8, 1.0)),
    ));

    panel.spawn((
        Text::new("Choose a location."),
        TextFont {
            font_size: 14.0,
            ..default()
        },
        TextColor(Color::srgb(0.6, 0.6, 0.6)),
    ));

    let available_sites = multi_site.available_sites_list();

    if available_sites.is_empty() {
        panel.spawn((
            Text::new("No sites available to rent."),
            TextFont {
                font_size: 14.0,
                ..default()
            },
            TextColor(Color::srgb(0.7, 0.7, 0.7)),
        ));
        return;
    }

    let current_site = &available_sites[carousel.current_index];

    // Carousel navigation
    panel
        .spawn(Node {
            width: Val::Percent(100.0),
            justify_content: JustifyContent::SpaceBetween,
            align_items: AlignItems::Center,
            ..default()
        })
        .with_children(|nav_row| {
            // Previous button
            let can_go_prev = carousel.current_index > 0;
            nav_row
                .spawn((
                    Button,
                    Node {
                        width: Val::Px(40.0),
                        height: Val::Px(40.0),
                        justify_content: JustifyContent::Center,
                        align_items: AlignItems::Center,
                        ..default()
                    },
                    BackgroundColor(if can_go_prev {
                        Color::srgb(0.3, 0.4, 0.5)
                    } else {
                        Color::srgb(0.2, 0.2, 0.2)
                    }),
                    BorderRadius::all(Val::Px(8.0)),
                    CarouselButton::Previous,
                ))
                .with_child((
                    ImageNode::new(image_assets.icon_arrow_left.clone()),
                    Node {
                        width: Val::Px(24.0),
                        height: Val::Px(24.0),
                        ..default()
                    },
                ));

            // Site counter
            nav_row.spawn((
                Text::new(format!(
                    "{} / {}",
                    carousel.current_index + 1,
                    available_sites.len()
                )),
                TextFont {
                    font_size: 14.0,
                    ..default()
                },
                TextColor(Color::srgb(0.7, 0.7, 0.7)),
            ));

            // Next button
            let can_go_next = carousel.current_index < available_sites.len() - 1;
            nav_row
                .spawn((
                    Button,
                    Node {
                        width: Val::Px(40.0),
                        height: Val::Px(40.0),
                        justify_content: JustifyContent::Center,
                        align_items: AlignItems::Center,
                        ..default()
                    },
                    BackgroundColor(if can_go_next {
                        Color::srgb(0.3, 0.4, 0.5)
                    } else {
                        Color::srgb(0.2, 0.2, 0.2)
                    }),
                    BorderRadius::all(Val::Px(8.0)),
                    CarouselButton::Next,
                ))
                .with_child((
                    ImageNode::new(image_assets.icon_arrow_right.clone()),
                    Node {
                        width: Val::Px(24.0),
                        height: Val::Px(24.0),
                        ..default()
                    },
                ));
        });

    // Site card
    panel
        .spawn((
            Node {
                width: Val::Percent(100.0),
                flex_direction: FlexDirection::Column,
                padding: UiRect::all(Val::Px(16.0)),
                row_gap: Val::Px(12.0),
                ..default()
            },
            BackgroundColor(Color::srgba(0.15, 0.17, 0.2, 0.9)),
            BorderColor::all(Color::srgb(0.3, 0.35, 0.4)),
            BorderRadius::all(Val::Px(8.0)),
        ))
        .with_children(|card| {
            // Site name
            card.spawn((
                Text::new(&current_site.name),
                TextFont {
                    font_size: 20.0,
                    ..default()
                },
                TextColor(Color::srgb(0.4, 0.8, 1.0)),
            ));

            // Archetype subtitle
            card.spawn((
                Text::new(current_site.archetype.display_name()),
                TextFont {
                    font_size: 14.0,
                    ..default()
                },
                TextColor(Color::srgb(0.6, 0.7, 0.8)),
            ));

            // Description
            card.spawn((
                Text::new(&current_site.description),
                TextFont {
                    font_size: 12.0,
                    ..default()
                },
                TextColor(Color::srgb(0.7, 0.7, 0.7)),
            ));

            // Grid capacity (prominent)
            card.spawn(Node {
                width: Val::Percent(100.0),
                padding: UiRect::all(Val::Px(8.0)),
                flex_direction: FlexDirection::Row,
                column_gap: Val::Px(6.0),
                align_items: AlignItems::Center,
                ..default()
            })
            .with_children(|row| {
                row.spawn((
                    ImageNode::new(image_assets.icon_power.clone()),
                    Node {
                        width: Val::Px(16.0),
                        height: Val::Px(16.0),
                        ..default()
                    },
                ));
                row.spawn((
                    Text::new(format!(
                        "Grid Power: {} kVA",
                        current_site.grid_capacity_kva
                    )),
                    TextFont {
                        font_size: 14.0,
                        ..default()
                    },
                    TextColor(if current_site.is_power_constrained() {
                        Color::srgb(1.0, 0.6, 0.2) // Warning orange
                    } else {
                        Color::srgb(0.4, 0.9, 0.4) // Green
                    }),
                ));
            });

            // Power warning if constrained
            if let Some(warning) = current_site.power_warning() {
                card.spawn(Node {
                    width: Val::Percent(100.0),
                    padding: UiRect::all(Val::Px(8.0)),
                    flex_direction: FlexDirection::Row,
                    column_gap: Val::Px(6.0),
                    align_items: AlignItems::Center,
                    ..default()
                })
                .with_children(|warning_box| {
                    warning_box.spawn((
                        ImageNode::new(image_assets.icon_warning.clone()),
                        Node {
                            width: Val::Px(14.0),
                            height: Val::Px(14.0),
                            ..default()
                        },
                    ));
                    warning_box.spawn((
                        Text::new(warning),
                        TextFont {
                            font_size: 11.0,
                            ..default()
                        },
                        TextColor(Color::srgb(1.0, 0.7, 0.3)),
                    ));
                });
            }

            // Climate warning for cold sites
            if let Some(warning) = current_site.archetype.climate_warning() {
                card.spawn(Node {
                    width: Val::Percent(100.0),
                    padding: UiRect::all(Val::Px(8.0)),
                    flex_direction: FlexDirection::Row,
                    column_gap: Val::Px(6.0),
                    align_items: AlignItems::Center,
                    ..default()
                })
                .with_children(|warning_box| {
                    warning_box.spawn((
                        ImageNode::new(image_assets.icon_weather_cold.clone()),
                        Node {
                            width: Val::Px(14.0),
                            height: Val::Px(14.0),
                            ..default()
                        },
                    ));
                    warning_box.spawn((
                        Text::new(warning),
                        TextFont {
                            font_size: 11.0,
                            ..default()
                        },
                        TextColor(Color::srgb(0.5, 0.8, 1.0)), // Cold blue color
                    ));
                });
            }

            // Stats grid
            card.spawn(Node {
                width: Val::Percent(100.0),
                flex_direction: FlexDirection::Column,
                row_gap: Val::Px(6.0),
                ..default()
            })
            .with_children(|stats| {
                // Popularity bar
                stats
                    .spawn(Node {
                        width: Val::Percent(100.0),
                        flex_direction: FlexDirection::Column,
                        row_gap: Val::Px(4.0),
                        ..default()
                    })
                    .with_children(|pop| {
                        pop.spawn((
                            Text::new("Popularity"),
                            TextFont {
                                font_size: 12.0,
                                ..default()
                            },
                            TextColor(Color::srgb(0.6, 0.6, 0.6)),
                        ));

                        // Progress bar
                        pop.spawn((
                            Node {
                                width: Val::Percent(100.0),
                                height: Val::Px(8.0),
                                ..default()
                            },
                            BackgroundColor(Color::srgb(0.2, 0.2, 0.2)),
                            BorderRadius::all(Val::Px(4.0)),
                        ))
                        .with_child((
                            Node {
                                width: Val::Percent(current_site.popularity),
                                height: Val::Percent(100.0),
                                ..default()
                            },
                            BackgroundColor(Color::srgb(0.2, 0.7, 1.0)),
                            BorderRadius::all(Val::Px(4.0)),
                        ));
                    });
            });

            // Rent button
            let is_owned = multi_site.owns_archetype(current_site.archetype);
            let can_afford = game_state.cash >= current_site.rent_cost;
            let button_enabled = !is_owned && can_afford;

            card.spawn((
                Button,
                Node {
                    width: Val::Percent(100.0),
                    height: Val::Px(48.0),
                    justify_content: JustifyContent::Center,
                    align_items: AlignItems::Center,
                    margin: UiRect::top(Val::Px(8.0)),
                    ..default()
                },
                BackgroundColor(if button_enabled {
                    Color::srgb(0.2, 0.6, 0.9)
                } else if is_owned {
                    Color::srgb(0.3, 0.3, 0.3)
                } else {
                    Color::srgb(0.5, 0.2, 0.2)
                }),
                BorderRadius::all(Val::Px(8.0)),
                RentSiteButton {
                    listing_index: carousel.current_index,
                },
            ))
            .with_child((
                Text::new(if is_owned {
                    "OWNED".to_string()
                } else if !can_afford {
                    format!(
                        "RENT - ${:.0} (Need ${:.0} more)",
                        current_site.rent_cost,
                        current_site.rent_cost - game_state.cash
                    )
                } else {
                    format!("RENT - ${:.0}", current_site.rent_cost)
                }),
                TextFont {
                    font_size: 16.0,
                    ..default()
                },
                TextColor(Color::WHITE),
            ));
        });
}

/// System to handle carousel navigation
pub fn handle_carousel_navigation(
    mut carousel: ResMut<RentCarouselState>,
    multi_site: Res<MultiSiteManager>,
    interaction_query: Query<(&Interaction, &CarouselButton), (Changed<Interaction>, With<Button>)>,
) {
    for (interaction, button) in &interaction_query {
        if *interaction == Interaction::Pressed {
            let available_count = multi_site.available_sites_list().len();
            if available_count == 0 {
                continue;
            }

            match button {
                CarouselButton::Previous => {
                    if carousel.current_index > 0 {
                        carousel.current_index -= 1;
                    }
                }
                CarouselButton::Next => {
                    if carousel.current_index < available_count - 1 {
                        carousel.current_index += 1;
                    }
                }
            }
        }
    }
}

/// System to handle rent button clicks
pub fn handle_rent_button(
    mut multi_site: ResMut<MultiSiteManager>,
    mut game_state: ResMut<GameState>,
    mut nav_state: ResMut<NavigationState>,
    mut switch_events: MessageWriter<SiteSwitchEvent>,
    template_cache: Res<crate::resources::SiteTemplateCache>,
    tiled_assets: Res<bevy::asset::Assets<bevy_ecs_tiled::prelude::TiledMapAsset>>,
    game_data: Res<crate::resources::GameDataAssets>,
    interaction_query: Query<(&Interaction, &RentSiteButton), (Changed<Interaction>, With<Button>)>,
) {
    for (interaction, rent_button) in &interaction_query {
        if *interaction == Interaction::Pressed {
            // Clone the listing data to avoid borrow issues
            let listing = {
                let available_sites = multi_site.available_sites_list();
                if rent_button.listing_index >= available_sites.len() {
                    continue;
                }
                available_sites[rent_button.listing_index].clone()
            };

            // Check if already owned
            if multi_site.owns_archetype(listing.archetype) {
                warn!("Site archetype {:?} already owned", listing.archetype);
                continue;
            }

            // Check if can afford
            if game_state.cash < listing.rent_cost {
                warn!(
                    "Cannot afford site: need ${:.0}, have ${:.0}",
                    listing.rent_cost, game_state.cash
                );
                continue;
            }

            // Rent the site
            match multi_site.rent_site(&listing, &template_cache, &tiled_assets, &game_data) {
                Ok(site_id) => {
                    game_state.spend_rent(listing.rent_cost);
                    info!(
                        "Rented site: {} ({:?}) for ${:.0}. New balance: ${:.0}",
                        listing.name, listing.archetype, listing.rent_cost, game_state.cash
                    );
                    info!("Site ID: {:?}", site_id);

                    // Automatically switch to the new site
                    switch_events.write(SiteSwitchEvent {
                        target_site_id: site_id,
                    });

                    // Switch to build panel for the new site
                    nav_state.set_primary(PrimaryNav::Build);
                }
                Err(e) => {
                    error!("Failed to rent site: {}", e);
                }
            }
        }
    }
}
