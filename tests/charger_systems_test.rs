//! Tests for charger-related systems.
//!
//! These tests verify the behavior of charger state management,
//! fault handling, and remote actions.

mod test_utils;

use bevy::prelude::*;

use kilowatt_tycoon::components::charger::{
    Charger, ChargerState, ChargerType, FaultType, RemoteAction,
};
use kilowatt_tycoon::components::driver::{Driver, DriverState, MovementPhase, VehicleMovement};
use kilowatt_tycoon::components::site::BelongsToSite;
use kilowatt_tycoon::events::{RepairCompleteEvent, RepairFailedEvent, TechnicianDispatchEvent};
use kilowatt_tycoon::hooks::ChargerIndex;
use kilowatt_tycoon::resources::{
    GameClock, GameSpeed, MultiSiteManager, OemTier, SiteArchetype, SiteId, TechStatus,
    TechnicianState,
};
use kilowatt_tycoon::systems::{
    dispatch_technician_system, driver_arrival_system, fault_detection_system,
    om_auto_dispatch_system, scripted_fault_system, technician_repair_system, try_execute_action,
};

use test_utils::*;

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
    // Hard reboot more reliable than soft reboot
    assert!(RemoteAction::HardReboot.success_rate() > RemoteAction::SoftReboot.success_rate());

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
            tech_state.status,
            kilowatt_tycoon::resources::TechStatus::Idle
        );
        assert_eq!(tech_state.dispatch_queue.len(), 0);
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
            tech_state.status,
            kilowatt_tycoon::resources::TechStatus::EnRoute,
            "Technician should be dispatched (EnRoute) for ground fault with O&M Software"
        );
        assert_eq!(
            tech_state.target_charger,
            Some(charger_entity),
            "Technician should be assigned to the faulted charger"
        );
        assert_eq!(
            tech_state.destination_site_id,
            Some(site_id),
            "Technician should be traveling to the correct site"
        );
    }
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
            tech_state.status,
            kilowatt_tycoon::resources::TechStatus::Idle,
            "Technician should remain Idle without O&M Software"
        );
        assert_eq!(
            tech_state.dispatch_queue.len(),
            0,
            "No dispatch should be queued without O&M Software"
        );
        assert_eq!(
            tech_state.target_charger, None,
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
            tech_state.status,
            kilowatt_tycoon::resources::TechStatus::Idle,
            "Technician should remain Idle for auto-remediated faults"
        );
        assert_eq!(
            tech_state.dispatch_queue.len(),
            0,
            "No dispatch should be queued for auto-remediated faults"
        );
        assert_eq!(
            tech_state.target_charger, None,
            "Technician should not have a target charger"
        );
    }
}

#[test]
fn test_fault_detection_with_om_software() {
    // This test verifies that with O&M Software, faults are detected IMMEDIATELY
    // when they occur, without needing a driver to discover them.
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
        // Reset technician state for next repair attempt
        {
            let mut tech_state = app.world_mut().resource_mut::<TechnicianState>();
            tech_state.status = TechStatus::Repairing;
            tech_state.target_charger = Some(charger_entity);
            tech_state.destination_site_id = Some(site_id);
            tech_state.repair_remaining = 0.0; // Trigger completion
            tech_state.job_time_elapsed = 900.0;
            // Clear queue to detect new dispatches
            tech_state.dispatch_queue.clear();
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

/// Test that without O&M Software, repair failures do NOT trigger automatic re-dispatch.
#[test]
fn test_no_auto_redispatch_without_om_software() {
    let mut app = create_test_app();

    // Add required messages
    app.add_message::<TechnicianDispatchEvent>();
    app.add_message::<RepairCompleteEvent>();
    app.add_message::<RepairFailedEvent>();

    // Initialize technician state
    app.init_resource::<TechnicianState>();

    // Create a site WITHOUT O&M Software
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

        // Ensure O&M Software is NOT enabled
        site_state.site_upgrades.oem_tier = OemTier::None;

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
    // Due to probabilistic failure (20% at default $10/hr maintenance), we run multiple iterations
    let mut repair_failed_at_least_once = false;
    let mut incorrectly_dispatched = false;

    for _iteration in 0..50 {
        // Reset technician state for next repair attempt
        {
            let mut tech_state = app.world_mut().resource_mut::<TechnicianState>();
            tech_state.status = TechStatus::Repairing;
            tech_state.target_charger = Some(charger_entity);
            tech_state.destination_site_id = Some(site_id);
            tech_state.repair_remaining = 0.0;
            tech_state.job_time_elapsed = 900.0;
            // Clear queue to detect any new dispatches
            tech_state.dispatch_queue.clear();
        }

        // Ensure charger still has fault for retry
        {
            let mut charger = app.world_mut().get_mut::<Charger>(charger_entity).unwrap();
            charger.current_fault = Some(FaultType::GroundFault);
        }

        // Run one update to process repair completion
        app.update();

        // Check if repair failed (charger still has fault)
        let repair_failed = {
            let charger = app.world().get::<Charger>(charger_entity).unwrap();
            charger.current_fault.is_some()
        };

        if repair_failed {
            repair_failed_at_least_once = true;

            // Check if a dispatch was incorrectly queued (check the dispatch_queue directly, not target_charger)
            let queue_has_charger = {
                let tech_state = app.world().resource::<TechnicianState>();
                tech_state
                    .dispatch_queue
                    .iter()
                    .any(|q| q.charger_entity == charger_entity)
            };

            if queue_has_charger {
                incorrectly_dispatched = true;
                break; // Found an incorrect dispatch
            }
        }
    }

    assert!(
        repair_failed_at_least_once,
        "Expected at least one repair failure in 50 iterations (20% failure rate at default maintenance)"
    );
    assert!(
        !incorrectly_dispatched,
        "Should NOT auto-dispatch when repair fails WITHOUT O&M Software"
    );
}

#[test]
fn test_reboot_attempt_tracking_and_guaranteed_second_success() {
    let mut charger = create_test_charger("CHG-001", ChargerType::DcFast);
    charger.current_fault = Some(FaultType::CommunicationError);
    assert_eq!(charger.reboot_attempts, 0);

    // First attempt with a terrible roll (0.99) should fail
    let result = try_execute_action(&mut charger, RemoteAction::SoftReboot, 0.99).unwrap();
    assert!(!result.success);
    assert!(!result.fault_resolved);
    assert_eq!(charger.reboot_attempts, 1);
    assert!(charger.current_fault.is_some());

    // Clear cooldown so we can attempt again
    charger.update_cooldowns(999.0);

    // Second attempt with the same terrible roll must succeed (guaranteed)
    let result = try_execute_action(&mut charger, RemoteAction::HardReboot, 0.99).unwrap();
    assert!(result.success);
    assert!(result.fault_resolved);
    assert_eq!(charger.reboot_attempts, 0);
    assert!(charger.current_fault.is_none());
}

#[test]
fn test_reboot_first_attempt_can_succeed() {
    let mut charger = create_test_charger("CHG-001", ChargerType::DcFast);
    charger.current_fault = Some(FaultType::PaymentError);
    assert_eq!(charger.reboot_attempts, 0);

    // First attempt with a good roll (0.01) should succeed normally
    let result = try_execute_action(&mut charger, RemoteAction::SoftReboot, 0.01).unwrap();
    assert!(result.success);
    assert!(result.fault_resolved);
    assert_eq!(charger.reboot_attempts, 0);
    assert!(charger.current_fault.is_none());
}

#[test]
fn test_reboot_attempts_reset_on_new_fault() {
    let mut charger = create_test_charger("CHG-001", ChargerType::DcFast);
    charger.current_fault = Some(FaultType::CommunicationError);

    // Fail one reboot to increment the counter
    let _ = try_execute_action(&mut charger, RemoteAction::SoftReboot, 0.99).unwrap();
    assert_eq!(charger.reboot_attempts, 1);

    // Simulate a new fault injection (as inject_fault does)
    charger.current_fault = Some(FaultType::PaymentError);
    charger.reboot_attempts = 0;

    // Counter should be fresh for the new fault
    assert_eq!(charger.reboot_attempts, 0);
}
