//! Driver emotion evaluation system

use crate::components::charger::Charger;
use crate::components::driver::{Driver, DriverMood, DriverState};
use crate::components::emotion::{DriverEmotion, EmotionMood, EmotionReason};
use crate::components::site::BelongsToSite;
use crate::resources::{EnvironmentState, GameClock, MultiSiteManager};
use bevy::prelude::*;

/// Add DriverEmotion component to newly spawned drivers
pub fn init_driver_emotions(
    mut commands: Commands,
    game_clock: Res<GameClock>,
    drivers: Query<(Entity, &Driver), (Added<Driver>, Without<DriverEmotion>)>,
) {
    for (entity, driver) in &drivers {
        let reason = EmotionReason::JustArrived;
        let variations = reason.speech_variations();
        let speech_text = if !variations.is_empty() {
            let idx = (rand::random::<f32>() * variations.len() as f32) as usize % variations.len();
            Some(variations[idx])
        } else {
            None
        };

        commands.entity(entity).try_insert(DriverEmotion {
            mood: EmotionMood::Neutral,
            reason,
            set_at: game_clock.total_real_time,
            duration: reason.display_duration(),
            speech_text,
            last_driver_state: Some(driver.state),
            last_frustration_reason: None,
        });
    }
}

/// Continuously evaluate and update driver emotions
/// Reads per-site ServiceStrategy so drivers react to the player's actual price
pub fn evaluate_driver_emotions(
    mut drivers: Query<(&mut Driver, &mut DriverEmotion, &BelongsToSite)>,
    chargers: Query<&Charger>,
    multi_site: Res<MultiSiteManager>,
    environment: Res<EnvironmentState>,
    game_clock: Res<GameClock>,
) {
    if game_clock.is_paused() {
        return;
    }

    for (mut driver, mut emotion, belongs) in &mut drivers {
        let switched = driver.just_switched_charger;
        let state_changed = emotion.last_driver_state != Some(driver.state);
        let emotion_expired = emotion.is_expired(game_clock.total_real_time);

        if !emotion_expired && !state_changed && !switched {
            continue;
        }

        // Clear the one-shot flag now that we've observed it
        if switched {
            driver.just_switched_charger = false;
        }

        let Some(site_state) = multi_site.get_site(belongs.site_id) else {
            continue;
        };

        let effective_price = site_state.service_strategy.pricing.effective_price(
            game_clock.game_time,
            &site_state.site_energy_config,
            site_state.charger_utilization,
        );
        let (new_mood, new_reason) =
            evaluate_emotion_for_state(&driver, &chargers, effective_price, &environment, switched);

        let would_change = emotion.mood != new_mood || emotion.reason != new_reason;

        if !would_change {
            if state_changed {
                emotion.last_driver_state = Some(driver.state);
            }
            continue;
        }

        emotion.set_emotion(new_mood, new_reason, game_clock.total_real_time);
        emotion.last_driver_state = Some(driver.state);
    }
}

/// Helper to determine what emotion a driver should have based on their state.
///
/// `just_switched` is true when the driver just moved to an alternative charger
/// and should show a `SwitchedCharger` bubble before the normal charging emotion.
fn evaluate_emotion_for_state(
    driver: &Driver,
    chargers: &Query<&Charger>,
    effective_price: f32,
    environment: &EnvironmentState,
    just_switched: bool,
) -> (EmotionMood, EmotionReason) {
    // One-shot: driver just switched to an alternative charger
    if just_switched && driver.state == DriverState::Charging {
        return (EmotionMood::Happy, EmotionReason::SwitchedCharger);
    }

    match driver.state {
        DriverState::Arriving => {
            // Check if they're assigned vs waiting
            if driver.assigned_charger.is_some() {
                let price = effective_price;

                // Adjust thresholds based on weather
                // In extreme weather, drivers are more sensitive to price
                let (great_threshold, fair_threshold) = match environment.current_weather {
                    crate::resources::WeatherType::Heatwave => (0.25, 0.35), // More price sensitive
                    crate::resources::WeatherType::Cold => (0.28, 0.40),
                    _ => (0.30, 0.45), // Normal thresholds
                };

                if price <= great_threshold {
                    (EmotionMood::Happy, EmotionReason::PriceGreat)
                } else if price <= fair_threshold {
                    (EmotionMood::Neutral, EmotionReason::PriceFair)
                } else {
                    (
                        EmotionMood::Frustrated,
                        EmotionReason::FrustrationTooExpensive,
                    )
                }
            } else {
                // No charger available - use short frustration text
                (EmotionMood::Frustrated, EmotionReason::FrustrationBusy)
            }
        }
        DriverState::WaitingForCharger | DriverState::Queued => {
            // Update based on patience - use short frustration texts
            let patience_pct = driver.patience / driver.patience_level.initial_patience();
            if patience_pct < 0.25 {
                (EmotionMood::Angry, EmotionReason::FrustrationBusy)
            } else if patience_pct < 0.50 {
                (EmotionMood::Frustrated, EmotionReason::FrustrationBusy)
            } else {
                // Still have patience, use mild waiting message
                (EmotionMood::Neutral, EmotionReason::MustWait)
            }
        }
        DriverState::Frustrated => {
            // Driver is frustrated - determine reason from charger state
            let charger_broken = driver
                .assigned_charger
                .and_then(|e| chargers.get(e).ok())
                .is_some_and(|c| c.current_fault.is_some());

            if charger_broken {
                // Charger is broken - use "didn't work" text
                (EmotionMood::Angry, EmotionReason::FrustrationDidntWork)
            } else {
                // Other frustration (technician working, etc.) - use busy text
                (EmotionMood::Frustrated, EmotionReason::FrustrationBusy)
            }
        }
        DriverState::Charging => {
            // Zero power takes priority — driver can see 0 kW on their dashboard (Rule 2)
            if driver.zero_power_seconds > 0.0 {
                return if driver.zero_power_seconds > 60.0 {
                    (EmotionMood::Angry, EmotionReason::FrustrationNoPower)
                } else {
                    (EmotionMood::Frustrated, EmotionReason::NoPower)
                };
            }

            let progress = driver.charge_progress();

            if progress < 0.1 {
                (EmotionMood::Happy, EmotionReason::ChargingStarted)
            } else if progress > 0.9 {
                (EmotionMood::VeryHappy, EmotionReason::ChargingAlmostDone)
            } else if let Some(charger_entity) = driver.assigned_charger {
                if let Ok(charger) = chargers.get(charger_entity) {
                    let power_ratio = if charger.requested_power_kw > 0.0 {
                        charger.allocated_power_kw / charger.requested_power_kw
                    } else {
                        1.0
                    };

                    if power_ratio < 0.5 {
                        return (EmotionMood::Frustrated, EmotionReason::WaitingTooLong);
                    }
                }
                (EmotionMood::Happy, EmotionReason::ChargingStarted)
            } else {
                (EmotionMood::Happy, EmotionReason::ChargingStarted)
            }
        }
        DriverState::Complete => (EmotionMood::VeryHappy, EmotionReason::ChargingComplete),
        DriverState::Leaving => {
            if driver.charge_received_kwh > 0.0 {
                (EmotionMood::VeryHappy, EmotionReason::ChargingComplete)
            } else {
                (EmotionMood::Neutral, EmotionReason::MustWait)
            }
        }
        DriverState::LeftAngry => (EmotionMood::Angry, EmotionReason::LeavingAngry),
    }
}

/// Update driver mood component to match emotion
pub fn sync_mood_with_emotion(
    mut drivers: Query<(&mut Driver, &DriverEmotion), Changed<DriverEmotion>>,
) {
    for (mut driver, emotion) in &mut drivers {
        let new_mood = match emotion.mood {
            EmotionMood::VeryHappy | EmotionMood::Happy => DriverMood::Happy,
            EmotionMood::Neutral => DriverMood::Neutral,
            EmotionMood::Skeptical | EmotionMood::Frustrated => DriverMood::Impatient,
            EmotionMood::Angry => DriverMood::Angry,
        };
        // Only update if mood actually changed to avoid triggering unnecessary change detection
        if driver.mood != new_mood {
            driver.mood = new_mood;
        }
    }
}
