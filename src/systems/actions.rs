//! Remote action system
//!
//! This module demonstrates error handling patterns in game systems:
//! - Using `Result` types for operations that can fail
//! - Graceful handling of missing entities
//! - Event-driven error reporting

use bevy::prelude::*;
use rand::Rng;

use crate::components::charger::{Charger, FaultType, RemoteAction};
use crate::errors::{ChargeOpsError, ChargeOpsResult};
use crate::events::{ChargerFaultResolvedEvent, RemoteActionRequestEvent, RemoteActionResultEvent};

/// Result of executing a remote action
#[derive(Debug, Clone)]
pub struct ActionExecutionResult {
    pub success: bool,
    pub fault_resolved: bool,
    pub message: String,
}

/// Try to execute a remote action on a charger.
///
/// This is a fallible helper function that can be used for testing
/// or for more granular error handling.
pub fn try_execute_action(
    charger: &mut Charger,
    action: RemoteAction,
    success_roll: f32,
) -> ChargeOpsResult<ActionExecutionResult> {
    // Validate action can be performed
    if charger.is_on_cooldown(action) {
        return Err(ChargeOpsError::ActionFailed {
            action: format!("{action:?}"),
            reason: "Action is on cooldown".to_string(),
        });
    }

    let is_reboot = matches!(action, RemoteAction::SoftReboot | RemoteAction::HardReboot);

    // Reboots always succeed; other actions use a probability roll
    let effective_rate = charger.effective_action_success(action);
    let success = is_reboot || success_roll < effective_rate;

    // Start cooldown regardless of success
    charger.start_cooldown(action);

    if !success {
        return Ok(ActionExecutionResult {
            success: false,
            fault_resolved: false,
            message: format!(
                "{:?} failed (rolled {:.0}%, needed < {:.0}%)",
                action,
                success_roll * 100.0,
                effective_rate * 100.0
            ),
        });
    }

    // Execute the action
    let (fault_resolved, message) = match action {
        RemoteAction::SoftReboot | RemoteAction::HardReboot => execute_reboot(charger, action),
        RemoteAction::ReleaseConnector => execute_release_connector(charger),
        RemoteAction::Disable => execute_disable(charger),
        RemoteAction::Enable => execute_enable(charger),
    };

    Ok(ActionExecutionResult {
        success: true,
        fault_resolved,
        message,
    })
}

fn execute_reboot(charger: &mut Charger, action: RemoteAction) -> (bool, String) {
    if let Some(fault) = charger.current_fault {
        match fault {
            FaultType::CommunicationError | FaultType::FirmwareFault | FaultType::PaymentError => {
                charger.current_fault = None;
                charger.fault_discovered = false;
                charger.reboot_attempts = 0;
                (true, format!("{action:?} succeeded, fault cleared"))
            }
            FaultType::CableDamage | FaultType::GroundFault | FaultType::CableTheft => (
                false,
                format!("{action:?} succeeded but fault {fault:?} requires technician"),
            ),
        }
    } else {
        (false, format!("{action:?} succeeded, no fault to clear"))
    }
}

fn execute_release_connector(charger: &mut Charger) -> (bool, String) {
    if charger.current_fault == Some(FaultType::CableDamage) {
        // Clearing current_fault causes state() to return Available
        charger.current_fault = None;
        charger.fault_discovered = false;
        (true, "Connector released successfully".to_string())
    } else {
        (false, "No cable damage to release".to_string())
    }
}

fn execute_disable(charger: &mut Charger) -> (bool, String) {
    // Setting is_disabled causes state() to return Disabled
    charger.is_disabled = true;
    charger.is_charging = false;
    charger.current_power_kw = 0.0;
    (false, format!("Charger {} disabled", charger.id))
}

fn execute_enable(charger: &mut Charger) -> (bool, String) {
    // Clearing is_disabled causes state() to compute from current_fault/is_charging
    charger.is_disabled = false;
    (false, format!("Charger {} enabled", charger.id))
}

/// Process remote action requests (always available, not gated on O&M tier)
pub fn action_system(
    mut action_events: MessageReader<RemoteActionRequestEvent>,
    mut chargers: Query<(&mut Charger, &crate::components::BelongsToSite)>,
    mut result_events: MessageWriter<RemoteActionResultEvent>,
    mut resolved_events: MessageWriter<ChargerFaultResolvedEvent>,
    game_clock: Res<crate::resources::GameClock>,
    multi_site: Res<crate::resources::MultiSiteManager>,
    mut game_state: ResMut<crate::resources::GameState>,
) {
    let mut rng = rand::rng();

    for event in action_events.read() {
        // Try to get the charger - log warning and continue if not found
        let Ok((mut charger, belongs)) = chargers.get_mut(event.charger_entity) else {
            warn!(
                "Action {:?} requested for non-existent entity {:?}",
                event.action, event.charger_entity
            );
            continue;
        };

        let charger_id = charger.id.clone();
        let success_roll = rng.random::<f32>();

        // Capture fault timestamps before action (may clear them)
        let fault_occurred_at = charger.fault_occurred_at;

        // Execute the action using the fallible helper
        match try_execute_action(&mut charger, event.action, success_roll) {
            Ok(execution_result) => {
                info!("{}: {}", charger_id, execution_result.message);

                // Send fault resolved event if applicable
                if execution_result.fault_resolved {
                    let occurred_at = fault_occurred_at.unwrap_or(game_clock.total_game_time);
                    let resolved_at = game_clock.total_game_time;

                    // Recover reliability for fast remote fix
                    let downtime = resolved_at - occurred_at;
                    let oem_recovery = multi_site
                        .get_site(belongs.site_id)
                        .map(|s| s.site_upgrades.oem_tier.reliability_recovery_multiplier())
                        .unwrap_or(1.0);
                    charger.recover_reliability_fast_fix(downtime, oem_recovery);

                    // Successful fault resolution earns a small reputation bonus
                    game_state.change_reputation(1);

                    // Clear fault timestamps
                    charger.fault_occurred_at = None;
                    charger.fault_detected_at = None;
                    charger.fault_is_detected = true;

                    resolved_events.write(ChargerFaultResolvedEvent {
                        charger_entity: event.charger_entity,
                        charger_id: charger_id.clone(),
                    });
                }

                // Send result event
                result_events.write(RemoteActionResultEvent {
                    charger_entity: event.charger_entity,
                    charger_id,
                    action: event.action,
                    success: execution_result.success,
                });
            }
            Err(e) => {
                // Action couldn't be executed (e.g., on cooldown)
                info!("Action {:?} on {}: {}", event.action, charger_id, e);

                // Still send a failure result event
                result_events.write(RemoteActionResultEvent {
                    charger_entity: event.charger_entity,
                    charger_id,
                    action: event.action,
                    success: false,
                });
            }
        }
    }
}
