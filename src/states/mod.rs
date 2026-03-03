//! Game state management for controlling game flow.
//!
//! This module implements a state machine for managing different phases of the game:
//! - `MainMenu`: Initial menu screen
//! - `Loading`: Asset loading with progress display
//! - `CharacterSetup`: Character selection and name input
//! - `Playing`: Active gameplay (time can be paused/running, building always allowed)
//! - `Paused`: Game paused (time frozen, UI visible)
//! - `DayEnd`: End of day summary screen
//! - `GameOver`: End screen (win or lose)

pub mod character_setup;
pub mod loading;
pub mod main_menu;
pub mod playing;

use bevy::ecs::hierarchy::ChildSpawnerCommands;
use bevy::input::mouse::{MouseScrollUnit, MouseWheel};
use bevy::prelude::*;

use crate::resources::SelectedChargerEntity;

pub use character_setup::*;
pub use loading::*;
pub use main_menu::*;
pub use playing::*;

/// Primary application state for controlling game flow.
///
/// # State Transitions
/// ```text
/// MainMenu -> Loading -> CharacterSetup -> Playing <-> Paused
///                                             |
///                                             v
///                                          DayEnd -> Playing (next day)
///                                             |
///                                             v
///                                          GameOver -> MainMenu
/// ```
#[derive(States, Debug, Clone, Copy, Default, Eq, PartialEq, Hash)]
pub enum AppState {
    /// Main menu
    MainMenu,
    /// Loading assets and preparing the scene
    #[default]
    Loading,
    /// Character selection and name input
    CharacterSetup,
    /// Active gameplay (time can be paused/running, building always allowed)
    Playing,
    /// Game paused (from Playing state only)
    Paused,
    /// End of day summary screen
    DayEnd,
    /// Game over screen (win or lose)
    GameOver,
}

impl AppState {
    /// Returns true if the game simulation should be running
    pub fn is_simulation_active(&self) -> bool {
        matches!(self, AppState::Playing)
    }

    /// Returns true if we're in a state where the game world is visible
    pub fn is_game_visible(&self) -> bool {
        matches!(
            self,
            AppState::CharacterSetup
                | AppState::Playing
                | AppState::Paused
                | AppState::DayEnd
                | AppState::GameOver
        )
    }

    /// Returns true if we're in a menu state
    pub fn is_menu(&self) -> bool {
        matches!(self, AppState::MainMenu | AppState::GameOver)
    }
}

/// Plugin that sets up the state machine and state-related systems
pub struct StatesPlugin;

impl Plugin for StatesPlugin {
    fn build(&self, app: &mut App) {
        app.init_state::<AppState>()
            // Main menu systems - disabled (game now starts at Loading state)
            // .add_systems(OnEnter(AppState::MainMenu), setup_main_menu)
            // .add_systems(OnExit(AppState::MainMenu), cleanup_main_menu)
            // .add_systems(
            //     Update,
            //     (
            //         spawn_menu_background_when_ready,
            //         spawn_menu_ui_when_ready,
            //         main_menu_system,
            //         animate_lightning_bolt_glow,
            //     )
            //         .run_if(in_state(AppState::MainMenu)),
            // )
            // Loading systems - full loading screen with progress tracking
            .add_systems(
                OnEnter(AppState::Loading),
                (setup_loading_screen, start_asset_loading),
            )
            .add_systems(OnExit(AppState::Loading), cleanup_loading_screen)
            .add_systems(
                Update,
                (
                    update_loading_progress,
                    populate_template_cache,
                    populate_available_sites,
                    populate_game_data_from_assets,
                    check_loading_complete,
                    update_minimal_loading_indicator,
                )
                    .run_if(in_state(AppState::Loading)),
            )
            // Character setup systems (run as overlay during Playing, gated on SetupStep resource)
            .add_systems(
                Update,
                (
                    handle_character_selection,
                    handle_character_card_hover,
                    handle_next_button,
                    handle_name_input,
                    animate_cursor,
                    cycle_placeholder_names,
                    handle_start_button,
                )
                    .run_if(in_state(AppState::Playing).and(resource_exists::<SetupStep>)),
            )
            // Playing state - most game systems run here
            .add_systems(OnEnter(AppState::Playing), on_enter_playing)
            .add_systems(OnExit(AppState::Playing), on_exit_playing)
            // Paused state
            .add_systems(OnEnter(AppState::Paused), on_enter_paused)
            .add_systems(OnExit(AppState::Paused), on_exit_paused)
            .add_systems(Update, pause_menu_system.run_if(in_state(AppState::Paused)))
            // DayEnd state
            .add_systems(
                OnEnter(AppState::DayEnd),
                (
                    (
                        cleanup_entities_on_day_end,
                        cleanup_hacker_entities_on_day_end,
                    ),
                    on_enter_day_end,
                    auto_submit_score_on_day_end,
                )
                    .chain(),
            )
            .add_systems(OnExit(AppState::DayEnd), on_exit_day_end)
            .add_systems(
                Update,
                (day_end_system, kpi_toggle_system, linkedin_share_system)
                    .run_if(in_state(AppState::DayEnd)),
            )
            // Global UI scroll for any container with overflow: scroll_y + ScrollPosition
            .init_resource::<TouchScrollState>()
            .add_systems(Update, (ui_scroll_system, ui_touch_scroll_system))
            // GameOver state
            .add_systems(OnEnter(AppState::GameOver), setup_game_over)
            .add_systems(OnExit(AppState::GameOver), cleanup_game_over)
            .add_systems(
                Update,
                game_over_system.run_if(in_state(AppState::GameOver)),
            )
            // Pause toggle works in both Playing and Paused states
            .add_systems(
                Update,
                toggle_pause.run_if(in_state(AppState::Playing).or(in_state(AppState::Paused))),
            );
    }
}

/// Marker component for main menu UI elements
#[derive(Component)]
pub struct MainMenuUI;

/// Marker component for character setup UI elements
#[derive(Component)]
pub struct CharacterSetupUI;

/// Marker component for pause menu UI elements
#[derive(Component)]
pub struct PauseMenuUI;

/// Marker component for game over UI elements
#[derive(Component)]
pub struct GameOverUI;

/// Marker component for day end UI elements
#[derive(Component)]
pub struct DayEndUI;

/// Continue to next day button marker for day end modal
#[derive(Component, Debug, Clone, Copy)]
pub struct DayEndContinueButton;

/// Marker for the collapsed KPI view (flat summary rows)
#[derive(Component, Debug, Clone, Copy)]
struct KpiCollapsed;

/// Marker for the expanded KPI view (full Revenue/Energy/Operations breakdown)
#[derive(Component, Debug, Clone, Copy)]
struct KpiExpanded;

/// Toggle button between collapsed and expanded KPI views
#[derive(Component, Debug, Clone, Copy)]
pub struct KpiToggleButton;

/// Marker for the scrollable body of the day-end modal (used by test bridge).
#[derive(Component, Debug, Clone, Copy)]
pub struct DayEndScrollBody;

/// Marker for the modal container so the toggle system can change its width
#[derive(Component, Debug, Clone, Copy)]
struct DayEndModalContainer;

/// Share on LinkedIn button marker
#[derive(Component, Debug, Clone, Copy)]
struct LinkedInShareButton;

/// Resource storing LinkedIn share text for the current day end
#[derive(Resource, Debug, Clone)]
struct DayEndShareText(String);

/// System to toggle pause state with Escape key
fn toggle_pause(
    keyboard: Res<ButtonInput<KeyCode>>,
    current_state: Res<State<AppState>>,
    mut next_state: ResMut<NextState<AppState>>,
    selected: Res<SelectedChargerEntity>,
    tutorial: Res<crate::resources::TutorialState>,
) {
    if keyboard.just_pressed(KeyCode::Escape) {
        // Don't toggle pause if radial menu is open - Escape dismisses it instead
        if selected.0.is_some() {
            return;
        }
        // Don't toggle pause if the tutorial is active — Escape skips the tutorial instead
        if tutorial.is_active() {
            return;
        }
        match current_state.get() {
            AppState::Playing => next_state.set(AppState::Paused),
            AppState::Paused => next_state.set(AppState::Playing),
            _ => {}
        }
    }
}

/// System to handle pause menu interactions
fn pause_menu_system(
    keyboard: Res<ButtonInput<KeyCode>>,
    mut next_state: ResMut<NextState<AppState>>,
    mut interaction_query: Query<
        (&Interaction, &PauseMenuButton),
        (Changed<Interaction>, With<Button>),
    >,
) {
    // Handle button clicks
    for (interaction, button) in &mut interaction_query {
        if *interaction == Interaction::Pressed {
            match button {
                PauseMenuButton::Resume => next_state.set(AppState::Playing),
                PauseMenuButton::MainMenu => next_state.set(AppState::Loading),
            }
        }
    }

    // R key to resume
    if keyboard.just_pressed(KeyCode::KeyR) {
        next_state.set(AppState::Playing);
    }
}

/// Types of buttons in the pause menu
#[derive(Component, Debug, Clone, Copy)]
pub enum PauseMenuButton {
    Resume,
    MainMenu,
}

/// Called when entering the paused state
fn on_enter_paused(mut commands: Commands) {
    // Spawn pause menu overlay
    commands
        .spawn((
            PauseMenuUI,
            Node {
                width: Val::Percent(100.0),
                height: Val::Percent(100.0),
                position_type: PositionType::Absolute,
                justify_content: JustifyContent::Center,
                align_items: AlignItems::Center,
                flex_direction: FlexDirection::Column,
                row_gap: Val::Px(20.0),
                ..default()
            },
            BackgroundColor(Color::srgba(0.0, 0.0, 0.0, 0.7)),
            GlobalZIndex(1000),
        ))
        .with_children(|parent| {
            // Title
            parent.spawn((
                Text::new("PAUSED"),
                TextFont {
                    font_size: 48.0,
                    ..default()
                },
                TextColor(Color::WHITE),
            ));

            // Resume button
            parent
                .spawn((
                    Button,
                    Node {
                        width: Val::Px(200.0),
                        height: Val::Px(50.0),
                        justify_content: JustifyContent::Center,
                        align_items: AlignItems::Center,
                        ..default()
                    },
                    BackgroundColor(Color::srgb(0.2, 0.6, 0.2)),
                    PauseMenuButton::Resume,
                ))
                .with_children(|btn| {
                    btn.spawn((
                        Text::new("Resume (R)"),
                        TextFont {
                            font_size: 20.0,
                            ..default()
                        },
                        TextColor(Color::WHITE),
                    ));
                });

            // Main menu button
            parent
                .spawn((
                    Button,
                    Node {
                        width: Val::Px(200.0),
                        height: Val::Px(50.0),
                        justify_content: JustifyContent::Center,
                        align_items: AlignItems::Center,
                        ..default()
                    },
                    BackgroundColor(Color::srgb(0.6, 0.2, 0.2)),
                    PauseMenuButton::MainMenu,
                ))
                .with_children(|btn| {
                    btn.spawn((
                        Text::new("Main Menu"),
                        TextFont {
                            font_size: 20.0,
                            ..default()
                        },
                        TextColor(Color::WHITE),
                    ));
                });
        });
}

/// Called when exiting the paused state
fn on_exit_paused(mut commands: Commands, query: Query<Entity, With<PauseMenuUI>>) {
    for entity in &query {
        commands.entity(entity).try_despawn();
    }
}

/// Called when entering the playing state
fn on_enter_playing(
    mut commands: Commands,
    mut game_state: ResMut<crate::resources::GameState>,
    mut multi_site: ResMut<crate::resources::MultiSiteManager>,
    mut game_clock: ResMut<crate::resources::GameClock>,
    achievement_state: Res<crate::resources::achievements::AchievementState>,
) {
    // Reset game state if coming from game over
    if game_state.result.is_ended() {
        game_state.reset();
        // Also reset MultiSiteManager so initialize_game_on_first_play can rent starter site again
        *multi_site = crate::resources::MultiSiteManager::default();
        game_clock.reset();
    }

    // Set ledger date for this day
    if let Some(date) =
        chrono::NaiveDate::from_ymd_opt(game_clock.year as i32, game_clock.month, game_clock.day)
    {
        game_state.ledger.current_date = date;
    }

    // Snapshot achievements at day start so we can diff at day end
    commands.insert_resource(crate::resources::achievements::AchievementSnapshot {
        unlocked_at_day_start: achievement_state.snapshot(),
    });
}

/// Called when exiting the playing state
fn on_exit_playing() {
    // Placeholder for any cleanup needed
}

/// Despawn all vehicles, robbers, and sprites when entering DayEnd.
/// Runs before the day-end UI is built so entities disappear immediately.
fn cleanup_entities_on_day_end(
    mut commands: Commands,
    drivers: Query<Entity, With<crate::components::driver::Driver>>,
    ambient_vehicles: Query<Entity, With<crate::systems::ambient_traffic::AmbientVehicle>>,
    vehicle_sprites: Query<Entity, With<crate::systems::sprite::VehicleSprite>>,
    character_sprites: Query<Entity, With<crate::systems::sprite::DriverCharacterSprite>>,
    robbers: Query<Entity, With<crate::components::robber::Robber>>,
    theft_alarms: Query<Entity, With<crate::systems::sprite::TheftAlarmVfx>>,
    stealing_sparks: Query<Entity, With<crate::systems::sprite::StealingSparkVfx>>,
    loot_bubbles: Query<Entity, With<crate::systems::sprite::RobberLootBubble>>,
    stolen_cables: Query<Entity, With<crate::systems::sprite::StolenCableSprite>>,
    firetrucks: Query<Entity, With<crate::systems::power::EmergencyFiretruck>>,
    fire_vfx: Query<Entity, With<crate::systems::power::PixelFire>>,
    water_vfx: Query<Entity, With<crate::systems::power::TransformerWaterVfx>>,
) {
    for entity in &drivers {
        commands.entity(entity).try_despawn();
    }
    for entity in &ambient_vehicles {
        commands.entity(entity).try_despawn();
    }
    for entity in &vehicle_sprites {
        commands.entity(entity).try_despawn();
    }
    for entity in &character_sprites {
        commands.entity(entity).try_despawn();
    }
    for entity in &robbers {
        commands.entity(entity).try_despawn();
    }
    for entity in &theft_alarms {
        commands.entity(entity).try_despawn();
    }
    for entity in &stealing_sparks {
        commands.entity(entity).try_despawn();
    }
    for entity in &loot_bubbles {
        commands.entity(entity).try_despawn();
    }
    for entity in &stolen_cables {
        commands.entity(entity).try_despawn();
    }
    for entity in &firetrucks {
        commands.entity(entity).try_despawn();
    }
    for entity in &fire_vfx {
        commands.entity(entity).try_despawn();
    }
    for entity in &water_vfx {
        commands.entity(entity).try_despawn();
    }
}

/// Despawn hacker entities and cancel active hacker effects at day end.
fn cleanup_hacker_entities_on_day_end(
    mut commands: Commands,
    hackers: Query<Entity, With<crate::components::hacker::Hacker>>,
    hacker_glitch_vfx: Query<Entity, With<crate::systems::sprite::HackingGlitchVfx>>,
    hacker_loot_bubbles: Query<Entity, With<crate::systems::sprite::HackerLootBubble>>,
    mut multi_site: ResMut<crate::resources::MultiSiteManager>,
) {
    for entity in &hackers {
        commands.entity(entity).try_despawn();
    }
    for entity in &hacker_glitch_vfx {
        commands.entity(entity).try_despawn();
    }
    for entity in &hacker_loot_bubbles {
        commands.entity(entity).try_despawn();
    }

    for (_site_id, site_state) in multi_site.owned_sites.iter_mut() {
        site_state.hacker_overload_remaining_secs = 0.0;
        site_state.service_strategy.pricing.hacker_price_override = None;
        site_state
            .service_strategy
            .pricing
            .hacker_price_override_remaining_secs = 0.0;
    }
}

/// Called when entering the day end state
fn on_enter_day_end(
    mut commands: Commands,
    game_clock: Res<crate::resources::GameClock>,
    mut game_state: ResMut<crate::resources::GameState>,
    mut multi_site: ResMut<crate::resources::MultiSiteManager>,
    carbon_market: Res<crate::resources::site_energy::CarbonCreditMarket>,
    player_profile: Res<crate::resources::PlayerProfile>,
    image_assets: Res<crate::resources::ImageAssets>,
    achievement_state: Res<crate::resources::achievements::AchievementState>,
    achievement_snapshot: Option<Res<crate::resources::achievements::AchievementSnapshot>>,
    chargers: Query<&crate::components::charger::Charger>,
    transformers: Query<&crate::components::power::Transformer>,
) {
    info!("Day {} complete!", game_clock.day);

    // Calculate carbon credits from energy delivered at the active site
    let rate_per_kwh = carbon_market.rate_per_kwh();
    let total_carbon_credits = if let Some(site_state) = multi_site.active_site_mut() {
        let carbon_credit_revenue = site_state.energy_delivered_kwh_today * rate_per_kwh;

        if site_state.energy_delivered_kwh_today > 0.0
            && site_state.utility_meter.total_imported_kwh() == 0.0
        {
            game_state.zero_grid_day_achieved = true;
        }

        site_state.energy_delivered_kwh_today = 0.0;
        site_state.sessions_today = 0;
        carbon_credit_revenue
    } else {
        0.0
    };

    // Flush all accumulated per-site costs to the ledger before verification
    game_state.flush_site_costs(&mut multi_site.owned_sites);

    // Add carbon credit revenue to game state
    game_state.add_carbon_credit_revenue(total_carbon_credits);

    info!(
        "Carbon credits: {:.1} kWh delivered at ${:.2}/kWh = ${:.2}",
        total_carbon_credits / rate_per_kwh,
        rate_per_kwh,
        total_carbon_credits
    );

    // Verify ledger cash balance matches game state
    if let Err(e) = game_state.ledger.verify_cash(game_state.cash) {
        error!("Ledger balance verification failed: {e}");
    }

    // Build DailyRecord from the ledger (financial fields) + tracker (non-financial stats)
    let site_id = game_state
        .daily_history
        .current_day
        .site_id
        .unwrap_or(crate::resources::multi_site::SiteId(1));
    let reputation_change =
        game_state.reputation - game_state.daily_history.current_day.starting_reputation;
    let financials = game_state
        .ledger
        .daily_totals(game_state.ledger.current_date);

    let daily_record = crate::resources::game_state::DailyRecord::from_ledger(
        &financials,
        game_clock.day,
        game_clock.month,
        game_clock.year,
        site_id,
        game_state.daily_history.current_day.sessions,
        game_state.daily_history.current_day.sessions_failed_today,
        game_state.daily_history.current_day.dispatches,
        reputation_change,
    );

    // Calculate values for display (extract before pushing to history)
    let charging_revenue = daily_record.financials.charging_revenue;
    let total_revenue = daily_record.total_revenue();
    let carbon_credits = daily_record.financials.carbon_credits;
    let solar_export_revenue = daily_record.financials.solar_export_revenue;
    let energy_cost = daily_record.financials.energy_cost;
    let demand_charge = daily_record.financials.demand_charge;
    let repair_parts = daily_record.financials.repair_parts;
    let repair_labor = daily_record.financials.repair_labor;
    let maintenance = daily_record.financials.maintenance;
    let opex = repair_parts + repair_labor + maintenance;
    let cable_theft_cost = daily_record.financials.cable_theft_cost;
    let warranty_cost = daily_record.financials.warranty_cost;
    let warranty_recovery = daily_record.financials.warranty_recovery;
    let refunds = daily_record.financials.refunds;
    let penalties = daily_record.financials.penalties;
    let rent = daily_record.financials.rent;
    let upgrades = daily_record.financials.upgrades;
    let net_profit = daily_record.net_profit();
    let sessions_delta = daily_record.sessions;
    let sessions_failed_today = daily_record.sessions_failed_today;
    let dispatches_delta = daily_record.dispatches;
    let reputation_delta = daily_record.reputation_change;

    // Snapshot charger online/total counts
    let chargers_total = chargers.iter().count() as i32;
    let chargers_online = chargers
        .iter()
        .filter(|c| {
            !matches!(
                c.state(),
                crate::components::charger::ChargerState::Offline
                    | crate::components::charger::ChargerState::Disabled
            )
        })
        .count() as i32;

    // Count pending cable theft repairs (chargers still faulted with CableTheft)
    let pending_cable_thefts: u32 = chargers
        .iter()
        .filter(|c| {
            matches!(
                c.current_fault,
                Some(crate::components::charger::FaultType::CableTheft)
            )
        })
        .count() as u32;
    let pending_cable_cost = pending_cable_thefts as f32 * 2000.0;

    let destroyed_transformers = transformers.iter().filter(|t| t.destroyed).count() as i32;

    // Store the record in history
    game_state.daily_history.records.push(daily_record);

    // Helper to format delta with + or -
    let format_delta = |val: f32| -> String {
        if val >= 0.0 {
            format!("+${val:.2}")
        } else {
            format!("-${:.2}", val.abs())
        }
    };

    let format_int_delta = |val: i32| -> String {
        if val >= 0 {
            format!("+{val}")
        } else {
            format!("{val}")
        }
    };

    // Get the character avatar handle
    let avatar_handle = match player_profile.character {
        Some(crate::resources::player_profile::CharacterKind::Ant) => {
            image_assets.character_main_ant.clone()
        }
        Some(crate::resources::player_profile::CharacterKind::Mallard) => {
            image_assets.character_main_mallard.clone()
        }
        Some(crate::resources::player_profile::CharacterKind::Raccoon) => {
            image_assets.character_main_raccoon.clone()
        }
        None => image_assets.character_main_ant.clone(),
    };

    // Get character display info
    let char_name = player_profile
        .character
        .map(|c| c.display_name())
        .unwrap_or("Player");
    let char_role = player_profile
        .character
        .map(|c| c.role())
        .unwrap_or("Operator");

    // Find achievements newly unlocked today (highest tier first)
    let empty_snapshot = std::collections::HashSet::new();
    let snapshot = achievement_snapshot
        .as_ref()
        .map(|s| &s.unlocked_at_day_start)
        .unwrap_or(&empty_snapshot);
    let new_badges = achievement_state.newly_unlocked_since(snapshot);

    // Compose LinkedIn share text and store as resource
    let total_income = total_revenue + carbon_credits;
    let share_text = format!(
        "I just finished Day {} of Kilowatt Tycoon ⚡\n\n${:.0} in revenue\n{} sessions\n{} dispatches\n\nIs this really what it feels like to run an EV charging empire?\n\n#KilowattTycoon #EVCharging",
        game_clock.day, total_income, sessions_delta, dispatches_delta,
    );
    commands.insert_resource(DayEndShareText(share_text));

    // Precompute values for both KPI views.
    // Operating expenses exclude fixed costs (rent, upgrades) so
    // Operating Profit = Revenue - Operating Expenses.
    let total_energy = financials.category_total(crate::resources::ledger::ExpenseCategory::Energy);
    let total_opex =
        financials.category_total(crate::resources::ledger::ExpenseCategory::Operations);
    let total_fixed = financials.category_total(crate::resources::ledger::ExpenseCategory::Fixed);
    let total_expenses = total_energy + total_opex;
    let operating_profit = total_income - total_expenses;

    let profit_color = if operating_profit >= 0.0 {
        Color::srgb(0.4, 0.9, 0.4)
    } else {
        Color::srgb(0.9, 0.4, 0.4)
    };
    let rep_color = if reputation_delta >= 0 {
        Color::srgb(0.4, 0.9, 0.4)
    } else {
        Color::srgb(0.9, 0.4, 0.4)
    };

    // Compute day title and pro-tip
    let (title_text, subtitle_text) = day_title(
        game_clock.day,
        net_profit,
        reputation_delta,
        sessions_delta,
        charging_revenue,
        energy_cost,
        opex,
        warranty_cost,
        warranty_recovery,
    );
    let pro_tip_text = generate_pro_tip(
        char_name,
        net_profit,
        sessions_delta,
        charging_revenue,
        energy_cost,
        opex,
        reputation_delta,
        warranty_cost,
        warranty_recovery,
        destroyed_transformers,
    );

    // Compute per-kWh pricing for expanded view
    let total_imported_kwh: f32 = multi_site
        .owned_sites
        .values()
        .map(|s| s.utility_meter.total_imported_kwh())
        .sum();
    let avg_sell_price_kwh: f32 = if total_imported_kwh > 0.01 {
        // Blended customer sell price: total charging revenue / total kWh imported
        // (approximation -- uses imported kWh as proxy for delivered kWh)
        charging_revenue / total_imported_kwh
    } else {
        0.0
    };
    let avg_buy_price_kwh: f32 = if total_imported_kwh > 0.01 {
        energy_cost / total_imported_kwh
    } else {
        0.0
    };

    let has_solar = multi_site
        .owned_sites
        .values()
        .any(|s| s.grid.total_solar_kw > 0.0);

    // Grid event stats for the viewed site (only meaningful for challenge_level >= 2)
    let grid_event_revenue: f32 = multi_site
        .active_site()
        .filter(|s| s.challenge_level >= 2)
        .map(|s| s.grid_events.event_revenue_today)
        .unwrap_or(0.0);
    let grid_event_import_surcharge: f32 = multi_site
        .active_site()
        .filter(|s| s.challenge_level >= 2)
        .map(|s| s.grid_events.event_import_surcharge_today)
        .unwrap_or(0.0);
    let best_spike: Option<(&'static str, f32)> = multi_site
        .active_site()
        .filter(|s| s.challenge_level >= 2)
        .and_then(|s| {
            s.grid_events
                .best_event_type
                .map(|e| (e.name(), s.grid_events.best_event_export_multiplier))
        });

    // Revenue/cost hints for collapsed view
    let revenue_hint: Option<&str> = if charging_revenue < energy_cost && energy_cost > 0.01 {
        Some("(Pricing: Too Low?)")
    } else {
        None
    };
    let expense_hint: Option<&str> = if total_opex > total_energy && total_opex > 0.01 {
        Some("(Mostly Repairs...)")
    } else if total_energy > 0.01 {
        Some("(Energy Costs)")
    } else {
        None
    };

    // Unit economy (per-session)
    let revenue_per_session = if sessions_delta > 0 {
        charging_revenue / sessions_delta as f32
    } else {
        0.0
    };
    let cost_per_session = if sessions_delta > 0 {
        energy_cost / sessions_delta as f32
    } else {
        0.0
    };

    // Get the highest-tier badge earned today (new_badges is sorted highest-tier first)
    let top_badge_data: Option<(
        crate::resources::achievements::AchievementKind,
        Handle<Image>,
    )> = new_badges.first().map(|badge| {
        let icon = match badge.tier() {
            crate::resources::achievements::AchievementTier::Bronze => {
                image_assets.icon_medal_bronze.clone()
            }
            crate::resources::achievements::AchievementTier::Silver => {
                image_assets.icon_medal_silver.clone()
            }
            crate::resources::achievements::AchievementTier::Gold => {
                image_assets.icon_medal_gold.clone()
            }
        };
        (*badge, icon)
    });

    // Spawn day end overlay (dim background)
    commands
        .spawn((
            DayEndUI,
            Node {
                width: Val::Percent(100.0),
                height: Val::Percent(100.0),
                position_type: PositionType::Absolute,
                justify_content: JustifyContent::Center,
                align_items: AlignItems::Center,
                ..default()
            },
            BackgroundColor(Color::srgba(0.0, 0.0, 0.0, 0.7)),
            GlobalZIndex(1000),
        ))
        .with_children(|overlay| {
                // Modal container (width toggled by kpi_toggle_system; buttons always visible at bottom)
                overlay
                    .spawn((
                        DayEndModalContainer,
                        Node {
                            width: Val::Px(480.0),
                            max_height: Val::Percent(85.0),
                            flex_direction: FlexDirection::Column,
                            ..default()
                        },
                        BackgroundColor(Color::srgb(0.12, 0.14, 0.18)),
                        BorderColor::all(Color::srgb(0.3, 0.35, 0.4)),
                        BorderRadius::all(Val::Px(12.0)),
                    ))
                    .with_children(|modal_outer| {
                        // Scrollable body — grows to fill available height so buttons stay pinned below
                        modal_outer
                            .spawn((
                                DayEndScrollBody,
                                Node {
                                    width: Val::Percent(100.0),
                                    flex_direction: FlexDirection::Column,
                                    flex_grow: 1.0,
                                    overflow: Overflow::scroll_y(),
                                    ..default()
                                },
                                ScrollPosition::default(),
                            ))
                            .with_children(|scroll_outer| {
                        // Inner content with padding
                        scroll_outer
                            .spawn(Node {
                                width: Val::Percent(100.0),
                                flex_direction: FlexDirection::Column,
                                padding: UiRect::all(Val::Px(24.0)),
                                row_gap: Val::Px(12.0),
                                ..default()
                            })
                            .with_children(|modal| {
                            // ===== HEADER =====
                            modal
                                .spawn(Node {
                                    width: Val::Percent(100.0),
                                    flex_direction: FlexDirection::Column,
                                    align_items: AlignItems::Center,
                                    row_gap: Val::Px(4.0),
                                    ..default()
                                })
                                .with_children(|header| {
                                    header.spawn((
                                        Text::new(format!("DAY {} COMPLETE", game_clock.day)),
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

                            // ===== AVATAR SECTION =====
                            modal
                                .spawn(Node {
                                    width: Val::Percent(100.0),
                                    flex_direction: FlexDirection::Row,
                                    column_gap: Val::Px(16.0),
                                    align_items: AlignItems::Center,
                                    padding: UiRect::vertical(Val::Px(8.0)),
                                    ..default()
                                })
                                .with_children(|avatar_row| {
                                    // Character avatar
                                    avatar_row.spawn((
                                        ImageNode::new(avatar_handle),
                                        Node {
                                            width: Val::Px(64.0),
                                            height: Val::Px(64.0),
                                            border: UiRect::all(Val::Px(2.0)),
                                            ..default()
                                        },
                                        BorderColor::all(Color::srgb(0.3, 0.6, 0.8)),
                                        BorderRadius::all(Val::Px(4.0)),
                                    ));

                                    // Name + Role
                                    avatar_row
                                        .spawn(Node {
                                            flex_direction: FlexDirection::Column,
                                            row_gap: Val::Px(4.0),
                                            ..default()
                                        })
                                        .with_children(|info| {
                                            info.spawn((
                                                Text::new(char_name),
                                                TextFont {
                                                    font_size: 20.0,
                                                    ..default()
                                                },
                                                TextColor(Color::WHITE),
                                            ));
                                            info.spawn((
                                                Text::new(format!("Role: {char_role}")),
                                                TextFont {
                                                    font_size: 14.0,
                                                    ..default()
                                                },
                                                TextColor(Color::srgb(0.5, 0.7, 0.5)),
                                            ));
                                        });
                                });

                            // ===== KPI SECTION =====
                            // Section header row with toggle button
                            modal
                                .spawn(Node {
                                    width: Val::Percent(100.0),
                                    flex_direction: FlexDirection::Row,
                                    justify_content: JustifyContent::SpaceBetween,
                                    align_items: AlignItems::Center,
                                    ..default()
                                })
                                .with_children(|kpi_header| {
                                    // Divider line + label
                                    kpi_header.spawn((
                                        Text::new("KPI SNAPSHOT"),
                                        TextFont {
                                            font_size: 14.0,
                                            ..default()
                                        },
                                        TextColor(Color::srgb(0.5, 0.5, 0.5)),
                                    ));

                                    // Toggle button
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
                            modal.spawn((
                                Node {
                                    width: Val::Percent(100.0),
                                    height: Val::Px(1.0),
                                    ..default()
                                },
                                BackgroundColor(Color::srgb(0.3, 0.35, 0.4)),
                            ));

                            // --- KPI area (stacked: toggle swaps between expanded and collapsed) ---
                            modal
                                .spawn(Node {
                                    width: Val::Percent(100.0),
                                    flex_direction: FlexDirection::Column,
                                    ..default()
                                })
                                .with_children(|kpi_row| {
                                    // --- KPI Expanded (hidden by default, shown on toggle) ---
                                    kpi_row
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
                                            // ===== A. Energy Margin =====
                                            let energy_color = Color::srgb(0.9, 0.7, 0.4);
                                            spawn_section_header(section, "Energy Margin", "[~]", energy_color);
                                            spawn_indented_row(
                                                section,
                                                "  Customer Price",
                                                &format!("${:.2} / kWh", avg_sell_price_kwh),
                                                Color::srgb(0.7, 0.7, 0.7),
                                            );
                                            spawn_indented_row(
                                                section,
                                                "  Grid Cost",
                                                &format!("${:.2} / kWh", avg_buy_price_kwh),
                                                Color::srgb(0.7, 0.7, 0.7),
                                            );
                                            spawn_indented_row(
                                                section,
                                                "  Charging Revenue",
                                                &format!("+${charging_revenue:.2}"),
                                                Color::srgb(0.4, 0.9, 0.4),
                                            );
                                            spawn_indented_row(
                                                section,
                                                "  Energy Costs",
                                                &format!("-${:.2}", energy_cost + demand_charge),
                                                Color::srgb(0.9, 0.7, 0.4),
                                            );
                                            let net_energy_margin = charging_revenue - energy_cost - demand_charge;
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
                                                energy_margin_insight(charging_revenue, energy_cost),
                                            );

                                            // ===== B. Operations =====
                                            let ops_color = Color::srgb(0.9, 0.4, 0.4);
                                            spawn_section_header(section, "Operations", "[*]", ops_color);
                                            if repair_parts > 0.01 {
                                                spawn_indented_row(
                                                    section,
                                                    "  Repair Parts",
                                                    &format!("-${:.2}", repair_parts),
                                                    ops_color,
                                                );
                                            }
                                            if repair_labor > 0.01 {
                                                spawn_indented_row(
                                                    section,
                                                    "  Repair Labor",
                                                    &format!("-${:.2}", repair_labor),
                                                    ops_color,
                                                );
                                            }
                                            if maintenance > 0.01 {
                                                spawn_indented_row(
                                                    section,
                                                    "  Maintenance/Amenities",
                                                    &format!("-${:.2}", maintenance),
                                                    Color::srgb(0.9, 0.6, 0.4),
                                                );
                                            }
                                            if cable_theft_cost > 0.01 {
                                                spawn_indented_row(
                                                    section,
                                                    "  Cable Theft (repaired)",
                                                    &format!("-${:.2}", cable_theft_cost),
                                                    Color::srgb(1.0, 0.2, 0.2),
                                                );
                                            }
                                            if pending_cable_thefts > 0 {
                                                spawn_indented_row(
                                                    section,
                                                    "  Cable Theft (pending)",
                                                    &format!(
                                                        "-${:.0} ({}x $2k)",
                                                        pending_cable_cost, pending_cable_thefts
                                                    ),
                                                    Color::srgb(1.0, 0.4, 0.1),
                                                );
                                            }
                                            if warranty_cost > 0.01 {
                                                spawn_indented_row(
                                                    section,
                                                    "  Warranty Premium",
                                                    &format!("-${:.2}", warranty_cost),
                                                    ops_color,
                                                );
                                            }
                                            if warranty_recovery > 0.01 {
                                                spawn_indented_row(
                                                    section,
                                                    "  Warranty Recovery",
                                                    &format!("+${:.2}", warranty_recovery),
                                                    Color::srgb(0.4, 0.9, 0.4),
                                                );
                                            }
                                            if refunds > 0.01 {
                                                spawn_indented_row(
                                                    section,
                                                    "  Refunds",
                                                    &format!("-${:.2}", refunds),
                                                    ops_color,
                                                );
                                            }
                                            if penalties > 0.01 {
                                                spawn_indented_row(
                                                    section,
                                                    "  Penalties",
                                                    &format!("-${:.2}", penalties),
                                                    ops_color,
                                                );
                                            }
                                            spawn_section_divider(section, ops_color);
                                            spawn_indented_row(
                                                section,
                                                "  Total OPEX",
                                                &format!("-${total_opex:.2}"),
                                                ops_color,
                                            );
                                            spawn_insight_row(
                                                section,
                                                operations_insight(opex, cable_theft_cost, dispatches_delta, warranty_recovery),
                                            );

                                            // ===== B2. Fixed Costs =====
                                            if total_fixed > 0.01 {
                                                let fixed_color = Color::srgb(0.7, 0.55, 0.9);
                                                spawn_section_header(section, "Fixed Costs", "[=]", fixed_color);
                                                if rent > 0.01 {
                                                    spawn_indented_row(
                                                        section,
                                                        "  Site Rent",
                                                        &format!("-${:.2}", rent),
                                                        fixed_color,
                                                    );
                                                }
                                                if upgrades > 0.01 {
                                                    spawn_indented_row(
                                                        section,
                                                        "  Upgrades",
                                                        &format!("-${:.2}", upgrades),
                                                        fixed_color,
                                                    );
                                                }
                                            }

                                            // ===== C. Reputation =====
                                            let rep_section_color = Color::srgb(0.4, 0.7, 0.9);
                                            spawn_section_header(section, "Reputation", "[#]", rep_section_color);
                                            spawn_indented_row(
                                                section,
                                                "  Successful Charges",
                                                &format!("+{}", sessions_delta),
                                                Color::srgb(0.4, 0.9, 0.4),
                                            );
                                            if sessions_failed_today > 0 {
                                                spawn_indented_row(
                                                    section,
                                                    "  Angry Drivers",
                                                    &format!("{}", sessions_failed_today),
                                                    Color::srgb(0.9, 0.4, 0.4),
                                                );
                                            }
                                            spawn_indented_row(
                                                section,
                                                "  Charger Availability",
                                                &format!("{}/{} online", chargers_online, chargers_total),
                                                if chargers_online < chargers_total {
                                                    Color::srgb(0.9, 0.7, 0.4)
                                                } else {
                                                    Color::srgb(0.4, 0.9, 0.4)
                                                },
                                            );
                                            spawn_section_divider(section, rep_section_color);
                                            spawn_indented_row(
                                                section,
                                                "  Net Change",
                                                &format_int_delta(reputation_delta),
                                                rep_color,
                                            );
                                            spawn_insight_row(
                                                section,
                                                reputation_insight(
                                                    reputation_delta,
                                                    sessions_delta,
                                                    sessions_failed_today,
                                                    chargers_online,
                                                    chargers_total,
                                                ),
                                            );

                                            // ===== D. Unit Economy =====
                                            let unit_color = Color::srgb(0.85, 0.85, 0.85);
                                            spawn_section_header(section, "Unit Economy", "[$]", unit_color);
                                            if sessions_delta > 0 {
                                                spawn_indented_row(
                                                    section,
                                                    "  Revenue / Session",
                                                    &format!("${:.2}", revenue_per_session),
                                                    Color::srgb(0.4, 0.9, 0.4),
                                                );
                                                spawn_indented_row(
                                                    section,
                                                    "  Energy Cost / Session",
                                                    &format!("-${:.2}", cost_per_session),
                                                    Color::srgb(0.9, 0.7, 0.4),
                                                );
                                                let margin_per = revenue_per_session - cost_per_session;
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
                                                    &unit_economy_verdict(revenue_per_session, cost_per_session),
                                                );
                                            } else {
                                                spawn_insight_row(
                                                    section,
                                                    "No sessions to analyze. Build more chargers or check your pricing!",
                                                );
                                            }

                                            // ===== E. Solar Export =====
                                            if has_solar {
                                                let solar_color = Color::srgb(1.0, 0.85, 0.1);
                                                spawn_section_header(section, "Solar Export", "[>]", solar_color);
                                                if solar_export_revenue > 0.01 {
                                                    spawn_indented_row(
                                                        section,
                                                        "  Grid Sellback",
                                                        &format!("+${solar_export_revenue:.2}"),
                                                        Color::srgb(0.4, 0.9, 0.4),
                                                    );
                                                    if grid_event_revenue > 0.01 {
                                                        spawn_indented_row(
                                                            section,
                                                            "  Event Revenue",
                                                            &format!("+${grid_event_revenue:.2}"),
                                                            Color::srgb(1.0, 0.78, 0.1),
                                                        );
                                                    }
                                                    if grid_event_import_surcharge > 0.01 {
                                                        spawn_indented_row(
                                                            section,
                                                            "  Event Surcharge",
                                                            &format!("-${grid_event_import_surcharge:.2}"),
                                                            Color::srgb(1.0, 0.4, 0.4),
                                                        );
                                                    }
                                                    if let Some((name, mult)) = best_spike {
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

                                    // --- KPI Summary (always visible, hidden when expanded) ---
                                    kpi_row
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
                                            // Dynamic day title
                                            section.spawn((
                                                Text::new(format!("DAY {}: \"{}\"", game_clock.day, title_text)),
                                                TextFont {
                                                    font_size: 20.0,
                                                    ..default()
                                                },
                                                TextColor(Color::srgb(0.4, 0.8, 1.0)),
                                            ));
                                            section.spawn((
                                                Text::new(subtitle_text),
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

                                            // KPI rows with status hints
                                            spawn_stat_row_with_hint(
                                                section,
                                                "Total Revenue",
                                                &format!("+${:.2}", total_income),
                                                Color::srgb(0.4, 0.9, 0.4),
                                                revenue_hint,
                                            );
                                            spawn_stat_row_with_hint(
                                                section,
                                                "Total Expenses",
                                                &format!("-${:.2}", total_expenses),
                                                Color::srgb(0.9, 0.4, 0.4),
                                                expense_hint,
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
                                                &format_delta(operating_profit),
                                                profit_color,
                                            );
                                            spawn_stat_row(
                                                section,
                                                "Reputation",
                                                &format!(
                                                    "{} ({})",
                                                    game_state.reputation,
                                                    format_int_delta(reputation_delta)
                                                ),
                                                rep_color,
                                            );
                                            spawn_stat_row(
                                                section,
                                                "Station Status",
                                                &format!("{}/{} Online", chargers_online, chargers_total),
                                                if chargers_online < chargers_total {
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
                                                    Text::new(&pro_tip_text),
                                                    TextFont {
                                                        font_size: 13.0,
                                                        ..default()
                                                    },
                                                    TextColor(Color::srgb(0.7, 0.8, 0.9)),
                                                ));
                                        });
                                }); // end kpi_row with_children

                            // ===== BADGE SECTION (conditional) =====
                            if let Some((badge, icon_handle)) = &top_badge_data {
                                // Divider
                                modal.spawn((
                                    Node {
                                        width: Val::Percent(100.0),
                                        height: Val::Px(1.0),
                                        margin: UiRect::vertical(Val::Px(4.0)),
                                        ..default()
                                    },
                                    BackgroundColor(Color::srgb(0.3, 0.35, 0.4)),
                                ));

                                // Badge header
                                modal
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
                                modal
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
                                        // Badge icon
                                        badge_row.spawn((
                                            ImageNode::new(icon_handle.clone()),
                                            Node {
                                                width: Val::Px(40.0),
                                                height: Val::Px(40.0),
                                                ..default()
                                            },
                                        ));

                                        // Badge name + tier
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
                                                    Text::new(format!(
                                                        "({})",
                                                        badge.tier().short_label()
                                                    )),
                                                    TextFont {
                                                        font_size: 12.0,
                                                        ..default()
                                                    },
                                                    TextColor(Color::srgb(0.5, 0.5, 0.5)),
                                                ));
                                            });
                                    });
                            }

                        }); // end inner content with_children(|modal|)
                    }); // end scroll_outer with_children

                        // ===== BUTTON FOOTER (pinned outside scroll — always visible) =====
                        modal_outer
                            .spawn((
                                Node {
                                    width: Val::Percent(100.0),
                                    flex_direction: FlexDirection::Row,
                                    justify_content: JustifyContent::Center,
                                    column_gap: Val::Px(12.0),
                                    padding: UiRect::new(
                                        Val::Px(16.0),
                                        Val::Px(16.0),
                                        Val::Px(12.0),
                                        Val::Px(12.0),
                                    ),
                                    border: UiRect::top(Val::Px(1.0)),
                                    ..default()
                                },
                                BorderColor::all(Color::srgb(0.3, 0.35, 0.4)),
                            ))
                            .with_children(|button_row| {
                                // Share on LinkedIn
                                button_row
                                    .spawn((
                                        Button,
                                        Node {
                                            padding: UiRect::new(
                                                Val::Px(16.0),
                                                Val::Px(16.0),
                                                Val::Px(10.0),
                                                Val::Px(10.0),
                                            ),
                                            justify_content: JustifyContent::Center,
                                            align_items: AlignItems::Center,
                                            border: UiRect::all(Val::Px(1.0)),
                                            ..default()
                                        },
                                        BackgroundColor(Color::srgb(0.04, 0.4, 0.76)),
                                        BorderColor::all(Color::srgb(0.06, 0.5, 0.86)),
                                        BorderRadius::all(Val::Px(4.0)),
                                        LinkedInShareButton,
                                    ))
                                    .with_child((
                                        Text::new("Share on LinkedIn"),
                                        TextFont {
                                            font_size: 14.0,
                                            ..default()
                                        },
                                        TextColor(Color::WHITE),
                                    ));

                                // Continue to Next Day
                                button_row
                                    .spawn((
                                        Button,
                                        Node {
                                            padding: UiRect::new(
                                                Val::Px(16.0),
                                                Val::Px(16.0),
                                                Val::Px(10.0),
                                                Val::Px(10.0),
                                            ),
                                            justify_content: JustifyContent::Center,
                                            align_items: AlignItems::Center,
                                            border: UiRect::all(Val::Px(2.0)),
                                            ..default()
                                        },
                                        BackgroundColor(Color::srgb(0.2, 0.6, 0.3)),
                                        BorderColor::all(Color::srgb(0.25, 0.7, 0.35)),
                                        BorderRadius::all(Val::Px(4.0)),
                                        DayEndContinueButton,
                                    ))
                                    .with_child((
                                        Text::new("Continue"),
                                        TextFont {
                                            font_size: 14.0,
                                            ..default()
                                        },
                                        TextColor(Color::WHITE),
                                    ));
                            }); // end button_row with_children
                    }); // end modal_outer with_children
        }); // end overlay with_children
}

/// Helper to spawn a stat row with label and value
fn spawn_stat_row(parent: &mut ChildSpawnerCommands, label: &str, value: &str, value_color: Color) {
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

/// Helper to spawn a prominent stat row (larger font, used for Net Profit)
fn spawn_prominent_stat_row(
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

/// Helper to spawn an indented stat row with label and value
fn spawn_indented_row(
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

/// Generate a dynamic day title and subtitle based on the day's performance.
///
/// Returns `(title, subtitle)` for display in the KPI collapsed header.
fn day_title(
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
    // Net loss cases
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
///
/// The tip is attributed to the character name for personality.
fn generate_pro_tip(
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

/// Helper to spawn a stat row with label, value, and an optional gray hint.
fn spawn_stat_row_with_hint(
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
            // Value + hint container
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

/// Helper to spawn an insight/flavor text line within an expanded section.
fn spawn_insight_row(parent: &mut ChildSpawnerCommands, text: &str) {
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

/// Helper to spawn a section header for the expanded KPI view.
fn spawn_section_header(parent: &mut ChildSpawnerCommands, label: &str, icon: &str, color: Color) {
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

/// Helper to spawn a thin colored separator line.
fn spawn_section_divider(parent: &mut ChildSpawnerCommands, color: Color) {
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

/// Generate an energy margin insight string based on whether the player is selling at a loss.
fn energy_margin_insight(charging_revenue: f32, energy_cost: f32) -> &'static str {
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

/// Generate an operations insight string based on what broke.
fn operations_insight(
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

/// Generate a reputation insight string explaining why rep changed.
fn reputation_insight(
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

/// Generate a unit economy verdict string explaining per-session economics.
fn unit_economy_verdict(revenue_per_session: f32, cost_per_session: f32) -> String {
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

/// Auto-submit score to leaderboard when entering day end state
fn auto_submit_score_on_day_end(
    mut commands: Commands,
    game_state: Res<crate::resources::GameState>,
    player_profile: Res<crate::resources::PlayerProfile>,
    supabase_config: Option<Res<crate::api::SupabaseConfig>>,
    mut leaderboard_data: ResMut<crate::resources::LeaderboardData>,
) {
    // Auto-submit score to leaderboard
    if let Some(config) = supabase_config.as_ref() {
        // Calculate cumulative score
        let cumulative_score = game_state.calculate_cumulative_score();

        info!(
            "Auto-submitting score: {} for player: {} (ID: {:?})",
            cumulative_score, player_profile.name, player_profile.player_id
        );

        // Spawn async task to submit score
        let config_clone = (**config).clone();
        let player_id = player_profile.player_id.clone();
        let player_name = player_profile.name.clone();

        let task = crate::ui::leaderboard_systems::spawn_network_task(async move {
            info!("Making API call to submit score: {}", cumulative_score);
            match crate::api::submit_score(
                &config_clone,
                player_id.as_deref(),
                &player_name,
                cumulative_score,
            )
            .await
            {
                Ok(response) => {
                    info!(
                        "Score submitted successfully: {} - {} (ID: {})",
                        response.player_name, response.score, response.id
                    );
                    Some(response.id)
                }
                Err(e) => {
                    error!("Failed to submit score: {}", e);
                    None
                }
            }
        });

        // Spawn entity with the task component
        commands.spawn(crate::ui::ScoreSubmitTask(task));
        info!("Score submission task spawned");

        // Force a leaderboard refresh by clearing the cache so the next
        // manual open of the leaderboard shows up-to-date results.
        leaderboard_data.last_fetched_at_secs = None;
    } else {
        info!("Supabase not configured, skipping score submission");
    }
}

/// Called when exiting the day end state
fn on_exit_day_end(
    mut commands: Commands,
    query: Query<Entity, With<DayEndUI>>,
    mut game_clock: ResMut<crate::resources::GameClock>,
    mut multi_site: ResMut<crate::resources::MultiSiteManager>,
    drivers: Query<Entity, With<crate::components::driver::Driver>>,
    ambient_vehicles: Query<Entity, With<crate::systems::ambient_traffic::AmbientVehicle>>,
    vehicle_sprites: Query<Entity, With<crate::systems::sprite::VehicleSprite>>,
    mut chargers: Query<&mut crate::components::charger::Charger>,
    mut game_state: ResMut<crate::resources::GameState>,
    mut build_state: ResMut<crate::resources::BuildState>,
    mut environment: ResMut<crate::resources::EnvironmentState>,
    mut carbon_market: ResMut<crate::resources::site_energy::CarbonCreditMarket>,
) {
    // Cleanup UI (despawn removes children automatically in Bevy 0.14+)
    for entity in &query {
        commands.entity(entity).try_despawn();
    }

    // Clean up share text resource
    commands.remove_resource::<DayEndShareText>();

    // Despawn all remaining drivers from previous day
    for entity in &drivers {
        commands.entity(entity).try_despawn();
    }

    // Despawn all ambient vehicles
    for entity in &ambient_vehicles {
        commands.entity(entity).try_despawn();
    }

    // Despawn all vehicle sprites
    for entity in &vehicle_sprites {
        commands.entity(entity).try_despawn();
    }

    // Reset charger charging state for new day (state is computed from is_charging/current_fault/is_disabled)
    for mut charger in &mut chargers {
        charger.is_charging = false;
        charger.current_power_kw = 0.0;
        charger.requested_power_kw = 0.0;
        charger.allocated_power_kw = 0.0;
        charger.session_start_game_time = None;
        charger.energy_delivered_kwh_today = 0.0;
    }

    // Reset per-site resources for the active site
    if let Some(site) = multi_site.active_site_mut() {
        site.charger_queue.clear();
        site.utility_meter.reset();
        site.grid_events.reset_daily();
        site.driver_schedule.next_driver_index = 0;
        site.driver_schedule.next_event_index = 0;
    }

    // Reset current day tracker for the new day
    game_state.daily_history.current_day = crate::resources::game_state::CurrentDayTracker {
        site_id: multi_site.viewed_site_id,
        starting_reputation: game_state.reputation,
        ..Default::default()
    };

    // Start new day - reset clock but keep the day counter
    game_clock.start_new_day();

    // Set ledger date for the new day
    if let Some(date) =
        chrono::NaiveDate::from_ymd_opt(game_clock.year as i32, game_clock.month, game_clock.day)
    {
        game_state.ledger.current_date = date;
    }

    // Reset environment timers for new day
    environment.reset_for_new_day();

    // Reset build state - day starts in "not started" mode
    // Time won't advance until they click "START DAY"
    build_state.is_open = false;

    // Fluctuate carbon credit market rate for the new day
    let mut rng = rand::rng();
    carbon_market.fluctuate(&mut rng);
    info!(
        "Starting Day {} - Carbon credit rate: ${:.2}/kWh",
        game_clock.day,
        carbon_market.rate_per_kwh()
    );
}

/// System to handle day end screen interactions
fn day_end_system(
    keyboard: Res<ButtonInput<KeyCode>>,
    mut next_state: ResMut<NextState<AppState>>,
    continue_query: Query<&Interaction, (Changed<Interaction>, With<DayEndContinueButton>)>,
) {
    // Handle continue button click
    for interaction in &continue_query {
        if *interaction == Interaction::Pressed {
            next_state.set(AppState::Playing);
        }
    }

    // Keyboard shortcuts to continue
    if keyboard.just_pressed(KeyCode::Space)
        || keyboard.just_pressed(KeyCode::Enter)
        || keyboard.just_pressed(KeyCode::Escape)
    {
        next_state.set(AppState::Playing);
    }
}

/// System to toggle between collapsed and expanded KPI views.
/// Swaps between the summary (collapsed) and detailed breakdown (expanded).
fn kpi_toggle_system(
    toggle_query: Query<&Interaction, (Changed<Interaction>, With<KpiToggleButton>)>,
    mut expanded_query: Query<
        &mut Node,
        (
            With<KpiExpanded>,
            Without<KpiCollapsed>,
            Without<DayEndModalContainer>,
        ),
    >,
    mut collapsed_query: Query<
        &mut Node,
        (
            With<KpiCollapsed>,
            Without<KpiExpanded>,
            Without<DayEndModalContainer>,
        ),
    >,
    mut modal_query: Query<
        &mut Node,
        (
            With<DayEndModalContainer>,
            Without<KpiExpanded>,
            Without<KpiCollapsed>,
        ),
    >,
    mut toggle_text: Query<&mut Text, With<KpiToggleButton>>,
) {
    for interaction in &toggle_query {
        if *interaction == Interaction::Pressed {
            // Toggle expanded visibility
            let mut now_showing_details = false;
            for mut node in &mut expanded_query {
                let is_currently_visible = node.display != Display::None;
                if is_currently_visible {
                    node.display = Display::None;
                } else {
                    node.display = Display::Flex;
                    now_showing_details = true;
                }
            }

            // Toggle collapsed visibility (hide when expanded, show when collapsed)
            for mut node in &mut collapsed_query {
                node.display = if now_showing_details {
                    Display::None
                } else {
                    Display::Flex
                };
            }

            // Toggle modal width
            for mut node in &mut modal_query {
                node.width = if now_showing_details {
                    Val::Px(700.0)
                } else {
                    Val::Px(480.0)
                };
            }

            // Update toggle button text
            for mut text in &mut toggle_text {
                **text = if now_showing_details {
                    "Collapse ^".to_string()
                } else {
                    "Expand v".to_string()
                };
            }
        }
    }
}

/// System to handle LinkedIn share button click
fn linkedin_share_system(
    share_query: Query<&Interaction, (Changed<Interaction>, With<LinkedInShareButton>)>,
    share_text: Option<Res<DayEndShareText>>,
) {
    for interaction in &share_query {
        if *interaction == Interaction::Pressed {
            let text = share_text
                .as_ref()
                .map(|s| s.0.as_str())
                .unwrap_or("Playing Kilowatt Tycoon! #KilowattTycoon #EVCharging");
            open_linkedin_share(text);
        }
    }
}

/// Open a LinkedIn share URL in the browser (cross-platform)
fn open_linkedin_share(text: &str) {
    let encoded_text = url_encode(text);
    let url = format!("https://www.linkedin.com/feed/?shareActive=true&text={encoded_text}");

    #[cfg(target_arch = "wasm32")]
    {
        if let Some(window) = web_sys::window() {
            let _ = window.open_with_url_and_target(&url, "_blank");
        }
    }

    #[cfg(not(target_arch = "wasm32"))]
    {
        if let Err(e) = webbrowser::open(&url) {
            bevy::log::error!("Failed to open LinkedIn share URL: {e}");
        }
    }
}

/// Simple URL-encode for share text (handles the common characters)
fn url_encode(s: &str) -> String {
    let mut result = String::with_capacity(s.len() * 3);
    for byte in s.bytes() {
        match byte {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                result.push(byte as char);
            }
            b' ' => result.push_str("%20"),
            _ => {
                result.push_str(&format!("%{byte:02X}"));
            }
        }
    }
    result
}

/// Handles mouse-wheel scrolling for any UI node with `Overflow::scroll_y()` and a `ScrollPosition`.
/// Runs globally in all states so modals, day-end summaries, etc. all scroll.
const SCROLL_LINE_HEIGHT: f32 = 21.0;

fn ui_scroll_system(
    mut mouse_wheel: MessageReader<MouseWheel>,
    mut scrollable: Query<(&mut ScrollPosition, &Node, &ComputedNode)>,
) {
    let mut total_dy = 0.0_f32;
    for ev in mouse_wheel.read() {
        let dy = match ev.unit {
            MouseScrollUnit::Line => -ev.y * SCROLL_LINE_HEIGHT,
            MouseScrollUnit::Pixel => -ev.y,
        };
        total_dy += dy;
    }

    if total_dy == 0.0 {
        return;
    }

    for (mut scroll_pos, node, computed) in scrollable.iter_mut() {
        if node.overflow.y != OverflowAxis::Scroll {
            continue;
        }
        let content_h = computed.content_size().y;
        let visible_h = computed.size().y;
        let scale = computed.inverse_scale_factor();
        let max_scroll = ((content_h - visible_h) * scale).max(0.0);
        scroll_pos.y = (scroll_pos.y + total_dy).clamp(0.0, max_scroll);
    }
}

/// Tracks pointer-drag scroll gestures over UI scroll containers.
///
/// Works with both mouse drag (desktop) and single-finger touch (tablet)
/// because it reads from the unified `GamePointer` abstraction.
#[derive(Resource, Default)]
pub struct TouchScrollState {
    prev_y: Option<f32>,
    active_entity: Option<Entity>,
    cumulative_drag: f32,
    pub scrolling: bool,
}

const TOUCH_SCROLL_DEAD_ZONE: f32 = 8.0;

/// Converts pointer drags (mouse or touch) into `ScrollPosition` updates for
/// any visible container with `Overflow::scroll_y()`.
///
/// `GamePointer.screen_position` is in logical (CSS) pixels.
/// `UiGlobalTransform.translation` and `ComputedNode::size()` are in physical
/// pixels. We divide both by the window DPR so the hit-test and delta operate
/// in the same CSS-pixel space as the pointer.
fn ui_touch_scroll_system(
    mut pointer: ResMut<crate::helpers::pointer::GamePointer>,
    mut state: ResMut<TouchScrollState>,
    windows: Query<&Window, With<bevy::window::PrimaryWindow>>,
    mut scrollable: Query<(
        Entity,
        &mut ScrollPosition,
        &Node,
        &ComputedNode,
        &bevy::ui::UiGlobalTransform,
    )>,
) {
    // Pointer released or inactive -- clear state.
    if !pointer.pressed && !pointer.just_pressed {
        if state.scrolling {
            pointer.just_released = false;
        }
        *state = TouchScrollState::default();
        return;
    }

    let Some(pos) = pointer.screen_position else {
        *state = TouchScrollState::default();
        return;
    };

    let dpr = windows
        .single()
        .map(|w: &Window| w.scale_factor())
        .unwrap_or(1.0);

    if pointer.just_pressed {
        // Pointer just went down -- hit-test against scrollable containers.
        // Convert UI rects from physical pixels to CSS pixels so they match
        // the pointer coordinate space.
        let mut best: Option<(Entity, f32)> = None;
        for (entity, _scroll, node, computed, ugt) in scrollable.iter() {
            if node.overflow.y != OverflowAxis::Scroll {
                continue;
            }
            let size = computed.size() / dpr;
            let center = ugt.translation / dpr;
            let half = size * 0.5;
            let min = center - half;
            let max = center + half;

            if pos.x >= min.x && pos.x <= max.x && pos.y >= min.y && pos.y <= max.y {
                let area = size.x * size.y;
                if best.is_none_or(|(_, a)| area < a) {
                    best = Some((entity, area));
                }
            }
        }

        *state = TouchScrollState {
            prev_y: Some(pos.y),
            active_entity: best.map(|(e, _)| e),
            cumulative_drag: 0.0,
            scrolling: false,
        };
        return;
    }

    // While actively scrolling, suppress pointer so buttons don't activate.
    if state.scrolling {
        pointer.just_pressed = false;
        pointer.just_released = false;
    }

    // Pointer held -- compute delta and potentially scroll.
    let Some(prev_y) = state.prev_y else {
        return;
    };
    let Some(target) = state.active_entity else {
        state.prev_y = Some(pos.y);
        return;
    };

    let raw_dy = pos.y - prev_y;
    state.cumulative_drag += raw_dy;
    state.prev_y = Some(pos.y);

    if !state.scrolling && state.cumulative_drag.abs() < TOUCH_SCROLL_DEAD_ZONE {
        return;
    }
    state.scrolling = true;

    // Negate: finger/mouse moving up (negative dy) scrolls content down (positive offset).
    // raw_dy is in CSS pixels; convert to Val::Px (the unit ScrollPosition uses)
    // by multiplying by dpr * inverse_scale_factor (CSS -> physical -> Val::Px).
    if let Ok((_entity, mut scroll_pos, _node, computed, _ugt)) = scrollable.get_mut(target) {
        let isf = computed.inverse_scale_factor();
        let scroll_dy = -raw_dy * dpr * isf;

        let content_h = computed.content_size().y;
        let visible_h = computed.size().y;
        let max_scroll = ((content_h - visible_h) * isf).max(0.0);
        scroll_pos.y = (scroll_pos.y + scroll_dy).clamp(0.0, max_scroll);
    }
}

/// System to handle game over screen
fn game_over_system(
    keyboard: Res<ButtonInput<KeyCode>>,
    mut next_state: ResMut<NextState<AppState>>,
    mut interaction_query: Query<
        (&Interaction, &GameOverButton),
        (Changed<Interaction>, With<Button>),
    >,
) {
    // Handle button clicks
    for (interaction, button) in &mut interaction_query {
        if *interaction == Interaction::Pressed {
            match button {
                GameOverButton::PlayAgain => next_state.set(AppState::Loading),
                GameOverButton::MainMenu => next_state.set(AppState::Loading),
            }
        }
    }

    // Space to play again, Escape for main menu
    if keyboard.just_pressed(KeyCode::Space) {
        next_state.set(AppState::Loading);
    }
    if keyboard.just_pressed(KeyCode::Escape) {
        next_state.set(AppState::Loading);
    }
}

/// Types of buttons in the game over screen
#[derive(Component, Debug, Clone, Copy)]
pub enum GameOverButton {
    PlayAgain,
    MainMenu,
}

/// Setup game over UI
fn setup_game_over(mut commands: Commands, game_state: Res<crate::resources::GameState>) {
    let (title, color) = match game_state.result {
        crate::resources::GameResult::Won => ("YOU WIN!", Color::srgb(0.2, 0.8, 0.2)),
        crate::resources::GameResult::LostBankruptcy => ("BANKRUPT!", Color::srgb(0.8, 0.2, 0.2)),
        crate::resources::GameResult::LostReputation => {
            ("REPUTATION LOST!", Color::srgb(0.8, 0.4, 0.2))
        }
        crate::resources::GameResult::LostTimeout => ("TIME'S UP!", Color::srgb(0.8, 0.6, 0.2)),
        crate::resources::GameResult::InProgress => ("GAME OVER", Color::WHITE),
    };

    commands
        .spawn((
            GameOverUI,
            Node {
                width: Val::Percent(100.0),
                height: Val::Percent(100.0),
                position_type: PositionType::Absolute,
                justify_content: JustifyContent::Center,
                align_items: AlignItems::Center,
                flex_direction: FlexDirection::Column,
                row_gap: Val::Px(20.0),
                ..default()
            },
            BackgroundColor(Color::srgba(0.0, 0.0, 0.0, 0.85)),
            GlobalZIndex(1000),
        ))
        .with_children(|parent| {
            // Title
            parent.spawn((
                Text::new(title),
                TextFont {
                    font_size: 64.0,
                    ..default()
                },
                TextColor(color),
            ));

            // Stats -- derive cable theft total from ledger (debit-normal account, positive balance)
            let total_cable_theft: f32 = {
                use rust_decimal::prelude::ToPrimitive;
                game_state
                    .ledger
                    .account_balance(crate::resources::ledger::Account::CableTheft)
                    .to_f32()
                    .unwrap_or(0.0)
            };

            parent.spawn((
                Text::new(format!(
                    "Revenue: ${:.0}\nReputation: {}\nSessions: {}",
                    game_state.ledger.net_revenue_f32(),
                    game_state.reputation,
                    game_state.sessions_completed
                )),
                TextFont {
                    font_size: 24.0,
                    ..default()
                },
                TextColor(Color::WHITE),
            ));

            // Cable theft total (prominent if any)
            if total_cable_theft > 0.01 {
                parent.spawn((
                    Text::new(format!(
                        "🔌 Total Cable Theft Losses: -${:.0}",
                        total_cable_theft
                    )),
                    TextFont {
                        font_size: 22.0,
                        ..default()
                    },
                    TextColor(Color::srgb(1.0, 0.2, 0.2)),
                ));
            }

            // Play again button
            parent
                .spawn((
                    Button,
                    Node {
                        width: Val::Px(200.0),
                        height: Val::Px(50.0),
                        justify_content: JustifyContent::Center,
                        align_items: AlignItems::Center,
                        ..default()
                    },
                    BackgroundColor(Color::srgb(0.2, 0.6, 0.2)),
                    GameOverButton::PlayAgain,
                ))
                .with_children(|btn| {
                    btn.spawn((
                        Text::new("Play Again (Space)"),
                        TextFont {
                            font_size: 20.0,
                            ..default()
                        },
                        TextColor(Color::WHITE),
                    ));
                });

            // Main menu button
            parent
                .spawn((
                    Button,
                    Node {
                        width: Val::Px(200.0),
                        height: Val::Px(50.0),
                        justify_content: JustifyContent::Center,
                        align_items: AlignItems::Center,
                        ..default()
                    },
                    BackgroundColor(Color::srgb(0.4, 0.4, 0.4)),
                    GameOverButton::MainMenu,
                ))
                .with_children(|btn| {
                    btn.spawn((
                        Text::new("Main Menu (Esc)"),
                        TextFont {
                            font_size: 20.0,
                            ..default()
                        },
                        TextColor(Color::WHITE),
                    ));
                });
        });
}

/// Cleanup game over UI
fn cleanup_game_over(mut commands: Commands, query: Query<Entity, With<GameOverUI>>) {
    for entity in &query {
        commands.entity(entity).try_despawn();
    }
}
