mod test_utils;

use bevy::ecs::message::MessageWriter;
use bevy::prelude::*;

use kilowatt_tycoon::components::charger::{ChargerType, FaultType};
use kilowatt_tycoon::components::site::BelongsToSite;
use kilowatt_tycoon::components::{Technician, TechnicianMovement, TechnicianPhase};
use kilowatt_tycoon::events::SiteSoldEvent;
use kilowatt_tycoon::resources::{
    AchievementState, BuildState, CarbonCreditMarket, FleetContractManager, GameState, ImageAssets,
    MultiSiteManager, RepairRequestRegistry, RepairRequestSource, RepairRequestStatus,
    SiteArchetype, SiteId, SiteState, TechnicianState,
};
use kilowatt_tycoon::states::day_end::report::{DayEndReport, prepare_day_end_report};
use kilowatt_tycoon::systems::{
    cleanup_sold_site_technician_state, cleanup_technicians_on_day_end, dispatch_technician_system,
    recover_dispatchable_requests_system, sync_viewed_technician_avatar_system,
    technician_repair_system, technician_travel_system,
};

use test_utils::{
    create_repair_request, create_test_app, create_test_charger, set_technician_en_route,
    set_technician_repairing,
};

fn insert_test_site(
    app: &mut App,
    site_id: SiteId,
    archetype: SiteArchetype,
    viewed: bool,
) -> Entity {
    let root_entity = app.world_mut().spawn_empty().id();

    let mut site = SiteState::new(
        site_id,
        archetype,
        format!("Site {}", site_id.0),
        500.0,
        50.0,
        1,
        (16, 12),
    );
    site.root_entity = Some(root_entity);

    let mut multi_site = app.world_mut().resource_mut::<MultiSiteManager>();
    multi_site.owned_sites.insert(site_id, site);
    if viewed {
        multi_site.viewed_site_id = Some(site_id);
    }

    root_entity
}

#[test]
fn test_flush_site_costs_drains_meter_state_for_all_sites() {
    let mut game_state = GameState::default();
    let mut multi_site = MultiSiteManager::default();

    let mut site_a = SiteState::new(
        SiteId(1),
        SiteArchetype::ParkingLot,
        "A".to_string(),
        500.0,
        50.0,
        1,
        (16, 12),
    );
    site_a.utility_meter.total_energy_cost = 120.0;
    site_a.utility_meter.demand_charge = 45.0;
    site_a.utility_meter.total_export_revenue = 10.0;
    site_a.pending_maintenance = 7.0;

    let mut site_b = SiteState::new(
        SiteId(2),
        SiteArchetype::GasStation,
        "B".to_string(),
        500.0,
        50.0,
        1,
        (16, 12),
    );
    site_b.utility_meter.total_energy_cost = 80.0;
    site_b.utility_meter.demand_charge = 25.0;
    site_b.pending_amenity = 5.0;
    site_b.pending_warranty = 3.0;

    multi_site.owned_sites.insert(site_a.id, site_a);
    multi_site.owned_sites.insert(site_b.id, site_b);

    game_state.flush_site_costs(&mut multi_site.owned_sites);

    for site in multi_site.owned_sites.values() {
        assert_eq!(site.utility_meter.total_energy_cost, 0.0);
        assert_eq!(site.utility_meter.demand_charge, 0.0);
        assert_eq!(site.utility_meter.total_export_revenue, 0.0);
        assert_eq!(site.pending_maintenance, 0.0);
        assert_eq!(site.pending_amenity, 0.0);
        assert_eq!(site.pending_warranty, 0.0);
    }
}

#[test]
fn test_reset_all_sites_for_new_day_clears_offscreen_daily_state() {
    let mut multi_site = MultiSiteManager::default();

    let mut site_a = SiteState::new(
        SiteId(1),
        SiteArchetype::ParkingLot,
        "A".to_string(),
        500.0,
        50.0,
        1,
        (16, 12),
    );
    site_a.energy_delivered_kwh_today = 24.0;
    site_a.sessions_today = 3;
    site_a.driver_schedule.next_driver_index = 4;
    site_a.driver_schedule.next_event_index = 2;
    site_a.utility_meter.total_energy_cost = 20.0;
    let queue_driver_a = Entity::PLACEHOLDER;
    site_a.charger_queue.dcfc_queue.push_back(queue_driver_a);

    let mut site_b = SiteState::new(
        SiteId(2),
        SiteArchetype::GasStation,
        "B".to_string(),
        500.0,
        50.0,
        1,
        (16, 12),
    );
    site_b.energy_delivered_kwh_today = 40.0;
    site_b.sessions_today = 6;
    site_b.driver_schedule.next_driver_index = 8;
    site_b.driver_schedule.next_event_index = 5;
    site_b.utility_meter.total_energy_cost = 30.0;
    let queue_driver_b = Entity::PLACEHOLDER;
    site_b.charger_queue.dcfc_queue.push_back(queue_driver_b);

    multi_site.owned_sites.insert(site_a.id, site_a);
    multi_site.owned_sites.insert(site_b.id, site_b);
    multi_site.viewed_site_id = Some(SiteId(1));

    multi_site.reset_all_sites_for_new_day();

    for site in multi_site.owned_sites.values() {
        assert_eq!(site.energy_delivered_kwh_today, 0.0);
        assert_eq!(site.sessions_today, 0);
        assert_eq!(site.driver_schedule.next_driver_index, 0);
        assert_eq!(site.driver_schedule.next_event_index, 0);
        assert!(site.charger_queue.dcfc_queue.is_empty());
        assert_eq!(site.utility_meter.total_energy_cost, 0.0);
    }
}

#[test]
fn test_offscreen_travel_completion_waits_for_visible_execution() {
    let mut app = create_test_app();
    app.init_resource::<TechnicianState>();
    app.add_systems(Update, technician_travel_system);

    let site_a = SiteId(1);
    let site_b = SiteId(2);
    insert_test_site(&mut app, site_a, SiteArchetype::ParkingLot, true);
    insert_test_site(&mut app, site_b, SiteArchetype::GasStation, false);

    let mut charger = create_test_charger("CHG-OFFSCREEN", ChargerType::DcFast);
    charger.current_fault = Some(FaultType::GroundFault);
    let charger_entity = app
        .world_mut()
        .spawn((
            charger,
            Transform::default(),
            GlobalTransform::default(),
            Visibility::default(),
            BelongsToSite::new(site_b),
        ))
        .id();

    let request_id = create_repair_request(
        &mut app,
        charger_entity,
        "CHG-OFFSCREEN",
        site_b,
        FaultType::GroundFault,
    );
    {
        let mut tech_state = app.world_mut().resource_mut::<TechnicianState>();
        tech_state.current_site_id = Some(site_a);
    }
    set_technician_en_route(&mut app, request_id, charger_entity, site_b, 0.0, 120.0);

    app.update();

    let tech_state = app.world().resource::<TechnicianState>();
    assert_eq!(
        tech_state.status(),
        kilowatt_tycoon::resources::TechStatus::WaitingAtSite
    );
    assert_eq!(tech_state.destination_site_id(), Some(site_b));

    let requests = app.world().resource::<RepairRequestRegistry>();
    assert_eq!(
        requests.get(request_id).map(|request| request.status),
        Some(RepairRequestStatus::WaitingAtSite)
    );

    let world = app.world_mut();
    let mut tech_query = world.query::<&Technician>();
    assert_eq!(tech_query.iter(world).count(), 0);
}

#[test]
fn test_new_day_recovery_prefers_the_viewed_site_request() {
    let mut app = create_test_app();
    app.init_resource::<TechnicianState>();
    app.add_systems(
        Update,
        (
            recover_dispatchable_requests_system,
            dispatch_technician_system,
        )
            .chain(),
    );

    let site_a = SiteId(1);
    let site_b = SiteId(2);
    insert_test_site(&mut app, site_a, SiteArchetype::ParkingLot, false);
    insert_test_site(&mut app, site_b, SiteArchetype::GasStation, true);

    let mut charger_a = create_test_charger("CHG-A", ChargerType::DcFast);
    charger_a.current_fault = Some(FaultType::GroundFault);
    let charger_a_entity = app
        .world_mut()
        .spawn((
            charger_a,
            Transform::default(),
            GlobalTransform::default(),
            Visibility::default(),
            BelongsToSite::new(site_a),
        ))
        .id();

    let mut charger_b = create_test_charger("CHG-B", ChargerType::DcFast);
    charger_b.current_fault = Some(FaultType::GroundFault);
    let charger_b_entity = app
        .world_mut()
        .spawn((
            charger_b,
            Transform::default(),
            GlobalTransform::default(),
            Visibility::default(),
            BelongsToSite::new(site_b),
        ))
        .id();

    let request_a = create_repair_request(
        &mut app,
        charger_a_entity,
        "CHG-A",
        site_a,
        FaultType::GroundFault,
    );
    let request_b = create_repair_request(
        &mut app,
        charger_b_entity,
        "CHG-B",
        site_b,
        FaultType::GroundFault,
    );

    {
        let mut requests = app.world_mut().resource_mut::<RepairRequestRegistry>();
        let _ = requests.queue(request_a, 10.0, RepairRequestSource::OemDetection);
        let _ = requests.queue(request_b, 20.0, RepairRequestSource::OemDetection);
        let _ = requests.set_status(request_a, RepairRequestStatus::WaitingAtSite);
    }
    {
        let mut tech_state = app.world_mut().resource_mut::<TechnicianState>();
        tech_state.current_site_id = Some(site_a);
        tech_state.set_waiting_at_site_job(request_a, charger_a_entity, site_a, 120.0);
    }

    {
        let mut requests = app.world_mut().resource_mut::<RepairRequestRegistry>();
        requests.reset_for_new_day();
    }
    {
        let mut tech_state = app.world_mut().resource_mut::<TechnicianState>();
        tech_state.reset_for_new_day();
    }

    app.update();

    let tech_state = app.world().resource::<TechnicianState>();
    assert_eq!(
        tech_state.status(),
        kilowatt_tycoon::resources::TechStatus::EnRoute
    );
    assert_eq!(tech_state.active_request_id(), Some(request_b));
    assert_eq!(tech_state.destination_site_id(), Some(site_b));
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
fn test_en_route_job_aborts_when_fault_disappears_before_arrival() {
    let mut app = create_test_app();
    app.init_resource::<TechnicianState>();
    app.init_resource::<Time>();
    app.add_systems(Update, technician_travel_system);

    let site_id = SiteId(1);
    insert_test_site(&mut app, site_id, SiteArchetype::ParkingLot, true);

    let mut charger = create_test_charger("CHG-CLEARED", ChargerType::DcFast);
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
        "CHG-CLEARED",
        site_id,
        FaultType::GroundFault,
    );
    set_technician_en_route(&mut app, request_id, charger_entity, site_id, 30.0, 120.0);

    {
        let mut charger = app
            .world_mut()
            .get_mut::<kilowatt_tycoon::components::charger::Charger>(charger_entity)
            .unwrap();
        charger.current_fault = None;
    }

    app.update();

    let tech_state = app.world().resource::<TechnicianState>();
    assert_eq!(
        tech_state.status(),
        kilowatt_tycoon::resources::TechStatus::Idle
    );
    assert_eq!(tech_state.active_charger(), None);

    let requests = app.world().resource::<RepairRequestRegistry>();
    assert_eq!(
        requests.get(request_id).map(|request| request.status),
        Some(RepairRequestStatus::Resolved)
    );
}

#[derive(Resource, Default)]
struct PendingSaleEvent(Option<SiteSoldEvent>);

fn emit_pending_site_sale(
    mut pending: ResMut<PendingSaleEvent>,
    mut writer: MessageWriter<SiteSoldEvent>,
) {
    if let Some(event) = pending.0.take() {
        writer.write(event);
    }
}

#[test]
fn test_cleanup_sold_site_cancels_leaving_site_and_starts_next_job() {
    let mut app = create_test_app();
    app.init_resource::<TechnicianState>();
    app.init_resource::<PendingSaleEvent>();
    app.add_message::<SiteSoldEvent>();
    app.add_systems(
        Update,
        (emit_pending_site_sale, cleanup_sold_site_technician_state).chain(),
    );

    let sold_site = SiteId(1);
    let next_site = SiteId(2);
    insert_test_site(&mut app, sold_site, SiteArchetype::ParkingLot, true);
    insert_test_site(&mut app, next_site, SiteArchetype::GasStation, false);

    let mut next_charger = create_test_charger("CHG-NEXT", ChargerType::DcFast);
    next_charger.current_fault = Some(FaultType::GroundFault);
    let next_charger_entity = app
        .world_mut()
        .spawn((
            next_charger,
            Transform::default(),
            GlobalTransform::default(),
            Visibility::default(),
            BelongsToSite::new(next_site),
        ))
        .id();
    let next_request_id = create_repair_request(
        &mut app,
        next_charger_entity,
        "CHG-NEXT",
        next_site,
        FaultType::GroundFault,
    );

    let sold_request_id = {
        let sold_charger = create_test_charger("CHG-SOLD", ChargerType::DcFast);
        let sold_charger_entity = app
            .world_mut()
            .spawn((
                sold_charger,
                Transform::default(),
                GlobalTransform::default(),
                Visibility::default(),
                BelongsToSite::new(sold_site),
            ))
            .id();
        create_repair_request(
            &mut app,
            sold_charger_entity,
            "CHG-SOLD",
            sold_site,
            FaultType::GroundFault,
        )
    };

    {
        let mut tech_state = app.world_mut().resource_mut::<TechnicianState>();
        tech_state.current_site_id = Some(sold_site);
        tech_state.begin_leaving_site(sold_site);
        assert!(tech_state.queue_dispatch(
            next_request_id,
            next_charger_entity,
            "CHG-NEXT".to_string(),
            next_site,
        ));
    }

    app.world_mut().spawn((
        Technician {
            target_charger: Entity::PLACEHOLDER,
            phase: TechnicianPhase::WalkingToExit,
            work_timer: 0.0,
            target_bay: None,
        },
        TechnicianMovement {
            phase: TechnicianPhase::WalkingToExit,
            speed: 60.0,
        },
        BelongsToSite::new(sold_site),
    ));

    app.world_mut().resource_mut::<PendingSaleEvent>().0 = Some(SiteSoldEvent {
        site_id: sold_site,
        refund_amount: 0.0,
    });
    app.update();

    let tech_state = app.world().resource::<TechnicianState>();
    assert_eq!(
        tech_state.status(),
        kilowatt_tycoon::resources::TechStatus::EnRoute
    );
    assert_eq!(tech_state.destination_site_id(), Some(next_site));
    assert_eq!(tech_state.current_site_id, None);

    let requests = app.world().resource::<RepairRequestRegistry>();
    assert_eq!(
        requests.get(sold_request_id).map(|request| request.status),
        Some(RepairRequestStatus::Cancelled)
    );

    let world = app.world_mut();
    let mut tech_query = world.query_filtered::<&BelongsToSite, With<Technician>>();
    assert_eq!(
        tech_query
            .iter(world)
            .filter(|belongs| belongs.site_id == sold_site)
            .count(),
        0
    );
}

#[test]
fn test_day_end_cleanup_resets_repairing_technician_but_preserves_request() {
    let mut app = create_test_app();
    app.init_resource::<TechnicianState>();
    app.add_systems(Update, cleanup_technicians_on_day_end);

    let site_id = SiteId(1);
    insert_test_site(&mut app, site_id, SiteArchetype::ParkingLot, true);

    let mut charger = create_test_charger("CHG-DAYEND-REPAIR", ChargerType::DcFast);
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
        "CHG-DAYEND-REPAIR",
        site_id,
        FaultType::GroundFault,
    );
    {
        let mut requests = app.world_mut().resource_mut::<RepairRequestRegistry>();
        assert!(requests.queue(request_id, 5.0, RepairRequestSource::ManualDispatch));
        assert!(requests.set_status(request_id, RepairRequestStatus::Repairing));
        assert!(requests.mark_dispatch_costs_recorded(request_id));
    }
    {
        let mut tech_state = app.world_mut().resource_mut::<TechnicianState>();
        tech_state.current_site_id = Some(site_id);
        tech_state.set_same_site_job(request_id, charger_entity, site_id, 45.0);
        assert!(tech_state.begin_repairing());
    }

    app.world_mut().spawn((
        Technician {
            target_charger: charger_entity,
            phase: TechnicianPhase::Working,
            work_timer: 0.0,
            target_bay: Some((2, 3)),
        },
        TechnicianMovement {
            phase: TechnicianPhase::Working,
            speed: 60.0,
        },
        BelongsToSite::new(site_id),
    ));

    app.update();

    let tech_state = app.world().resource::<TechnicianState>();
    assert_eq!(
        tech_state.status(),
        kilowatt_tycoon::resources::TechStatus::Idle
    );
    assert_eq!(tech_state.current_site_id, None);
    assert_eq!(tech_state.queue_len(), 0);

    {
        let world = app.world_mut();
        let mut tech_query = world.query::<&Technician>();
        assert_eq!(tech_query.iter(world).count(), 0);
    }

    {
        let requests = app.world().resource::<RepairRequestRegistry>();
        let request = requests.get(request_id).expect("request should persist");
        assert_eq!(request.status, RepairRequestStatus::OpenDiscovered);
        assert!(request.dispatch_costs_recorded);
    }

    let mut requests = app.world_mut().resource_mut::<RepairRequestRegistry>();
    assert!(requests.queue(request_id, 10.0, RepairRequestSource::Retry));
}

#[test]
fn test_day_end_cleanup_resets_leaving_technician_and_preserves_queued_dispatch() {
    let mut app = create_test_app();
    app.init_resource::<TechnicianState>();
    app.add_systems(Update, cleanup_technicians_on_day_end);

    let site_id = SiteId(1);
    insert_test_site(&mut app, site_id, SiteArchetype::ParkingLot, true);

    let mut charger = create_test_charger("CHG-DAYEND-QUEUE", ChargerType::DcFast);
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
        "CHG-DAYEND-QUEUE",
        site_id,
        FaultType::GroundFault,
    );
    {
        let mut requests = app.world_mut().resource_mut::<RepairRequestRegistry>();
        assert!(requests.queue(request_id, 5.0, RepairRequestSource::ManualDispatch));
    }
    {
        let mut tech_state = app.world_mut().resource_mut::<TechnicianState>();
        tech_state.current_site_id = Some(site_id);
        tech_state.begin_leaving_site(site_id);
        assert!(tech_state.queue_dispatch(
            request_id,
            charger_entity,
            "CHG-DAYEND-QUEUE".to_string(),
            site_id,
        ));
    }

    app.world_mut().spawn((
        Technician {
            target_charger: Entity::PLACEHOLDER,
            phase: TechnicianPhase::WalkingToExit,
            work_timer: 0.0,
            target_bay: None,
        },
        TechnicianMovement {
            phase: TechnicianPhase::WalkingToExit,
            speed: 60.0,
        },
        BelongsToSite::new(site_id),
    ));

    app.update();

    let tech_state = app.world().resource::<TechnicianState>();
    assert_eq!(
        tech_state.status(),
        kilowatt_tycoon::resources::TechStatus::Idle
    );
    assert_eq!(tech_state.current_site_id, None);
    assert_eq!(tech_state.queue_len(), 0);

    {
        let world = app.world_mut();
        let mut tech_query = world.query::<&Technician>();
        assert_eq!(tech_query.iter(world).count(), 0);
    }

    {
        let requests = app.world().resource::<RepairRequestRegistry>();
        let request = requests
            .get(request_id)
            .expect("queued request should persist");
        assert_eq!(request.status, RepairRequestStatus::OpenDiscovered);
        assert!(!request.dispatch_costs_recorded);
    }

    let mut requests = app.world_mut().resource_mut::<RepairRequestRegistry>();
    assert!(requests.queue(request_id, 10.0, RepairRequestSource::Retry));
}

#[test]
fn test_switching_to_viewed_waiting_site_spawns_walking_avatar() {
    let mut app = create_test_app();
    app.init_resource::<TechnicianState>();
    app.init_resource::<BuildState>();
    app.add_systems(
        Update,
        (
            technician_travel_system,
            sync_viewed_technician_avatar_system,
        ),
    );

    let viewed_site = SiteId(1);
    let repair_site = SiteId(2);
    let _ = insert_test_site(&mut app, viewed_site, SiteArchetype::ParkingLot, true);
    let _root_entity = insert_test_site(&mut app, repair_site, SiteArchetype::GasStation, false);

    let mut charger = create_test_charger("CHG-VIEWED", ChargerType::DcFast);
    charger.current_fault = Some(FaultType::GroundFault);
    charger.grid_position = Some((2, 3));
    let charger_entity = app
        .world_mut()
        .spawn((
            charger,
            Transform::default(),
            Visibility::default(),
            BelongsToSite::new(repair_site),
        ))
        .id();

    let request_id = create_repair_request(
        &mut app,
        charger_entity,
        "CHG-VIEWED",
        repair_site,
        FaultType::GroundFault,
    );
    {
        let mut tech_state = app.world_mut().resource_mut::<TechnicianState>();
        tech_state.current_site_id = Some(viewed_site);
    }
    set_technician_en_route(&mut app, request_id, charger_entity, repair_site, 0.0, 45.0);

    app.update();

    {
        let mut multi_site = app.world_mut().resource_mut::<MultiSiteManager>();
        multi_site.viewed_site_id = Some(repair_site);
    }

    app.update();

    let tech_state = app.world().resource::<TechnicianState>();
    assert_eq!(
        tech_state.status(),
        kilowatt_tycoon::resources::TechStatus::WalkingOnSite
    );

    let requests = app.world().resource::<RepairRequestRegistry>();
    assert_eq!(
        requests.get(request_id).map(|request| request.status),
        Some(RepairRequestStatus::WalkingOnSite)
    );

    let world = app.world_mut();
    let mut tech_query = world.query::<(&Technician, &TechnicianMovement, &BelongsToSite)>();
    let matches: Vec<_> = tech_query
        .iter(world)
        .filter(|(_, _, belongs)| belongs.site_id == repair_site)
        .collect();

    assert_eq!(matches.len(), 1);
    assert_eq!(matches[0].0.phase, TechnicianPhase::WalkingToCharger);
    assert_eq!(matches[0].1.phase, TechnicianPhase::WalkingToCharger);
}

#[test]
fn test_visible_arrival_stays_repairing_and_progresses_next_frame() {
    let mut app = create_test_app();
    app.init_resource::<TechnicianState>();
    app.init_resource::<BuildState>();
    app.init_resource::<Time>();
    app.init_resource::<Assets<Image>>();
    app.init_resource::<ImageAssets>();
    app.add_message::<kilowatt_tycoon::events::ChargerFaultResolvedEvent>();
    app.add_systems(
        Update,
        (
            sync_viewed_technician_avatar_system,
            kilowatt_tycoon::systems::technician_arrival_detection,
            technician_repair_system,
        ),
    );
    {
        let mut build_state = app.world_mut().resource_mut::<BuildState>();
        build_state.is_open = true;
    }

    let site_id = SiteId(1);
    insert_test_site(&mut app, site_id, SiteArchetype::ParkingLot, true);

    let mut charger = create_test_charger("CHG-STALL", ChargerType::DcFast);
    charger.current_fault = Some(FaultType::GroundFault);
    charger.grid_position = Some((2, 3));
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
        "CHG-STALL",
        site_id,
        FaultType::GroundFault,
    );
    {
        let mut requests = app.world_mut().resource_mut::<RepairRequestRegistry>();
        assert!(requests.set_status(request_id, RepairRequestStatus::WalkingOnSite));
    }
    {
        let mut tech_state = app.world_mut().resource_mut::<TechnicianState>();
        tech_state.set_same_site_job(request_id, charger_entity, site_id, 10.0);
    }

    app.world_mut().spawn((
        Technician {
            target_charger: charger_entity,
            phase: TechnicianPhase::WalkingToCharger,
            work_timer: 0.0,
            target_bay: Some((2, 3)),
        },
        TechnicianMovement {
            phase: TechnicianPhase::WalkingToCharger,
            speed: 60.0,
        },
        kilowatt_tycoon::components::emotion::TechnicianEmotion::default(),
        BelongsToSite::new(site_id),
        bevy_northstar::prelude::AgentPos(bevy::prelude::UVec3::new(2, 3, 0)),
    ));

    app.update();

    {
        let tech_state = app.world().resource::<TechnicianState>();
        assert_eq!(
            tech_state.status(),
            kilowatt_tycoon::resources::TechStatus::Repairing
        );
        assert_eq!(tech_state.repair_remaining(), Some(10.0));
    }
    {
        let requests = app.world().resource::<RepairRequestRegistry>();
        assert_eq!(
            requests.get(request_id).map(|request| request.status),
            Some(RepairRequestStatus::Repairing)
        );
    }

    app.update();

    let tech_state = app.world().resource::<TechnicianState>();
    assert_eq!(
        tech_state.status(),
        kilowatt_tycoon::resources::TechStatus::Repairing
    );
    assert!(tech_state.repair_remaining().unwrap_or_default() < 10.0);

    let requests = app.world().resource::<RepairRequestRegistry>();
    assert_eq!(
        requests.get(request_id).map(|request| request.status),
        Some(RepairRequestStatus::Repairing)
    );

    let world = app.world_mut();
    let mut tech_query = world.query::<(&Technician, &TechnicianMovement, &BelongsToSite)>();
    let matches: Vec<_> = tech_query
        .iter(world)
        .filter(|(_, _, belongs)| belongs.site_id == site_id)
        .collect();
    assert_eq!(matches.len(), 1);
    assert_eq!(matches[0].0.phase, TechnicianPhase::Working);
    assert_eq!(matches[0].1.phase, TechnicianPhase::Working);
}

#[test]
fn test_repair_does_not_progress_without_visible_technician_at_charger() {
    let mut app = create_test_app();
    app.init_resource::<TechnicianState>();
    app.init_resource::<Time>();
    app.init_resource::<Assets<Image>>();
    app.init_resource::<ImageAssets>();
    app.add_message::<kilowatt_tycoon::events::ChargerFaultResolvedEvent>();
    app.add_systems(Update, technician_repair_system);

    let site_id = SiteId(1);
    insert_test_site(&mut app, site_id, SiteArchetype::ParkingLot, true);

    let mut charger = create_test_charger("CHG-GUARD", ChargerType::DcFast);
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
        "CHG-GUARD",
        site_id,
        FaultType::GroundFault,
    );
    set_technician_repairing(&mut app, request_id, charger_entity, site_id, 0.0);
    {
        let mut requests = app.world_mut().resource_mut::<RepairRequestRegistry>();
        assert!(requests.set_status(request_id, RepairRequestStatus::Repairing));
    }
    {
        let mut tech_state = app.world_mut().resource_mut::<TechnicianState>();
        tech_state.job_time_elapsed = 900.0;
    }

    app.update();

    let tech_state = app.world().resource::<TechnicianState>();
    assert_eq!(
        tech_state.status(),
        kilowatt_tycoon::resources::TechStatus::Repairing
    );

    let requests = app.world().resource::<RepairRequestRegistry>();
    assert_eq!(
        requests.get(request_id).map(|request| request.status),
        Some(RepairRequestStatus::Repairing)
    );

    let charger = app
        .world()
        .get::<kilowatt_tycoon::components::charger::Charger>(charger_entity)
        .unwrap();
    assert_eq!(charger.total_repair_opex, 0.0);
    assert_eq!(charger.current_fault, Some(FaultType::GroundFault));
}

#[test]
fn test_repairing_job_aborts_without_billing_when_request_is_terminal() {
    let mut app = create_test_app();
    app.init_resource::<TechnicianState>();
    app.init_resource::<Time>();
    app.init_resource::<Assets<Image>>();
    app.init_resource::<ImageAssets>();
    app.add_message::<kilowatt_tycoon::events::ChargerFaultResolvedEvent>();
    app.add_systems(Update, technician_repair_system);

    let site_id = SiteId(1);
    insert_test_site(&mut app, site_id, SiteArchetype::ParkingLot, true);

    let mut charger = create_test_charger("CHG-TERM-REPAIR", ChargerType::DcFast);
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
        "CHG-TERM-REPAIR",
        site_id,
        FaultType::GroundFault,
    );
    set_technician_repairing(&mut app, request_id, charger_entity, site_id, 0.0);
    {
        let mut tech_state = app.world_mut().resource_mut::<TechnicianState>();
        tech_state.job_time_elapsed = 900.0;
    }
    {
        let mut requests = app.world_mut().resource_mut::<RepairRequestRegistry>();
        assert!(requests.resolve(
            request_id,
            5.0,
            kilowatt_tycoon::resources::RepairResolution::Cancelled
        ));
    }

    app.update();

    let tech_state = app.world().resource::<TechnicianState>();
    assert_eq!(
        tech_state.status(),
        kilowatt_tycoon::resources::TechStatus::Idle
    );

    let charger = app
        .world()
        .get::<kilowatt_tycoon::components::charger::Charger>(charger_entity)
        .unwrap();
    assert_eq!(charger.total_repair_opex, 0.0);
}

#[test]
fn test_day_end_report_pricing_uses_pre_flush_meter_snapshot() {
    let mut app = create_test_app();
    app.init_resource::<ImageAssets>();
    app.init_resource::<AchievementState>();
    app.init_resource::<CarbonCreditMarket>();
    app.init_resource::<FleetContractManager>();
    app.add_systems(Update, prepare_day_end_report);

    let site_id = SiteId(1);
    {
        let mut multi_site = app.world_mut().resource_mut::<MultiSiteManager>();
        let mut site = SiteState::new(
            site_id,
            SiteArchetype::ParkingLot,
            "A".to_string(),
            500.0,
            50.0,
            1,
            (16, 12),
        );
        site.utility_meter.off_peak_kwh = 20.0;
        site.utility_meter.total_energy_cost = 30.0;
        multi_site.owned_sites.insert(site_id, site);
        multi_site.viewed_site_id = Some(site_id);
    }
    {
        let mut game_state = app.world_mut().resource_mut::<GameState>();
        game_state.add_charging_revenue(100.0);
    }

    app.update();

    let report = app.world().resource::<DayEndReport>();
    assert!((report.avg_sell_price_kwh - 5.0).abs() < 0.001);
    assert!((report.avg_buy_price_kwh - 1.5).abs() < 0.001);

    let multi_site = app.world().resource::<MultiSiteManager>();
    let site = multi_site.owned_sites.get(&site_id).unwrap();
    assert_eq!(site.utility_meter.total_imported_kwh(), 0.0);
}
