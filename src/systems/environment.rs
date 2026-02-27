//! Environment system - weather and news event cycles

use crate::resources::{EnvironmentState, GameClock};
use bevy::prelude::*;

/// System to update weather and news events periodically
pub fn environment_system(mut environment: ResMut<EnvironmentState>, game_clock: Res<GameClock>) {
    if game_clock.is_paused() {
        return;
    }

    let game_time = game_clock.game_time;

    // Track if changes occurred for logging
    let old_weather = environment.current_weather;
    let old_news = environment.active_news.clone();

    // Roll for weather changes
    environment.roll_weather_change(game_time);

    // Roll for news events
    environment.roll_news_event(game_time);

    // Log only when actual changes occur
    if old_weather != environment.current_weather {
        info!(
            "Weather changed: {} at {:.0}°F | Demand: {:.0}%",
            environment.current_weather.display_name(),
            environment.temperature_f,
            environment.total_demand_multiplier() * 100.0
        );
    }

    if old_news != environment.active_news {
        if let Some(ref news) = environment.active_news {
            info!("📰 Breaking News: {}", news);
        } else {
            info!("📰 News cycle cleared");
        }
    }
}

/// System to log environment changes (debug/info)
/// This system is intentionally removed to avoid log spam.
/// Weather/news changes are logged directly in environment_system when they occur.
#[allow(dead_code)]
pub fn log_environment_changes(_environment: Res<EnvironmentState>) {
    // Intentionally empty - logging is done in environment_system
}
