//! Tests for charger-related systems.
//!
//! These tests verify the behavior of charger state management,
//! fault handling, and remote actions.

mod test_utils;

use bevy::ecs::message::MessageWriter;
use bevy::prelude::*;

use kilowatt_tycoon::components::charger::{
    Charger, ChargerState, ChargerType, FaultType, RemoteAction,
};
use kilowatt_tycoon::components::driver::{Driver, DriverState, MovementPhase, VehicleMovement};
use kilowatt_tycoon::components::site::BelongsToSite;
use kilowatt_tycoon::components::{
    Technician, TechnicianEmotion, TechnicianMovement, TechnicianPhase,
};
use kilowatt_tycoon::events::{RepairCompleteEvent, RepairFailedEvent, TechnicianDispatchEvent};
use kilowatt_tycoon::hooks::ChargerIndex;
use kilowatt_tycoon::resources::{
    GameClock, GameSpeed, MultiSiteManager, OemTier, QueuedDispatch, RepairRequestRegistry,
    RepairRequestStatus, RepairResolution, SiteArchetype, SiteId, TechStatus, TechnicianState,
};
use kilowatt_tycoon::systems::{
    cleanup_sold_charger_references, dispatch_technician_system, driver_arrival_system,
    fault_detection_system, om_auto_dispatch_system, reconcile_repair_requests_system,
    recover_dispatchable_requests_system, recover_orphaned_leaving_technician_system,
    scripted_fault_system, technician_repair_system, technician_travel_system, try_execute_action,
};

use test_utils::*;

#[derive(Resource, Default)]
struct PendingDispatchEvent(Option<TechnicianDispatchEvent>);

#[derive(Resource, Default)]
struct PendingDispatchEvents(Vec<TechnicianDispatchEvent>);

fn emit_pending_dispatch(
    mut pending: ResMut<PendingDispatchEvent>,
    mut writer: MessageWriter<TechnicianDispatchEvent>,
) {
    if let Some(event) = pending.0.take() {
        writer.write(event);
    }
}

fn emit_pending_dispatches(
    mut pending: ResMut<PendingDispatchEvents>,
    mut writer: MessageWriter<TechnicianDispatchEvent>,
) {
    for event in pending.0.drain(..) {
        writer.write(event);
    }
}

#[test]
fn test_charger_starts_available() {
    let mut app = create_test_app();

    let charger = create_test_charger("CHG-001", ChargerType::DcFast);
    let entity = spawn_charger(&mut app, charger);

    assert!(charger_has_state(&app, entity, ChargerState::Available));
    assert!(charger_has_fault(&app, entity, None));
}

#[test]
fn test_charger_cooldown_directly() {
    // Test the Charger::update_cooldowns method directly without a system
    let mut charger = create_test_charger("CHG-001", ChargerType::DcFast);
    charger.cooldowns.insert(RemoteAction::SoftReboot, 10.0);

    // Initially on cooldown
    assert!(charger.is_on_cooldown(RemoteAction::SoftReboot));

    // Update cooldowns by 5 seconds
    charger.update_cooldowns(5.0);

    // Should still be on cooldown but reduced
    let cooldown = charger
        .cooldowns
        .get(&RemoteAction::SoftReboot)
        .copied()
        .unwrap_or(0.0);
    assert!((cooldown - 5.0).abs() < 0.01);
    assert!(charger.is_on_cooldown(RemoteAction::SoftReboot));

    // Update by another 10 seconds to clear
    charger.update_cooldowns(10.0);
    assert!(!charger.is_on_cooldown(RemoteAction::SoftReboot));
}

#[test]
fn test_game_clock_pause_behavior() {
    // Test that GameClock properly handles pause state
    use kilowatt_tycoon::resources::GameClock;

    let mut clock = GameClock::default();

    // Normal speed
    clock.set_speed(GameSpeed::Normal);
    assert!(!clock.is_paused());

    // Pause
    clock.set_speed(GameSpeed::Paused);
    assert!(clock.is_paused());

    // When paused, tick should not advance time
    let initial_game_time = clock.game_time;
    clock.tick(1.0);
    assert_eq!(clock.game_time, initial_game_time);

    // Resume
    clock.set_speed(GameSpeed::Fast);
    assert!(!clock.is_paused());
    clock.tick(1.0);
    assert!(clock.game_time > initial_game_time);
}

#[test]
fn test_scripted_fault_triggers() {
    let mut app = create_test_app();

    // Add the scripted fault system
    app.add_systems(Update, scripted_fault_system);

    // Create a charger with a scripted fault
    let mut charger = create_test_charger("CHG-001", ChargerType::DcFast);
    charger.scripted_fault_time = Some(5.0); // Trigger at 5 seconds
    charger.scripted_fault_type = Some(FaultType::CommunicationError);
    let entity = spawn_charger(&mut app, charger);

    // Advance game time past the trigger point
    {
        let mut game_clock = app
            .world_mut()
            .resource_mut::<kilowatt_tycoon::resources::GameClock>();
        game_clock.game_time = 6.0;
    }

    // Run the system
    app.update();

    // Check that the fault was triggered
    let charger = app.world().get::<Charger>(entity).unwrap();
    assert_eq!(charger.current_fault, Some(FaultType::CommunicationError));
    assert_eq!(charger.state(), ChargerState::Warning);

    // Scripted fault time should be cleared
    assert!(charger.scripted_fault_time.is_none());
}

#[test]
fn test_scripted_fault_not_triggered_before_time() {
    let mut app = create_test_app();

    // Add the scripted fault system
    app.add_systems(Update, scripted_fault_system);

    // Create a charger with a scripted fault
    let mut charger = create_test_charger("CHG-001", ChargerType::DcFast);
    charger.scripted_fault_time = Some(10.0); // Trigger at 10 seconds
    charger.scripted_fault_type = Some(FaultType::GroundFault);
    let entity = spawn_charger(&mut app, charger);

    // Set game time before the trigger point
    {
        let mut game_clock = app
            .world_mut()
            .resource_mut::<kilowatt_tycoon::resources::GameClock>();
        game_clock.game_time = 5.0;
    }

    // Run the system
    app.update();

    // Check that the fault was NOT triggered
    let charger = app.world().get::<Charger>(entity).unwrap();
    assert!(charger.current_fault.is_none());
    assert_eq!(charger.state(), ChargerState::Available);
}

#[test]
fn test_ground_fault_sets_offline_state() {
    let mut app = create_test_app();

    // Add the scripted fault system
    app.add_systems(Update, scripted_fault_system);

    // Create a charger with a ground fault scheduled
    let mut charger = create_test_charger("CHG-001", ChargerType::DcFast);
    charger.scripted_fault_time = Some(1.0);
    charger.scripted_fault_type = Some(FaultType::GroundFault);
    let entity = spawn_charger(&mut app, charger);

    // Advance past trigger
    {
        let mut game_clock = app
            .world_mut()
            .resource_mut::<kilowatt_tycoon::resources::GameClock>();
        game_clock.game_time = 2.0;
    }

    app.update();

    // Ground fault should set Offline state
    let charger = app.world().get::<Charger>(entity).unwrap();
    assert_eq!(charger.state(), ChargerState::Offline);
}

#[test]
fn test_connector_jam_sets_cable_stuck_state() {
    let mut app = create_test_app();

    app.add_systems(Update, scripted_fault_system);

    let mut charger = create_test_charger("CHG-001", ChargerType::DcFast);
    charger.scripted_fault_time = Some(1.0);
    charger.scripted_fault_type = Some(FaultType::CableDamage);
    let entity = spawn_charger(&mut app, charger);

    {
        let mut game_clock = app
            .world_mut()
            .resource_mut::<kilowatt_tycoon::resources::GameClock>();
        game_clock.game_time = 2.0;
    }

    app.update();

    let charger = app.world().get::<Charger>(entity).unwrap();
    assert_eq!(charger.state(), ChargerState::Offline);
}

#[test]
fn test_charger_type_power_ratings() {
    let dcfc = create_test_charger("DCFC-001", ChargerType::DcFast);
    let l2 = create_test_charger("L2-001", ChargerType::AcLevel2);

    // DCFC should have higher power rating
    assert!(dcfc.rated_power_kw > l2.rated_power_kw);
    assert_eq!(dcfc.rated_power_kw, 150.0);
    assert_eq!(l2.rated_power_kw, 7.0);
}

#[test]
fn test_anti_theft_cable_price_by_charger_type() {
    let l2 = create_test_charger("L2-001", ChargerType::AcLevel2);
    assert_eq!(l2.rated_power_kw, 7.0);
    assert_eq!(l2.anti_theft_cable_price(), 800);

    let mut dcfc50 = create_test_charger("DCFC50", ChargerType::DcFast);
    dcfc50.rated_power_kw = 50.0;
    assert_eq!(dcfc50.anti_theft_cable_price(), 3_200);

    let dcfc150 = create_test_charger("DCFC150", ChargerType::DcFast);
    assert_eq!(dcfc150.rated_power_kw, 150.0);
    assert_eq!(dcfc150.anti_theft_cable_price(), 6_000);

    let mut dcfc350 = create_test_charger("DCFC350", ChargerType::DcFast);
    dcfc350.rated_power_kw = 350.0;
    assert_eq!(dcfc350.anti_theft_cable_price(), 10_000);
}

#[test]
fn test_charger_state_display_names() {
    assert_eq!(ChargerState::Available.display_name(), "Available");
    assert_eq!(ChargerState::Charging.display_name(), "Charging");
    assert_eq!(ChargerState::Warning.display_name(), "Warning");
    assert_eq!(ChargerState::Offline.display_name(), "Offline");
    assert_eq!(ChargerState::Disabled.display_name(), "Disabled");
}

#[test]
fn test_fault_type_display_names() {
    assert_eq!(
        FaultType::CommunicationError.display_name(),
        "Communication Error"
    );
    assert_eq!(FaultType::CableDamage.display_name(), "Cable Damage");
    assert_eq!(FaultType::PaymentError.display_name(), "Payment Error");
    assert_eq!(FaultType::GroundFault.display_name(), "Ground Fault");
    assert_eq!(FaultType::FirmwareFault.display_name(), "Firmware Fault");
}

#[test]
fn test_remote_action_cooldowns() {
    // Soft reboot has shorter cooldown than hard reboot
    assert!(
        RemoteAction::SoftReboot.cooldown_seconds() < RemoteAction::HardReboot.cooldown_seconds()
    );

    // Disable has no cooldown
    assert_eq!(RemoteAction::Disable.cooldown_seconds(), 0.0);
}

#[test]
fn test_remote_action_success_rates() {
    // Both reboots always succeed
    assert_eq!(RemoteAction::SoftReboot.success_rate(), 1.0);
    assert_eq!(RemoteAction::HardReboot.success_rate(), 1.0);

    // Disable always succeeds
    assert_eq!(RemoteAction::Disable.success_rate(), 1.0);
}

#[test]
fn test_charger_can_accept_driver() {
    let mut charger = create_test_charger("CHG-001", ChargerType::DcFast);

    // Available charger can accept
    assert!(charger.can_accept_driver());

    // Disabled charger cannot
    charger.is_disabled = true;
    assert!(!charger.can_accept_driver());

    // Re-enable
    charger.is_disabled = false;
    assert!(charger.can_accept_driver());

    // Charging charger cannot accept another
    charger.is_charging = true;
    assert!(!charger.can_accept_driver());
}

#[test]
fn test_charger_derated_power() {
    let mut charger = create_test_charger("CHG-001", ChargerType::DcFast);
    charger.rated_power_kw = 150.0;

    // Get the tier efficiency to account for in calculations
    let tier_efficiency = charger.tier.efficiency(); // Standard = 0.92

    // Full health = rated * efficiency
    charger.health = 1.0;
    assert!((charger.get_derated_power() - (150.0 * tier_efficiency)).abs() < 0.1);

    // Degraded health = rated * health * efficiency
    charger.health = 0.8;
    assert!((charger.get_derated_power() - (150.0 * 0.8 * tier_efficiency)).abs() < 0.1);

    // Very degraded
    charger.health = 0.5;
    assert!((charger.get_derated_power() - (150.0 * 0.5 * tier_efficiency)).abs() < 0.1);
}

#[test]
fn test_charger_cooldown_management() {
    let mut charger = create_test_charger("CHG-001", ChargerType::DcFast);

    // No cooldown initially
    assert!(!charger.is_on_cooldown(RemoteAction::SoftReboot));

    // Start cooldown
    charger.start_cooldown(RemoteAction::SoftReboot);
    assert!(charger.is_on_cooldown(RemoteAction::SoftReboot));

    // Update cooldowns (reduce by 10 seconds)
    charger.update_cooldowns(10.0);

    // Should still be on cooldown
    assert!(charger.is_on_cooldown(RemoteAction::SoftReboot));

    // Update more to clear cooldown
    charger.update_cooldowns(30.0);
    assert!(!charger.is_on_cooldown(RemoteAction::SoftReboot));
}

#[test]
fn test_auto_dispatch_with_om_software() {
    // This test verifies that when O&M Software is active, a technician is
    // automatically dispatched when a charger develops a fault that requires one.
    let mut app = create_test_app();

    // Initialize TechnicianState resource
    app.init_resource::<TechnicianState>();

    // Add the technician dispatch message (already added by create_test_app)
    app.add_message::<TechnicianDispatchEvent>();

    // Create a site with O&M Software enabled
    let site_id = SiteId(1);
    {
        let mut multi_site = app.world_mut().resource_mut::<MultiSiteManager>();

        // Create a basic site state with O&M Software enabled
        let mut site_state = kilowatt_tycoon::resources::SiteState::new(
            site_id,
            SiteArchetype::ParkingLot,
            "Test Site".to_string(),
            500.0,    // grid_capacity_kva
            50.0,     // popularity
            1,        // challenge_level
            (16, 12), // grid_size
        );

        // Enable O&M Software (Optimize tier = 10s detection delay)
        site_state.site_upgrades.oem_tier = OemTier::Optimize;

        multi_site.owned_sites.insert(site_id, site_state);
        multi_site.viewed_site_id = Some(site_id);
    }

    // Add the fault system, detection system, auto-dispatch system, and dispatch system
    app.add_systems(
        Update,
        (
            scripted_fault_system,
            fault_detection_system,
            om_auto_dispatch_system,
            dispatch_technician_system,
        )
            .chain(),
    );

    // Create a charger with a scripted ground fault (requires technician)
    let mut charger = create_test_charger("CHG-001", ChargerType::DcFast);
    charger.scripted_fault_time = Some(5.0); // Trigger at 5 seconds
    charger.scripted_fault_type = Some(FaultType::GroundFault);

    // Spawn the charger with the BelongsToSite component
    let charger_entity = app
        .world_mut()
        .spawn((
            charger,
            Transform::default(),
            Visibility::default(),
            BelongsToSite::new(site_id),
        ))
        .id();

    // Verify technician is initially idle
    {
        let tech_state = app.world().resource::<TechnicianState>();
        assert_eq!(
            tech_state.status(),
            kilowatt_tycoon::resources::TechStatus::Idle
        );
        assert_eq!(tech_state.queue_len(), 0);
    }

    // Advance game time past the fault trigger point
    {
        let mut game_clock = app
            .world_mut()
            .resource_mut::<kilowatt_tycoon::resources::GameClock>();
        game_clock.game_time = 6.0;
        game_clock.total_game_time = 6.0;
    }

    // Run the first update cycle (fault system triggers the fault)
    app.update();

    // Verify the fault was triggered (but not yet detected — detection has a delay)
    {
        let charger = app.world().get::<Charger>(charger_entity).unwrap();
        assert_eq!(charger.current_fault, Some(FaultType::GroundFault));
        assert_eq!(charger.state(), ChargerState::Offline);
    }

    // Advance total_game_time past the Optimize tier detection delay (10s)
    {
        let mut game_clock = app
            .world_mut()
            .resource_mut::<kilowatt_tycoon::resources::GameClock>();
        game_clock.total_game_time = 20.0;
    }

    // Run second update: fault_detection_system detects the fault and emits ChargerFaultEvent
    app.update();

    // Verify fault is now discovered
    {
        let charger = app.world().get::<Charger>(charger_entity).unwrap();
        assert!(
            charger.fault_discovered,
            "Fault should be discovered with O&M Software after detection delay"
        );
    }

    // Run third update: om_auto_dispatch_system reads ChargerFaultEvent, dispatch queues
    app.update();

    // Run fourth update: dispatch_technician_system processes TechnicianDispatchEvent
    app.update();

    // Verify that a technician was dispatched
    // Note: dispatch_technician_system not only queues but also starts the job if tech is idle,
    // so the dispatch is popped from the queue and the technician status changes to EnRoute
    {
        let tech_state = app.world().resource::<TechnicianState>();
        assert_eq!(
            tech_state.status(),
            kilowatt_tycoon::resources::TechStatus::EnRoute,
            "Technician should be dispatched (EnRoute) for ground fault with O&M Software"
        );
        assert_eq!(
            tech_state.active_charger(),
            Some(charger_entity),
            "Technician should be assigned to the faulted charger"
        );
        assert_eq!(
            tech_state.destination_site_id(),
            Some(site_id),
            "Technician should be traveling to the correct site"
        );
    }
}

#[test]
fn test_visible_site_dispatch_is_prioritized_over_older_offscreen_work() {
    let mut app = create_test_app();
    app.init_resource::<TechnicianState>();
    app.init_resource::<PendingDispatchEvents>();
    app.add_systems(
        Update,
        (emit_pending_dispatches, dispatch_technician_system).chain(),
    );

    let site_a = SiteId(1);
    let site_b = SiteId(2);
    {
        let mut multi_site = app.world_mut().resource_mut::<MultiSiteManager>();

        let mut site_state_a = kilowatt_tycoon::resources::SiteState::new(
            site_a,
            SiteArchetype::ParkingLot,
            "Site A".to_string(),
            500.0,
            50.0,
            1,
            (16, 12),
        );
        site_state_a.site_upgrades.oem_tier = OemTier::Optimize;

        let mut site_state_b = kilowatt_tycoon::resources::SiteState::new(
            site_b,
            SiteArchetype::GasStation,
            "Site B".to_string(),
            500.0,
            50.0,
            1,
            (16, 12),
        );
        site_state_b.site_upgrades.oem_tier = OemTier::Optimize;

        multi_site.owned_sites.insert(site_a, site_state_a);
        multi_site.owned_sites.insert(site_b, site_state_b);
        multi_site.viewed_site_id = Some(site_b);
    }

    let charger_a = app
        .world_mut()
        .spawn((
            create_faulted_charger("CHG-A", FaultType::GroundFault),
            Transform::default(),
            Visibility::default(),
            BelongsToSite::new(site_a),
        ))
        .id();
    let charger_b = app
        .world_mut()
        .spawn((
            create_faulted_charger("CHG-B", FaultType::GroundFault),
            Transform::default(),
            Visibility::default(),
            BelongsToSite::new(site_b),
        ))
        .id();

    let request_a =
        create_repair_request(&mut app, charger_a, "CHG-A", site_a, FaultType::GroundFault);
    let request_b =
        create_repair_request(&mut app, charger_b, "CHG-B", site_b, FaultType::GroundFault);

    app.world_mut()
        .resource_mut::<PendingDispatchEvents>()
        .0
        .extend([
            TechnicianDispatchEvent {
                request_id: request_a,
                charger_entity: charger_a,
                charger_id: "CHG-A".to_string(),
            },
            TechnicianDispatchEvent {
                request_id: request_b,
                charger_entity: charger_b,
                charger_id: "CHG-B".to_string(),
            },
        ]);

    app.update();

    let tech_state = app.world().resource::<TechnicianState>();
    assert_eq!(
        tech_state.status(),
        TechStatus::EnRoute,
        "The technician should start the visible-site job first"
    );
    assert_eq!(tech_state.active_request_id(), Some(request_b));
    assert_eq!(tech_state.destination_site_id(), Some(site_b));
    assert_eq!(tech_state.queue_len(), 1);
    assert_eq!(
        tech_state
            .dispatch_queue
            .front()
            .map(|dispatch| dispatch.request_id),
        Some(request_a)
    );

    let requests = app.world().resource::<RepairRequestRegistry>();
    assert_eq!(
        requests.get(request_a).map(|request| request.status),
        Some(RepairRequestStatus::Queued)
    );
    assert_eq!(
        requests.get(request_b).map(|request| request.status),
        Some(RepairRequestStatus::EnRoute)
    );
}

#[test]
fn test_visible_site_dispatch_preempts_offscreen_waiting_job() {
    let mut app = create_test_app();
    app.init_resource::<TechnicianState>();
    app.init_resource::<PendingDispatchEvents>();
    app.add_systems(
        Update,
        (emit_pending_dispatches, dispatch_technician_system).chain(),
    );

    let site_a = SiteId(1);
    let site_b = SiteId(2);
    {
        let mut multi_site = app.world_mut().resource_mut::<MultiSiteManager>();

        let mut site_state_a = kilowatt_tycoon::resources::SiteState::new(
            site_a,
            SiteArchetype::ParkingLot,
            "Site A".to_string(),
            500.0,
            50.0,
            1,
            (16, 12),
        );
        site_state_a.site_upgrades.oem_tier = OemTier::Optimize;

        let mut site_state_b = kilowatt_tycoon::resources::SiteState::new(
            site_b,
            SiteArchetype::GasStation,
            "Site B".to_string(),
            500.0,
            50.0,
            1,
            (16, 12),
        );
        site_state_b.site_upgrades.oem_tier = OemTier::Optimize;

        multi_site.owned_sites.insert(site_a, site_state_a);
        multi_site.owned_sites.insert(site_b, site_state_b);
        multi_site.viewed_site_id = Some(site_b);
    }

    let offscreen_charger = app
        .world_mut()
        .spawn((
            create_faulted_charger("CHG-OFFSCREEN", FaultType::GroundFault),
            Transform::default(),
            Visibility::default(),
            BelongsToSite::new(site_a),
        ))
        .id();
    let visible_charger = app
        .world_mut()
        .spawn((
            create_faulted_charger("CHG-VISIBLE", FaultType::GroundFault),
            Transform::default(),
            Visibility::default(),
            BelongsToSite::new(site_b),
        ))
        .id();

    let offscreen_request = create_repair_request(
        &mut app,
        offscreen_charger,
        "CHG-OFFSCREEN",
        site_a,
        FaultType::GroundFault,
    );
    let visible_request = create_repair_request(
        &mut app,
        visible_charger,
        "CHG-VISIBLE",
        site_b,
        FaultType::GroundFault,
    );

    {
        let mut requests = app.world_mut().resource_mut::<RepairRequestRegistry>();
        let _ = requests.set_status(offscreen_request, RepairRequestStatus::WaitingAtSite);
        let _ = requests.mark_dispatch_costs_recorded(offscreen_request);
    }
    {
        let mut tech_state = app.world_mut().resource_mut::<TechnicianState>();
        tech_state.current_site_id = Some(site_a);
        tech_state.set_waiting_at_site_job(offscreen_request, offscreen_charger, site_a, 120.0);
    }

    app.world_mut()
        .resource_mut::<PendingDispatchEvents>()
        .0
        .push(TechnicianDispatchEvent {
            request_id: visible_request,
            charger_entity: visible_charger,
            charger_id: "CHG-VISIBLE".to_string(),
        });

    app.update();

    let tech_state = app.world().resource::<TechnicianState>();
    assert_eq!(
        tech_state.status(),
        TechStatus::EnRoute,
        "Visible-site work should preempt an offscreen waiting job"
    );
    assert_eq!(tech_state.active_request_id(), Some(visible_request));
    assert_eq!(tech_state.destination_site_id(), Some(site_b));
    assert_eq!(tech_state.queue_len(), 1);
    assert_eq!(
        tech_state
            .dispatch_queue
            .front()
            .map(|dispatch| dispatch.request_id),
        Some(offscreen_request)
    );

    let requests = app.world().resource::<RepairRequestRegistry>();
    assert_eq!(
        requests
            .get(offscreen_request)
            .map(|request| request.status),
        Some(RepairRequestStatus::Queued)
    );
    assert_eq!(
        requests
            .get(offscreen_request)
            .map(|request| request.dispatch_costs_recorded),
        Some(true),
        "Preempting offscreen work should preserve prior dispatch billing"
    );
    assert_eq!(
        requests.get(visible_request).map(|request| request.status),
        Some(RepairRequestStatus::EnRoute)
    );
}

#[test]
fn test_no_auto_dispatch_without_om_software() {
    // This test verifies that without O&M Software, faults do NOT trigger
    // automatic technician dispatch (fault is silently set, waits for driver discovery).
    let mut app = create_test_app();

    // Initialize TechnicianState resource
    app.init_resource::<TechnicianState>();
    app.add_message::<TechnicianDispatchEvent>();

    // Create a site WITHOUT O&M Software
    let site_id = SiteId(1);
    {
        let mut multi_site = app.world_mut().resource_mut::<MultiSiteManager>();

        let site_state = kilowatt_tycoon::resources::SiteState::new(
            site_id,
            SiteArchetype::ParkingLot,
            "Test Site".to_string(),
            500.0,
            50.0,
            1,
            (16, 12),
        );

        // O&M Software is NOT enabled (default is None)
        assert!(!site_state.site_upgrades.has_om_software());

        multi_site.owned_sites.insert(site_id, site_state);
        multi_site.viewed_site_id = Some(site_id);
    }

    // Add both systems
    app.add_systems(Update, (scripted_fault_system, om_auto_dispatch_system));

    // Create a charger with a scripted ground fault
    let mut charger = create_test_charger("CHG-001", ChargerType::DcFast);
    charger.scripted_fault_time = Some(5.0);
    charger.scripted_fault_type = Some(FaultType::GroundFault);

    let charger_entity = app
        .world_mut()
        .spawn((
            charger,
            Transform::default(),
            Visibility::default(),
            BelongsToSite::new(site_id),
        ))
        .id();

    // Advance game time past the fault trigger
    {
        let mut game_clock = app
            .world_mut()
            .resource_mut::<kilowatt_tycoon::resources::GameClock>();
        game_clock.game_time = 6.0;
    }

    // Run the update
    app.update();

    // Verify the fault was set but NOT discovered
    {
        let charger = app.world().get::<Charger>(charger_entity).unwrap();
        assert_eq!(charger.current_fault, Some(FaultType::GroundFault));
        assert!(
            !charger.fault_discovered,
            "Fault should NOT be discovered without O&M Software"
        );
    }

    // Verify that NO technician dispatch occurred
    {
        let tech_state = app.world().resource::<TechnicianState>();
        assert_eq!(
            tech_state.status(),
            kilowatt_tycoon::resources::TechStatus::Idle,
            "Technician should remain Idle without O&M Software"
        );
        assert_eq!(
            tech_state.queue_len(),
            0,
            "No dispatch should be queued without O&M Software"
        );
        assert_eq!(
            tech_state.active_charger(),
            None,
            "Technician should not have a target charger"
        );
    }
}

#[test]
fn test_auto_dispatch_only_for_technician_faults() {
    // This test verifies that auto-dispatch only occurs for faults that
    // require a technician (GroundFault, CableDamage), not for remote-fixable
    // faults (CommunicationError, PaymentError, FirmwareFault).
    let mut app = create_test_app();

    app.init_resource::<TechnicianState>();
    app.add_message::<TechnicianDispatchEvent>();

    // Create a site with O&M Software enabled
    let site_id = SiteId(1);
    {
        let mut multi_site = app.world_mut().resource_mut::<MultiSiteManager>();

        let mut site_state = kilowatt_tycoon::resources::SiteState::new(
            site_id,
            SiteArchetype::ParkingLot,
            "Test Site".to_string(),
            500.0,
            50.0,
            1,
            (16, 12),
        );

        // Use Optimize tier (10s detection, auto-dispatch, auto-remediation)
        // CommunicationError will be auto-remediated, so no dispatch should occur
        site_state.site_upgrades.oem_tier = OemTier::Optimize;

        multi_site.owned_sites.insert(site_id, site_state);
        multi_site.viewed_site_id = Some(site_id);
    }

    app.add_systems(
        Update,
        (
            scripted_fault_system,
            fault_detection_system,
            om_auto_dispatch_system,
        )
            .chain(),
    );

    // Create a charger with a CommunicationError (does NOT require technician)
    let mut charger = create_test_charger("CHG-001", ChargerType::DcFast);
    charger.scripted_fault_time = Some(5.0);
    charger.scripted_fault_type = Some(FaultType::CommunicationError);

    let charger_entity = app
        .world_mut()
        .spawn((
            charger,
            Transform::default(),
            Visibility::default(),
            BelongsToSite::new(site_id),
        ))
        .id();

    // Advance time and trigger fault
    {
        let mut game_clock = app
            .world_mut()
            .resource_mut::<kilowatt_tycoon::resources::GameClock>();
        game_clock.game_time = 6.0;
        game_clock.total_game_time = 6.0;
    }

    // First update: fault triggers
    app.update();

    // Advance time past detection delay (10s)
    {
        let mut game_clock = app
            .world_mut()
            .resource_mut::<kilowatt_tycoon::resources::GameClock>();
        game_clock.total_game_time = 20.0;
    }

    // Second update: fault detected and auto-remediated (software fault)
    app.update();

    // CommunicationError should be auto-remediated (cleared) since it doesn't require technician
    {
        let charger = app.world().get::<Charger>(charger_entity).unwrap();
        assert_eq!(
            charger.current_fault, None,
            "Software fault should be auto-remediated"
        );
    }

    // Verify NO technician was dispatched (communication error was auto-remediated)
    {
        let tech_state = app.world().resource::<TechnicianState>();
        assert_eq!(
            tech_state.status(),
            kilowatt_tycoon::resources::TechStatus::Idle,
            "Technician should remain Idle for auto-remediated faults"
        );
        assert_eq!(
            tech_state.queue_len(),
            0,
            "No dispatch should be queued for auto-remediated faults"
        );
        assert_eq!(
            tech_state.active_charger(),
            None,
            "Technician should not have a target charger"
        );
    }
}

#[test]
fn test_fault_detection_with_om_software() {
    // This test verifies that with O&M Software, faults are detected after the
    // tier detection delay without needing a driver to discover them.
    let mut app = create_test_app();
    app.init_resource::<TechnicianState>();
    app.add_message::<TechnicianDispatchEvent>();

    // Create a site with O&M Software enabled
    let site_id = SiteId(1);
    {
        let mut multi_site = app.world_mut().resource_mut::<MultiSiteManager>();
        let mut site_state = kilowatt_tycoon::resources::SiteState::new(
            site_id,
            SiteArchetype::ParkingLot,
            "Test Site".to_string(),
            500.0,
            50.0,
            1,
            (16, 12),
        );

        // Enable O&M Software (Optimize tier = 10s detection delay)
        site_state.site_upgrades.oem_tier = OemTier::Optimize;

        multi_site.owned_sites.insert(site_id, site_state);
        multi_site.viewed_site_id = Some(site_id);
    }

    // Add the fault system and detection system
    app.add_systems(
        Update,
        (scripted_fault_system, fault_detection_system).chain(),
    );

    // Create a charger with a scripted ground fault
    let mut charger = create_test_charger("CHG-001", ChargerType::DcFast);
    charger.scripted_fault_time = Some(5.0);
    charger.scripted_fault_type = Some(FaultType::GroundFault);

    let charger_entity = app
        .world_mut()
        .spawn((
            charger,
            Transform::default(),
            Visibility::default(),
            BelongsToSite::new(site_id),
        ))
        .id();

    // Verify fault is not yet present
    {
        let charger = app.world().get::<Charger>(charger_entity).unwrap();
        assert!(
            charger.current_fault.is_none(),
            "Charger should start without fault"
        );
        assert!(!charger.fault_discovered, "No fault to discover yet");
    }

    // Advance time to trigger the fault (but no driver has arrived yet)
    {
        let mut game_clock = app.world_mut().resource_mut::<GameClock>();
        game_clock.game_time = 6.0;
        game_clock.total_game_time = 6.0;
    }

    // First update: fault system triggers the fault
    app.update();

    // Verify fault is set but not yet detected (detection has a delay)
    {
        let charger = app.world().get::<Charger>(charger_entity).unwrap();
        assert_eq!(
            charger.current_fault,
            Some(FaultType::GroundFault),
            "Fault should be set on charger"
        );
    }

    // Advance total_game_time past the Optimize detection delay (10s)
    {
        let mut game_clock = app.world_mut().resource_mut::<GameClock>();
        game_clock.total_game_time = 20.0;
    }

    // Second update: fault_detection_system detects the fault
    app.update();

    // KEY ASSERTION: With O&M Software, fault should be detected after the detection delay,
    // without needing a driver to discover it
    {
        let charger = app.world().get::<Charger>(charger_entity).unwrap();
        assert_eq!(
            charger.current_fault,
            Some(FaultType::GroundFault),
            "Fault should still be set on charger"
        );
        assert!(
            charger.fault_discovered,
            "WITH O&M Software: Fault should be discovered after detection delay, \
             before any driver arrives"
        );
        assert_eq!(
            charger.state(),
            ChargerState::Offline,
            "Charger should be offline due to ground fault"
        );
    }
}

#[test]
fn test_detect_tier_auto_remediates_software_fault_after_delay() {
    let mut app = create_test_app();

    let site_id = SiteId(1);
    {
        let mut multi_site = app.world_mut().resource_mut::<MultiSiteManager>();
        let mut site_state = kilowatt_tycoon::resources::SiteState::new(
            site_id,
            SiteArchetype::ParkingLot,
            "Test Site".to_string(),
            500.0,
            50.0,
            1,
            (16, 12),
        );
        site_state.site_upgrades.oem_tier = OemTier::Detect;

        multi_site.owned_sites.insert(site_id, site_state);
        multi_site.viewed_site_id = Some(site_id);
    }

    app.add_systems(
        Update,
        (scripted_fault_system, fault_detection_system).chain(),
    );

    let mut charger = create_test_charger("CHG-001", ChargerType::DcFast);
    charger.scripted_fault_time = Some(5.0);
    charger.scripted_fault_type = Some(FaultType::CommunicationError);

    let charger_entity = app
        .world_mut()
        .spawn((
            charger,
            Transform::default(),
            Visibility::default(),
            BelongsToSite::new(site_id),
        ))
        .id();

    {
        let mut game_clock = app.world_mut().resource_mut::<GameClock>();
        game_clock.game_time = 6.0;
        game_clock.total_game_time = 6.0;
    }

    app.update();

    {
        let charger = app.world().get::<Charger>(charger_entity).unwrap();
        assert_eq!(charger.current_fault, Some(FaultType::CommunicationError));
        assert!(
            !charger.fault_discovered,
            "Detect should still respect the normal detection delay"
        );
    }

    {
        let mut game_clock = app.world_mut().resource_mut::<GameClock>();
        game_clock.total_game_time = 20.0;
    }

    app.update();

    {
        let charger = app.world().get::<Charger>(charger_entity).unwrap();
        assert_eq!(
            charger.current_fault, None,
            "Detect should auto-remediate software faults once the delay elapses"
        );
        assert!(
            !charger.fault_discovered,
            "Auto-remediation should leave no discovered fault behind"
        );
    }
}

#[test]
fn test_detect_tier_driver_discovered_software_fault_still_auto_remediates() {
    let mut app = create_test_app();
    app.init_resource::<TechnicianState>();
    app.init_resource::<ChargerIndex>();

    let site_id = SiteId(1);
    {
        let mut multi_site = app.world_mut().resource_mut::<MultiSiteManager>();
        let mut site_state = kilowatt_tycoon::resources::SiteState::new(
            site_id,
            SiteArchetype::ParkingLot,
            "Test Site".to_string(),
            500.0,
            50.0,
            1,
            (16, 12),
        );
        site_state.site_upgrades.oem_tier = OemTier::Detect;

        multi_site.owned_sites.insert(site_id, site_state);
        multi_site.viewed_site_id = Some(site_id);
    }

    app.add_systems(
        Update,
        (
            scripted_fault_system,
            driver_arrival_system,
            fault_detection_system,
        )
            .chain(),
    );

    let mut charger = create_test_charger("CHG-001", ChargerType::DcFast);
    charger.scripted_fault_time = Some(5.0);
    charger.scripted_fault_type = Some(FaultType::CommunicationError);

    let charger_entity = app
        .world_mut()
        .spawn((
            charger,
            Transform::default(),
            Visibility::default(),
            BelongsToSite::new(site_id),
        ))
        .id();

    {
        let mut index = app.world_mut().resource_mut::<ChargerIndex>();
        index.by_id.insert("CHG-001".to_string(), charger_entity);
    }

    let mut driver = create_test_driver("DRV-001");
    driver.state = DriverState::Arriving;
    driver.assigned_charger = Some(charger_entity);

    let movement = VehicleMovement {
        phase: MovementPhase::Parked,
        ..Default::default()
    };

    let driver_entity = app
        .world_mut()
        .spawn((
            driver,
            movement,
            Transform::default(),
            Visibility::default(),
            BelongsToSite::new(site_id),
        ))
        .id();

    {
        let mut game_clock = app.world_mut().resource_mut::<GameClock>();
        game_clock.game_time = 6.0;
        game_clock.total_game_time = 6.0;
    }

    app.update();

    {
        let charger = app.world().get::<Charger>(charger_entity).unwrap();
        assert_eq!(
            charger.current_fault,
            Some(FaultType::CommunicationError),
            "Driver discovery should not clear the software fault before the Detect delay"
        );
        assert!(
            charger.fault_discovered,
            "Driver should still be able to discover the fault before O&M handles it"
        );
    }

    {
        let driver = app.world().get::<Driver>(driver_entity).unwrap();
        assert_eq!(driver.state, DriverState::Frustrated);
    }

    {
        let mut game_clock = app.world_mut().resource_mut::<GameClock>();
        game_clock.total_game_time = 20.0;
    }

    app.update();

    {
        let charger = app.world().get::<Charger>(charger_entity).unwrap();
        assert_eq!(
            charger.current_fault, None,
            "Detect should still auto-remediate after the delay even if a driver found the fault first"
        );
        assert!(
            !charger.fault_discovered,
            "The driver discovery path should not strand a discovered manual reboot behind O&M Detect"
        );
    }
}

#[test]
fn test_detect_tier_does_not_auto_dispatch_hardware_faults() {
    let mut app = create_test_app();
    app.init_resource::<TechnicianState>();
    app.add_message::<TechnicianDispatchEvent>();

    let site_id = SiteId(1);
    {
        let mut multi_site = app.world_mut().resource_mut::<MultiSiteManager>();
        let mut site_state = kilowatt_tycoon::resources::SiteState::new(
            site_id,
            SiteArchetype::ParkingLot,
            "Test Site".to_string(),
            500.0,
            50.0,
            1,
            (16, 12),
        );
        site_state.site_upgrades.oem_tier = OemTier::Detect;

        multi_site.owned_sites.insert(site_id, site_state);
        multi_site.viewed_site_id = Some(site_id);
    }

    app.add_systems(
        Update,
        (
            scripted_fault_system,
            fault_detection_system,
            om_auto_dispatch_system,
            dispatch_technician_system,
        )
            .chain(),
    );

    let mut charger = create_test_charger("CHG-001", ChargerType::DcFast);
    charger.scripted_fault_time = Some(5.0);
    charger.scripted_fault_type = Some(FaultType::GroundFault);

    let charger_entity = app
        .world_mut()
        .spawn((
            charger,
            Transform::default(),
            Visibility::default(),
            BelongsToSite::new(site_id),
        ))
        .id();

    {
        let mut game_clock = app.world_mut().resource_mut::<GameClock>();
        game_clock.game_time = 6.0;
        game_clock.total_game_time = 6.0;
    }

    app.update();

    {
        let mut game_clock = app.world_mut().resource_mut::<GameClock>();
        game_clock.total_game_time = 20.0;
    }

    app.update();
    app.update();

    {
        let charger = app.world().get::<Charger>(charger_entity).unwrap();
        assert_eq!(
            charger.current_fault,
            Some(FaultType::GroundFault),
            "Detect should not auto-remediate hardware faults"
        );
        assert!(charger.fault_discovered);
    }

    {
        let tech_state = app.world().resource::<TechnicianState>();
        assert_eq!(
            tech_state.status(),
            TechStatus::Idle,
            "Detect should not auto-dispatch the technician for hardware faults"
        );
        assert_eq!(tech_state.queue_len(), 0);
        assert_eq!(tech_state.active_charger(), None);
    }
}

#[test]
fn test_fault_detection_without_om_software_driver_discovery() {
    // This test verifies that WITHOUT O&M Software, faults are NOT detected immediately.
    // Instead, they're only discovered when a driver tries to use the charger and fails.
    let mut app = create_test_app();
    app.init_resource::<TechnicianState>();
    app.init_resource::<ChargerIndex>(); // Needed by driver_arrival_system
    app.add_message::<TechnicianDispatchEvent>();

    // Create a site WITHOUT O&M Software
    let site_id = SiteId(1);
    {
        let mut multi_site = app.world_mut().resource_mut::<MultiSiteManager>();
        let site_state = kilowatt_tycoon::resources::SiteState::new(
            site_id,
            SiteArchetype::ParkingLot,
            "Test Site".to_string(),
            500.0,
            50.0,
            1,
            (16, 12),
        );

        // O&M Software is NOT enabled
        assert!(!site_state.site_upgrades.has_om_software());

        multi_site.owned_sites.insert(site_id, site_state);
        multi_site.viewed_site_id = Some(site_id);
    }

    // Add both systems: fault injection and driver arrival
    app.add_systems(Update, (scripted_fault_system, driver_arrival_system));

    // Create a charger with a scripted ground fault
    let mut charger = create_test_charger("CHG-001", ChargerType::DcFast);
    charger.scripted_fault_time = Some(5.0);
    charger.scripted_fault_type = Some(FaultType::GroundFault);

    let charger_entity = app
        .world_mut()
        .spawn((
            charger,
            Transform::default(),
            Visibility::default(),
            BelongsToSite::new(site_id),
        ))
        .id();

    // Advance time to trigger the fault
    {
        let mut game_clock = app.world_mut().resource_mut::<GameClock>();
        game_clock.game_time = 6.0;
    }

    // Run update - fault system triggers
    app.update();

    // Verify fault is set but NOT discovered (no driver has arrived yet)
    {
        let charger = app.world().get::<Charger>(charger_entity).unwrap();
        assert_eq!(
            charger.current_fault,
            Some(FaultType::GroundFault),
            "Fault should be set on charger"
        );
        assert!(
            !charger.fault_discovered,
            "WITHOUT O&M Software: Fault should NOT be discovered until a driver tries to use it"
        );
        assert_eq!(
            charger.state(),
            ChargerState::Offline,
            "Charger should be offline (visual indicator), but fault is undiscovered"
        );
    }

    // Manually register the charger in the ChargerIndex (since we're not using HooksPlugin)
    {
        let mut index = app.world_mut().resource_mut::<ChargerIndex>();
        index.by_id.insert("CHG-001".to_string(), charger_entity);
    }

    // Now create a driver that arrives at this broken charger
    let mut driver = create_test_driver("DRV-001");
    driver.state = DriverState::Arriving;
    driver.assigned_charger = Some(charger_entity);

    let movement = VehicleMovement {
        phase: MovementPhase::Parked, // Driver has arrived
        ..Default::default()
    };

    let driver_entity = app
        .world_mut()
        .spawn((
            driver,
            movement,
            Transform::default(),
            Visibility::default(),
            BelongsToSite::new(site_id),
        ))
        .id();

    // Store initial patience to verify it decreased
    let initial_patience = app.world().get::<Driver>(driver_entity).unwrap().patience;

    // Run update - driver arrival system processes the driver
    app.update();

    // KEY ASSERTIONS: Driver discovers the fault
    {
        let charger = app.world().get::<Charger>(charger_entity).unwrap();
        assert!(
            charger.fault_discovered,
            "After driver arrives: Fault should NOW be discovered"
        );
    }

    {
        let driver = app.world().get::<Driver>(driver_entity).unwrap();
        assert_eq!(
            driver.state,
            DriverState::Frustrated,
            "Driver should be frustrated after finding broken charger"
        );
        assert!(
            driver.patience < initial_patience,
            "Driver's patience should decrease when they find a broken charger (initial: {}, current: {})",
            initial_patience,
            driver.patience
        );
    }
}

#[test]
fn test_fault_discovered_flag_prevents_duplicate_events() {
    // This test verifies that the fault_discovered flag prevents duplicate
    // fault events when multiple drivers encounter the same broken charger.
    let mut app = create_test_app();
    app.init_resource::<TechnicianState>();
    app.init_resource::<ChargerIndex>(); // Needed by driver_arrival_system

    // Create a site WITHOUT O&M Software
    let site_id = SiteId(1);
    {
        let mut multi_site = app.world_mut().resource_mut::<MultiSiteManager>();
        let site_state = kilowatt_tycoon::resources::SiteState::new(
            site_id,
            SiteArchetype::ParkingLot,
            "Test Site".to_string(),
            500.0,
            50.0,
            1,
            (16, 12),
        );

        multi_site.owned_sites.insert(site_id, site_state);
        multi_site.viewed_site_id = Some(site_id);
    }

    app.add_systems(Update, driver_arrival_system);

    // Create a charger with a fault already set (manually, not via system)
    let mut charger = create_test_charger("CHG-001", ChargerType::DcFast);
    charger.current_fault = Some(FaultType::GroundFault);
    charger.fault_discovered = false; // Not yet discovered

    let charger_entity = app
        .world_mut()
        .spawn((
            charger,
            Transform::default(),
            Visibility::default(),
            BelongsToSite::new(site_id),
        ))
        .id();

    // Manually register the charger in the ChargerIndex (since we're not using HooksPlugin)
    {
        let mut index = app.world_mut().resource_mut::<ChargerIndex>();
        index.by_id.insert("CHG-001".to_string(), charger_entity);
    }

    // Create first driver
    let mut driver1 = create_test_driver("DRV-001");
    driver1.state = DriverState::Arriving;
    driver1.assigned_charger = Some(charger_entity);
    let movement1 = VehicleMovement {
        phase: MovementPhase::Parked,
        ..Default::default()
    };

    app.world_mut().spawn((
        driver1,
        movement1,
        Transform::default(),
        Visibility::default(),
        BelongsToSite::new(site_id),
    ));

    // First driver arrives - should discover fault
    app.update();

    // Verify fault is now discovered
    {
        let charger = app.world().get::<Charger>(charger_entity).unwrap();
        assert!(
            charger.fault_discovered,
            "First driver should discover the fault"
        );
    }

    // Now create a second driver arriving at the same broken charger
    let mut driver2 = create_test_driver("DRV-002");
    driver2.state = DriverState::Arriving;
    driver2.assigned_charger = Some(charger_entity);
    let movement2 = VehicleMovement {
        phase: MovementPhase::Parked,
        ..Default::default()
    };

    app.world_mut().spawn((
        driver2,
        movement2,
        Transform::default(),
        Visibility::default(),
        BelongsToSite::new(site_id),
    ));

    // Second driver arrives - should NOT trigger another fault event
    // (The system checks fault_discovered flag before emitting)
    app.update();

    // Verify fault is still discovered (didn't change)
    {
        let charger = app.world().get::<Charger>(charger_entity).unwrap();
        assert!(
            charger.fault_discovered,
            "Fault should remain discovered (not reset)"
        );
    }

    // Note: We can't easily verify that only ONE event was emitted in this test
    // structure, but the code logic shows it checks !fault_discovered before
    // emitting the event. This test mainly verifies the flag persists correctly.
}

/// Test that when a technician fails to repair a charger and O&M Software is active,
/// a new dispatch event is automatically triggered for retry.
#[test]
fn test_om_auto_redispatch_on_repair_failure() {
    let mut app = create_test_app();

    // Add required messages
    app.add_message::<TechnicianDispatchEvent>();
    app.add_message::<RepairCompleteEvent>();
    app.add_message::<RepairFailedEvent>();

    // Initialize technician state
    app.init_resource::<TechnicianState>();

    // Create a site WITH O&M Software enabled
    let site_id = SiteId(1);
    {
        let mut multi_site = app.world_mut().resource_mut::<MultiSiteManager>();
        let mut site_state = kilowatt_tycoon::resources::SiteState::new(
            site_id,
            SiteArchetype::ParkingLot,
            "Test Site".to_string(),
            500.0,
            50.0,
            1,
            (16, 12),
        );

        // Enable O&M Optimize tier (auto-dispatch + auto-redispatch on failure)
        site_state.site_upgrades.oem_tier = OemTier::Optimize;

        multi_site.owned_sites.insert(site_id, site_state);
        multi_site.viewed_site_id = Some(site_id);
    }

    // Create a charger with a fault that requires technician
    let mut charger = create_test_charger("CHG-001", ChargerType::DcFast);
    charger.current_fault = Some(FaultType::GroundFault);
    charger.grid_position = Some((5, 5));

    let charger_entity = app
        .world_mut()
        .spawn((
            charger,
            Transform::default(),
            GlobalTransform::default(),
            Visibility::default(),
            BelongsToSite::new(site_id),
        ))
        .id();

    // Add required Bevy resources for the systems
    app.init_resource::<Time>();
    app.init_resource::<Assets<Image>>();
    app.init_resource::<kilowatt_tycoon::resources::ImageAssets>();
    app.add_message::<kilowatt_tycoon::events::ChargerFaultResolvedEvent>();

    // Ensure game clock is not paused
    {
        let mut game_clock = app.world_mut().resource_mut::<GameClock>();
        game_clock.set_speed(GameSpeed::Normal);
    }

    // Add the repair and dispatch systems in correct order
    // (dispatch must run AFTER repair to process the events)
    app.add_systems(
        Update,
        (
            technician_repair_system,
            dispatch_technician_system.after(technician_repair_system),
        ),
    );

    // Run the system multiple times to trigger repair completion
    // Due to probabilistic failure (20% at default $10/hr maintenance), we need many iterations
    let mut auto_redispatch_occurred = false;
    let mut repair_failed_at_least_once = false;

    for _iteration in 0..200 {
        let request_id = create_repair_request(
            &mut app,
            charger_entity,
            "CHG-001",
            site_id,
            FaultType::GroundFault,
        );
        // Reset technician state for next repair attempt
        {
            set_technician_repairing(&mut app, request_id, charger_entity, site_id, 0.0);
            let mut requests = app.world_mut().resource_mut::<RepairRequestRegistry>();
            assert!(requests.set_status(request_id, RepairRequestStatus::Repairing));
            let mut tech_state = app.world_mut().resource_mut::<TechnicianState>();
            tech_state.job_time_elapsed = 900.0;
            tech_state.dispatch_queue.clear();
        }

        {
            let technician_entities: Vec<_> = {
                let world = app.world_mut();
                let mut query = world.query_filtered::<Entity, With<Technician>>();
                query.iter(world).collect()
            };
            for entity in technician_entities {
                app.world_mut().entity_mut(entity).despawn();
            }
            app.world_mut().spawn((
                Technician {
                    target_charger: charger_entity,
                    phase: TechnicianPhase::Working,
                    work_timer: 0.0,
                    target_bay: Some((5, 5)),
                },
                TechnicianMovement {
                    phase: TechnicianPhase::Working,
                    speed: 60.0,
                },
                TechnicianEmotion::default(),
                BelongsToSite::new(site_id),
                bevy_northstar::prelude::AgentPos(bevy::prelude::UVec3::new(5, 5, 0)),
            ));
        }

        // Ensure charger still has fault for retry
        {
            let mut charger = app.world_mut().get_mut::<Charger>(charger_entity).unwrap();
            charger.current_fault = Some(FaultType::GroundFault);
        }

        // Run one update to process repair completion
        app.update();

        // Check the charger state to see if repair failed (fault still present)
        let repair_failed = {
            let charger = app.world().get::<Charger>(charger_entity).unwrap();
            charger.current_fault.is_some()
        };

        if repair_failed {
            repair_failed_at_least_once = true;

            // Check if a dispatch was queued (check dispatch_queue directly, not target_charger)
            let queue_has_charger = {
                let tech_state = app.world().resource::<TechnicianState>();
                tech_state
                    .dispatch_queue
                    .iter()
                    .any(|q| q.charger_entity == charger_entity)
            };

            if queue_has_charger {
                auto_redispatch_occurred = true;
                break; // Success! Found what we're looking for
            }
        }
    }

    assert!(
        repair_failed_at_least_once,
        "Expected at least one repair failure in 200 iterations (20% failure rate at default maintenance)"
    );
    assert!(
        auto_redispatch_occurred,
        "Expected auto-dispatch to queue charger when repair fails with O&M Software active"
    );
}

/// Test that retryable failed work survives the leaving-site transition and
/// becomes a fresh dispatch once the technician is idle again.
#[test]
fn test_failed_repair_retry_returns_after_leaving_site() {
    let mut app = create_test_app();
    app.init_resource::<TechnicianState>();
    let site_id = SiteId(1);
    {
        let mut multi_site = app.world_mut().resource_mut::<MultiSiteManager>();
        let site_state = kilowatt_tycoon::resources::SiteState::new(
            site_id,
            SiteArchetype::ParkingLot,
            "Test Site".to_string(),
            500.0,
            50.0,
            1,
            (16, 12),
        );
        multi_site.owned_sites.insert(site_id, site_state);
        multi_site.viewed_site_id = Some(site_id);
    }

    let mut charger = create_test_charger("CHG-001", ChargerType::DcFast);
    charger.current_fault = Some(FaultType::GroundFault);
    let charger_entity = app
        .world_mut()
        .spawn((
            charger,
            Transform::default(),
            GlobalTransform::default(),
            Visibility::default(),
            BelongsToSite::new(site_id),
        ))
        .id();

    let request_id = create_repair_request(
        &mut app,
        charger_entity,
        "CHG-001",
        site_id,
        FaultType::GroundFault,
    );
    {
        let mut requests = app.world_mut().resource_mut::<RepairRequestRegistry>();
        assert!(requests.mark_retry_needed(request_id, Some(5.0)));
        assert!(requests.mark_dispatch_costs_recorded(request_id));
    }
    {
        let mut tech_state = app.world_mut().resource_mut::<TechnicianState>();
        tech_state.current_site_id = Some(site_id);
        tech_state.begin_leaving_site(site_id);
    }

    app.add_systems(
        Update,
        (
            recover_orphaned_leaving_technician_system,
            recover_dispatchable_requests_system,
            dispatch_technician_system,
        )
            .chain(),
    );

    app.update();

    let tech_state = app.world().resource::<TechnicianState>();
    assert_eq!(tech_state.status(), TechStatus::EnRoute);
    assert_eq!(tech_state.active_charger(), Some(charger_entity));

    let requests = app.world().resource::<RepairRequestRegistry>();
    let request = requests
        .get(request_id)
        .expect("retry request should still exist");
    assert_eq!(request.status, RepairRequestStatus::EnRoute);
    assert!(request.dispatch_costs_recorded);
}

#[test]
fn test_terminal_request_cannot_enqueue_dispatch_work() {
    let mut app = create_test_app();
    app.init_resource::<TechnicianState>();
    app.init_resource::<PendingDispatchEvent>();
    app.add_systems(
        Update,
        (emit_pending_dispatch, dispatch_technician_system).chain(),
    );

    let site_id = SiteId(1);
    {
        let mut multi_site = app.world_mut().resource_mut::<MultiSiteManager>();
        let site_state = kilowatt_tycoon::resources::SiteState::new(
            site_id,
            SiteArchetype::ParkingLot,
            "Test Site".to_string(),
            500.0,
            50.0,
            1,
            (16, 12),
        );
        multi_site.owned_sites.insert(site_id, site_state);
        multi_site.viewed_site_id = Some(site_id);
    }

    let mut charger = create_test_charger("CHG-TERM", ChargerType::DcFast);
    charger.current_fault = Some(FaultType::GroundFault);
    let charger_entity = app
        .world_mut()
        .spawn((
            charger,
            Transform::default(),
            Visibility::default(),
            BelongsToSite::new(site_id),
        ))
        .id();

    let request_id = create_repair_request(
        &mut app,
        charger_entity,
        "CHG-TERM",
        site_id,
        FaultType::GroundFault,
    );
    {
        let mut requests = app.world_mut().resource_mut::<RepairRequestRegistry>();
        assert!(requests.resolve(request_id, 5.0, RepairResolution::Cancelled));
    }

    app.world_mut().resource_mut::<PendingDispatchEvent>().0 = Some(TechnicianDispatchEvent {
        request_id,
        charger_entity,
        charger_id: "CHG-TERM".to_string(),
    });

    app.update();

    let tech_state = app.world().resource::<TechnicianState>();
    assert_eq!(tech_state.status(), TechStatus::Idle);
    assert_eq!(tech_state.queue_len(), 0);

    let requests = app.world().resource::<RepairRequestRegistry>();
    assert_eq!(
        requests.get(request_id).map(|request| request.status),
        Some(RepairRequestStatus::Cancelled)
    );
}

#[test]
fn test_skipped_queued_job_reconciles_request_before_next_dispatch_starts() {
    let mut app = create_test_app();
    app.init_resource::<TechnicianState>();
    app.add_systems(Update, dispatch_technician_system);

    let site_id = SiteId(1);
    {
        let mut multi_site = app.world_mut().resource_mut::<MultiSiteManager>();
        let site_state = kilowatt_tycoon::resources::SiteState::new(
            site_id,
            SiteArchetype::ParkingLot,
            "Test Site".to_string(),
            500.0,
            50.0,
            1,
            (16, 12),
        );
        multi_site.owned_sites.insert(site_id, site_state);
        multi_site.viewed_site_id = Some(site_id);
    }

    let skipped_charger = app
        .world_mut()
        .spawn((
            create_test_charger("CHG-SKIP", ChargerType::DcFast),
            Transform::default(),
            Visibility::default(),
            BelongsToSite::new(site_id),
        ))
        .id();
    let mut valid_charger = create_test_charger("CHG-NEXT", ChargerType::DcFast);
    valid_charger.current_fault = Some(FaultType::GroundFault);
    let valid_charger_entity = app
        .world_mut()
        .spawn((
            valid_charger,
            Transform::default(),
            Visibility::default(),
            BelongsToSite::new(site_id),
        ))
        .id();

    let skipped_request_id = create_repair_request(
        &mut app,
        skipped_charger,
        "CHG-SKIP",
        site_id,
        FaultType::GroundFault,
    );
    let valid_request_id = create_repair_request(
        &mut app,
        valid_charger_entity,
        "CHG-NEXT",
        site_id,
        FaultType::GroundFault,
    );

    {
        let mut tech_state = app.world_mut().resource_mut::<TechnicianState>();
        assert!(tech_state.queue_dispatch(
            skipped_request_id,
            skipped_charger,
            "CHG-SKIP".to_string(),
            site_id,
        ));
        assert!(tech_state.queue_dispatch(
            valid_request_id,
            valid_charger_entity,
            "CHG-NEXT".to_string(),
            site_id,
        ));
    }

    app.update();

    let requests = app.world().resource::<RepairRequestRegistry>();
    assert_eq!(
        requests
            .get(skipped_request_id)
            .map(|request| request.status),
        Some(RepairRequestStatus::Resolved)
    );
    assert_eq!(
        requests.get(valid_request_id).map(|request| request.status),
        Some(RepairRequestStatus::EnRoute)
    );

    let tech_state = app.world().resource::<TechnicianState>();
    assert_eq!(tech_state.status(), TechStatus::EnRoute);
    assert_eq!(tech_state.active_charger(), Some(valid_charger_entity));
    assert_eq!(tech_state.queue_len(), 0);
}

#[test]
fn test_technician_travel_abort_starts_next_queued_job() {
    let mut app = create_test_app();

    app.init_resource::<TechnicianState>();

    let site_id = SiteId(1);
    {
        let mut multi_site = app.world_mut().resource_mut::<MultiSiteManager>();
        let site_state = kilowatt_tycoon::resources::SiteState::new(
            site_id,
            SiteArchetype::ParkingLot,
            "Test Site".to_string(),
            500.0,
            50.0,
            1,
            (16, 12),
        );
        multi_site.owned_sites.insert(site_id, site_state);
        multi_site.viewed_site_id = Some(site_id);
    }

    let mut queued_charger = create_test_charger("CHG-QUEUED", ChargerType::DcFast);
    queued_charger.current_fault = Some(FaultType::GroundFault);
    queued_charger.grid_position = Some((5, 5));

    let queued_charger_entity = app
        .world_mut()
        .spawn((
            queued_charger,
            Transform::default(),
            Visibility::default(),
            BelongsToSite::new(site_id),
        ))
        .id();

    let missing_target = app.world_mut().spawn_empty().id();
    app.world_mut().entity_mut(missing_target).despawn();
    let missing_request_id = create_repair_request(
        &mut app,
        missing_target,
        "CHG-MISSING",
        site_id,
        FaultType::GroundFault,
    );
    let queued_request_id = create_repair_request(
        &mut app,
        queued_charger_entity,
        "CHG-QUEUED",
        site_id,
        FaultType::GroundFault,
    );

    {
        set_technician_en_route(
            &mut app,
            missing_request_id,
            missing_target,
            site_id,
            0.0,
            0.0,
        );
        let mut tech_state = app.world_mut().resource_mut::<TechnicianState>();
        tech_state.dispatch_queue.push_back(QueuedDispatch {
            request_id: queued_request_id,
            charger_entity: queued_charger_entity,
            charger_id: "CHG-QUEUED".to_string(),
            site_id,
        });
    }

    {
        let mut game_clock = app.world_mut().resource_mut::<GameClock>();
        game_clock.set_speed(GameSpeed::Normal);
    }

    app.add_systems(Update, technician_travel_system);
    app.update();

    let tech_state = app.world().resource::<TechnicianState>();
    assert_eq!(
        tech_state.status(),
        TechStatus::EnRoute,
        "Travel abort should immediately advance to the next queued dispatch"
    );
    assert_eq!(
        tech_state.active_charger(),
        Some(queued_charger_entity),
        "Queued charger should become the active dispatch target"
    );
    assert_eq!(
        tech_state.queue_len(),
        0,
        "Next queued dispatch should be consumed when the technician recovers"
    );
}

#[test]
fn test_same_site_chain_redirects_to_walking_not_immediate_repair() {
    let mut app = create_test_app();

    app.init_resource::<TechnicianState>();
    app.init_resource::<Time>();
    app.init_resource::<Assets<Image>>();
    app.init_resource::<kilowatt_tycoon::resources::ImageAssets>();
    app.add_message::<kilowatt_tycoon::events::ChargerFaultResolvedEvent>();

    let site_id = SiteId(1);
    {
        let mut multi_site = app.world_mut().resource_mut::<MultiSiteManager>();
        let site_state = kilowatt_tycoon::resources::SiteState::new(
            site_id,
            SiteArchetype::ParkingLot,
            "Test Site".to_string(),
            500.0,
            50.0,
            1,
            (16, 12),
        );
        multi_site.owned_sites.insert(site_id, site_state);
        multi_site.viewed_site_id = Some(site_id);
    }

    let mut current_charger = create_test_charger("CHG-CURRENT", ChargerType::DcFast);
    current_charger.current_fault = Some(FaultType::GroundFault);
    current_charger.grid_position = Some((5, 5));
    let current_charger_entity = app
        .world_mut()
        .spawn((
            current_charger,
            Transform::default(),
            GlobalTransform::default(),
            Visibility::default(),
            BelongsToSite::new(site_id),
        ))
        .id();

    let mut next_charger = create_test_charger("CHG-NEXT", ChargerType::DcFast);
    next_charger.current_fault = Some(FaultType::GroundFault);
    next_charger.grid_position = Some((7, 5));
    let next_charger_entity = app
        .world_mut()
        .spawn((
            next_charger,
            Transform::default(),
            GlobalTransform::default(),
            Visibility::default(),
            BelongsToSite::new(site_id),
        ))
        .id();

    let current_request_id = create_repair_request(
        &mut app,
        current_charger_entity,
        "CHG-CURRENT",
        site_id,
        FaultType::GroundFault,
    );
    let next_request_id = create_repair_request(
        &mut app,
        next_charger_entity,
        "CHG-NEXT",
        site_id,
        FaultType::GroundFault,
    );

    {
        let mut requests = app.world_mut().resource_mut::<RepairRequestRegistry>();
        assert!(requests.set_status(current_request_id, RepairRequestStatus::Repairing));
        assert!(requests.set_status(next_request_id, RepairRequestStatus::Queued));
    }

    {
        set_technician_repairing(
            &mut app,
            current_request_id,
            current_charger_entity,
            site_id,
            0.0,
        );
        let mut tech_state = app.world_mut().resource_mut::<TechnicianState>();
        tech_state.job_time_elapsed = 900.0;
        tech_state.dispatch_queue.push_back(QueuedDispatch {
            request_id: next_request_id,
            charger_entity: next_charger_entity,
            charger_id: "CHG-NEXT".to_string(),
            site_id,
        });
    }

    app.world_mut().spawn((
        Technician {
            target_charger: current_charger_entity,
            phase: TechnicianPhase::Working,
            work_timer: 0.0,
            target_bay: Some((5, 5)),
        },
        TechnicianMovement {
            phase: TechnicianPhase::Working,
            speed: 60.0,
        },
        TechnicianEmotion::default(),
        BelongsToSite::new(site_id),
        bevy_northstar::prelude::AgentPos(bevy::prelude::UVec3::new(5, 5, 0)),
    ));

    app.add_systems(Update, technician_repair_system);
    app.update();

    let tech_state = app.world().resource::<TechnicianState>();
    assert_eq!(tech_state.status(), TechStatus::WalkingOnSite);
    assert_eq!(tech_state.active_charger(), Some(next_charger_entity));

    let requests = app.world().resource::<RepairRequestRegistry>();
    assert_eq!(
        requests.get(next_request_id).map(|request| request.status),
        Some(RepairRequestStatus::WalkingOnSite)
    );

    let world = app.world_mut();
    let mut tech_query = world.query::<(&Technician, &TechnicianMovement, &BelongsToSite)>();
    let matches: Vec<_> = tech_query
        .iter(world)
        .filter(|(_, _, belongs)| belongs.site_id == site_id)
        .collect();

    assert_eq!(matches.len(), 1);
    assert_eq!(matches[0].0.target_charger, next_charger_entity);
    assert_eq!(matches[0].0.phase, TechnicianPhase::WalkingToCharger);
    assert_eq!(matches[0].1.phase, TechnicianPhase::WalkingToCharger);
}

#[test]
fn test_same_site_chain_fallback_keeps_dispatch_queued() {
    let mut app = create_test_app();

    app.init_resource::<TechnicianState>();
    app.init_resource::<Time>();
    app.init_resource::<Assets<Image>>();
    app.init_resource::<kilowatt_tycoon::resources::ImageAssets>();
    app.add_message::<kilowatt_tycoon::events::ChargerFaultResolvedEvent>();

    let site_id = SiteId(1);
    {
        let mut multi_site = app.world_mut().resource_mut::<MultiSiteManager>();
        let site_state = kilowatt_tycoon::resources::SiteState::new(
            site_id,
            SiteArchetype::ParkingLot,
            "Test Site".to_string(),
            500.0,
            50.0,
            1,
            (16, 12),
        );
        multi_site.owned_sites.insert(site_id, site_state);
        multi_site.viewed_site_id = Some(site_id);
    }

    let mut current_charger = create_test_charger("CHG-CURRENT", ChargerType::DcFast);
    current_charger.current_fault = Some(FaultType::GroundFault);
    current_charger.grid_position = Some((5, 5));
    let current_charger_entity = app
        .world_mut()
        .spawn((
            current_charger,
            Transform::default(),
            GlobalTransform::default(),
            Visibility::default(),
            BelongsToSite::new(site_id),
        ))
        .id();

    let mut next_charger = create_test_charger("CHG-NEXT", ChargerType::DcFast);
    next_charger.current_fault = Some(FaultType::GroundFault);
    next_charger.grid_position = None;
    let next_charger_entity = app
        .world_mut()
        .spawn((
            next_charger,
            Transform::default(),
            GlobalTransform::default(),
            Visibility::default(),
            BelongsToSite::new(site_id),
        ))
        .id();

    app.world_mut().spawn((
        Technician {
            target_charger: current_charger_entity,
            phase: TechnicianPhase::Working,
            work_timer: 0.0,
            target_bay: Some((5, 5)),
        },
        TechnicianMovement {
            phase: TechnicianPhase::Working,
            speed: 60.0,
        },
        TechnicianEmotion::default(),
        BelongsToSite::new(site_id),
    ));
    let current_request_id = create_repair_request(
        &mut app,
        current_charger_entity,
        "CHG-CURRENT",
        site_id,
        FaultType::GroundFault,
    );
    let next_request_id = create_repair_request(
        &mut app,
        next_charger_entity,
        "CHG-NEXT",
        site_id,
        FaultType::GroundFault,
    );

    {
        let mut game_clock = app.world_mut().resource_mut::<GameClock>();
        game_clock.set_speed(GameSpeed::Normal);
    }

    {
        set_technician_repairing(
            &mut app,
            current_request_id,
            current_charger_entity,
            site_id,
            0.0,
        );
        let mut tech_state = app.world_mut().resource_mut::<TechnicianState>();
        tech_state.job_time_elapsed = 900.0;
        tech_state.dispatch_queue.push_back(QueuedDispatch {
            request_id: next_request_id,
            charger_entity: next_charger_entity,
            charger_id: "CHG-NEXT".to_string(),
            site_id,
        });
    }

    // Remove the site state to force the chain logic down its "cannot route on-site"
    // fallback path while keeping the queued dispatch valid in ECS.
    {
        let mut multi_site = app.world_mut().resource_mut::<MultiSiteManager>();
        multi_site.owned_sites.remove(&site_id);
        multi_site.viewed_site_id = None;
    }

    app.add_systems(Update, technician_repair_system);
    app.update();

    let tech_state = app.world().resource::<TechnicianState>();
    assert!(
        tech_state
            .dispatch_queue
            .iter()
            .any(|queued| queued.charger_entity == next_charger_entity),
        "If same-site chaining cannot route the next charger, the queued dispatch should remain pending"
    );
}

#[test]
fn test_reconcile_creates_repair_request_for_hardware_fault() {
    let mut app = create_test_app();

    let site_id = SiteId(1);
    {
        let mut multi_site = app.world_mut().resource_mut::<MultiSiteManager>();
        let site_state = kilowatt_tycoon::resources::SiteState::new(
            site_id,
            SiteArchetype::ParkingLot,
            "Test Site".to_string(),
            500.0,
            50.0,
            1,
            (16, 12),
        );
        multi_site.owned_sites.insert(site_id, site_state);
        multi_site.viewed_site_id = Some(site_id);
    }

    let mut charger = create_test_charger("CHG-REQ", ChargerType::DcFast);
    charger.current_fault = Some(FaultType::GroundFault);
    charger.fault_occurred_at = Some(42.0);
    charger.fault_is_detected = false;
    charger.fault_discovered = false;
    let charger_entity = app
        .world_mut()
        .spawn((
            charger,
            Transform::default(),
            Visibility::default(),
            BelongsToSite::new(site_id),
        ))
        .id();

    app.add_systems(Update, reconcile_repair_requests_system);
    app.update();

    let requests = app.world().resource::<RepairRequestRegistry>();
    let request = requests
        .active_for_charger(charger_entity)
        .expect("technician-required fault should create a durable repair request");
    assert_eq!(request.charger_id, "CHG-REQ");
    assert_eq!(request.site_id, site_id);
    assert_eq!(request.status, RepairRequestStatus::OpenUndiscovered);
}

#[test]
fn test_driver_discovery_reuses_existing_repair_request() {
    let mut app = create_test_app();
    app.init_resource::<TechnicianState>();
    app.init_resource::<ChargerIndex>();

    let site_id = SiteId(1);
    {
        let mut multi_site = app.world_mut().resource_mut::<MultiSiteManager>();
        let site_state = kilowatt_tycoon::resources::SiteState::new(
            site_id,
            SiteArchetype::ParkingLot,
            "Test Site".to_string(),
            500.0,
            50.0,
            1,
            (16, 12),
        );
        multi_site.owned_sites.insert(site_id, site_state);
        multi_site.viewed_site_id = Some(site_id);
    }

    let mut charger = create_test_charger("CHG-DISCOVER", ChargerType::DcFast);
    charger.current_fault = Some(FaultType::GroundFault);
    charger.fault_discovered = false;
    charger.fault_is_detected = false;
    let charger_entity = app
        .world_mut()
        .spawn((
            charger,
            Transform::default(),
            Visibility::default(),
            BelongsToSite::new(site_id),
        ))
        .id();

    {
        let mut index = app.world_mut().resource_mut::<ChargerIndex>();
        index
            .by_id
            .insert("CHG-DISCOVER".to_string(), charger_entity);
    }

    let driver = Driver {
        state: DriverState::Arriving,
        assigned_charger: Some(charger_entity),
        ..create_test_driver("DRV-DISCOVER")
    };
    app.world_mut().spawn((
        driver,
        VehicleMovement {
            phase: MovementPhase::Parked,
            ..Default::default()
        },
        Transform::default(),
        Visibility::default(),
        BelongsToSite::new(site_id),
    ));

    app.add_systems(
        Update,
        (
            reconcile_repair_requests_system,
            driver_arrival_system,
            reconcile_repair_requests_system,
        )
            .chain(),
    );
    app.update();

    let requests = app.world().resource::<RepairRequestRegistry>();
    let request = requests
        .active_for_charger(charger_entity)
        .expect("repair request should still exist after driver discovery");
    assert!(request.discovered_at.is_some());
    assert_eq!(request.status, RepairRequestStatus::OpenDiscovered);
    assert_eq!(
        requests
            .iter()
            .filter(|request| request.charger_entity == charger_entity && request.status.is_open())
            .count(),
        1,
        "driver discovery should not create a duplicate repair request"
    );
}

#[test]
fn test_cleanup_sold_charger_cancels_active_repair_request() {
    let mut app = create_test_app();
    app.init_resource::<TechnicianState>();
    app.init_resource::<kilowatt_tycoon::resources::SelectedChargerEntity>();

    let site_id = SiteId(1);
    {
        let mut multi_site = app.world_mut().resource_mut::<MultiSiteManager>();
        let site_state = kilowatt_tycoon::resources::SiteState::new(
            site_id,
            SiteArchetype::ParkingLot,
            "Test Site".to_string(),
            500.0,
            50.0,
            1,
            (16, 12),
        );
        multi_site.owned_sites.insert(site_id, site_state);
        multi_site.viewed_site_id = Some(site_id);
    }

    let mut charger = create_test_charger("CHG-SOLD", ChargerType::DcFast);
    charger.current_fault = Some(FaultType::GroundFault);
    let charger_entity = app
        .world_mut()
        .spawn((
            charger,
            Transform::default(),
            Visibility::default(),
            BelongsToSite::new(site_id),
        ))
        .id();
    let request_id = create_repair_request(
        &mut app,
        charger_entity,
        "CHG-SOLD",
        site_id,
        FaultType::GroundFault,
    );
    set_technician_repairing(&mut app, request_id, charger_entity, site_id, 10.0);

    app.world_mut().entity_mut(charger_entity).despawn();
    app.add_systems(Update, cleanup_sold_charger_references);
    app.update();

    let requests = app.world().resource::<RepairRequestRegistry>();
    let request = requests
        .get(request_id)
        .expect("repair request should remain queryable for audit");
    assert_eq!(request.status, RepairRequestStatus::Cancelled);
    assert_eq!(
        app.world().resource::<TechnicianState>().status(),
        TechStatus::Idle,
        "active technician job should be aborted when its charger is sold"
    );
}

#[test]
fn test_reboot_always_succeeds_regardless_of_roll() {
    let mut charger = create_test_charger("CHG-001", ChargerType::DcFast);
    charger.current_fault = Some(FaultType::CommunicationError);

    // Even the worst possible roll (0.99) should succeed for a reboot
    let result = try_execute_action(&mut charger, RemoteAction::SoftReboot, 0.99).unwrap();
    assert!(result.success);
    assert!(result.fault_resolved);
    assert!(charger.current_fault.is_none());
}

#[test]
fn test_hard_reboot_always_succeeds() {
    let mut charger = create_test_charger("CHG-001", ChargerType::DcFast);
    charger.current_fault = Some(FaultType::FirmwareFault);

    let result = try_execute_action(&mut charger, RemoteAction::HardReboot, 0.99).unwrap();
    assert!(result.success);
    assert!(result.fault_resolved);
    assert!(charger.current_fault.is_none());
}

#[test]
fn test_reboot_clears_software_fault_but_not_hardware() {
    let mut charger = create_test_charger("CHG-001", ChargerType::DcFast);
    charger.current_fault = Some(FaultType::GroundFault);

    // Reboot succeeds but cannot resolve a hardware fault
    let result = try_execute_action(&mut charger, RemoteAction::SoftReboot, 0.5).unwrap();
    assert!(result.success);
    assert!(!result.fault_resolved);
    assert!(charger.current_fault.is_some());
}
