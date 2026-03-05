use bevy::prelude::*;

use crate::states::day_end::report::DayEndReport;
use crate::states::day_end::{
    DayEndModalContainer, DayEndShareText, KpiCollapsed, KpiExpanded, LinkedInShareButton,
};
use crate::states::{AppState, DayEndContinueButton, DayEndUI, KpiToggleButton};

/// Handle continue button click and keyboard shortcuts to advance to next day.
pub(crate) fn day_end_system(
    keyboard: Res<ButtonInput<KeyCode>>,
    mut next_state: ResMut<NextState<AppState>>,
    continue_query: Query<&Interaction, (Changed<Interaction>, With<DayEndContinueButton>)>,
) {
    for interaction in &continue_query {
        if *interaction == Interaction::Pressed {
            next_state.set(AppState::Playing);
        }
    }

    if keyboard.just_pressed(KeyCode::Space)
        || keyboard.just_pressed(KeyCode::Enter)
        || keyboard.just_pressed(KeyCode::Escape)
    {
        next_state.set(AppState::Playing);
    }
}

/// Toggle between collapsed and expanded KPI views.
pub(crate) fn kpi_toggle_system(
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

            for mut node in &mut collapsed_query {
                node.display = if now_showing_details {
                    Display::None
                } else {
                    Display::Flex
                };
            }

            for mut node in &mut modal_query {
                node.width = if now_showing_details {
                    Val::Px(700.0)
                } else {
                    Val::Px(480.0)
                };
            }

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

/// Handle LinkedIn share button click.
pub(crate) fn linkedin_share_system(
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

/// Auto-submit score to leaderboard when entering day end state.
pub(crate) fn auto_submit_score_on_day_end(
    mut commands: Commands,
    game_state: Res<crate::resources::GameState>,
    player_profile: Res<crate::resources::PlayerProfile>,
    supabase_config: Option<Res<crate::api::SupabaseConfig>>,
    mut leaderboard_data: ResMut<crate::resources::LeaderboardData>,
) {
    if let Some(config) = supabase_config.as_ref() {
        let cumulative_score = game_state.calculate_cumulative_score();

        info!(
            "Auto-submitting score: {} for player: {} (ID: {:?})",
            cumulative_score, player_profile.name, player_profile.player_id
        );

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

        commands.spawn(crate::ui::ScoreSubmitTask(task));
        info!("Score submission task spawned");

        leaderboard_data.last_fetched_at_secs = None;
    } else {
        info!("Supabase not configured, skipping score submission");
    }
}

/// Cleanup and transition logic when exiting the day end state.
pub(crate) fn on_exit_day_end(
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
    for entity in &query {
        commands.entity(entity).try_despawn();
    }

    commands.remove_resource::<DayEndShareText>();
    commands.remove_resource::<DayEndReport>();

    for entity in &drivers {
        commands.entity(entity).try_despawn();
    }

    for entity in &ambient_vehicles {
        commands.entity(entity).try_despawn();
    }

    for entity in &vehicle_sprites {
        commands.entity(entity).try_despawn();
    }

    for mut charger in &mut chargers {
        charger.is_charging = false;
        charger.current_power_kw = 0.0;
        charger.requested_power_kw = 0.0;
        charger.allocated_power_kw = 0.0;
        charger.session_start_game_time = None;
        charger.energy_delivered_kwh_today = 0.0;
    }

    if let Some(site) = multi_site.active_site_mut() {
        site.charger_queue.clear();
        site.utility_meter.reset();
        site.grid_events.reset_daily();
        site.driver_schedule.next_driver_index = 0;
        site.driver_schedule.next_event_index = 0;
    }

    game_state.daily_history.current_day = crate::resources::game_state::CurrentDayTracker {
        site_id: multi_site.viewed_site_id,
        starting_reputation: game_state.reputation,
        ..Default::default()
    };

    game_clock.start_new_day();

    if let Some(date) =
        chrono::NaiveDate::from_ymd_opt(game_clock.year as i32, game_clock.month, game_clock.day)
    {
        game_state.ledger.current_date = date;
    }

    environment.reset_for_new_day();

    build_state.is_open = false;

    let mut rng = rand::rng();
    carbon_market.fluctuate(&mut rng);
    info!(
        "Starting Day {} - Carbon credit rate: ${:.2}/kWh",
        game_clock.day,
        carbon_market.rate_per_kwh()
    );
}
