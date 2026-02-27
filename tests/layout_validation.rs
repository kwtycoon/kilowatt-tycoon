use kilowatt_tycoon::resources::{ChargerPadType, GRID_WIDTH, LotTemplate, SiteGrid, TileContent};

#[test]
fn site_grid_default_has_end_to_end_road_preserving_entry_exit() {
    let grid = SiteGrid::default();
    let (ex, ey) = grid.entry_pos;
    let (ox, oy) = grid.exit_pos;

    assert_eq!(ey, oy, "entry and exit should be on same road row");
    assert_eq!(ex, 0, "entry should be at left edge");
    assert_eq!(ox, GRID_WIDTH - 1, "exit should be at right edge");

    assert_eq!(grid.get_content(ex, ey), TileContent::Entry);
    assert_eq!(grid.get_content(ox, oy), TileContent::Exit);

    for x in 1..(GRID_WIDTH - 1) {
        assert_eq!(
            grid.get_content(x, ey),
            TileContent::Road,
            "expected continuous road tile at ({x},{ey})"
        );
    }
}

#[test]
fn templates_do_not_overwrite_entry_exit() {
    for template in [LotTemplate::Small, LotTemplate::Medium, LotTemplate::Large] {
        let grid = template.build_grid();
        let (ex, ey) = grid.entry_pos;
        let (ox, oy) = grid.exit_pos;
        assert_eq!(
            grid.get_content(ex, ey),
            TileContent::Entry,
            "template {template:?} overwrote entry tile"
        );
        assert_eq!(
            grid.get_content(ox, oy),
            TileContent::Exit,
            "template {template:?} overwrote exit tile"
        );
    }
}

#[test]
fn templates_have_charger_ready_bays_adjacent_to_road() {
    for template in [LotTemplate::Small, LotTemplate::Medium, LotTemplate::Large] {
        let grid = template.build_grid();
        let bays = grid.get_parking_bays();
        assert!(
            !bays.is_empty(),
            "template {template:?} should prebuild parking bays"
        );

        for (x, y) in bays {
            assert!(
                grid.has_adjacent_road(x, y),
                "parking bay ({x},{y}) in template {template:?} must have adjacent road"
            );
        }
    }
}

#[test]
fn validate_for_open_does_not_require_building_roads() {
    // This ensures the "Need road connected to entry" issue does not appear once
    // transformer + charger are placed (road should be prebuilt).
    // Note: Templates now come with a pre-placed transformer, so we only need to add a charger.
    let mut grid = LotTemplate::Small.build_grid();

    // Verify transformer is already placed by the template
    assert!(
        grid.has_transformer(),
        "template should have a pre-placed transformer"
    );

    // Place a charger on the first available bay.
    let (bay_x, bay_y) = grid
        .get_parking_bays()
        .into_iter()
        .next()
        .expect("expected at least one bay");
    grid.place_charger(bay_x, bay_y, ChargerPadType::L2)
        .expect("should place charger");

    let validation = grid.validate_for_open();
    assert!(
        validation
            .issues
            .iter()
            .all(|s| s != "Need road connected to entry"),
        "road should already be connected to entry; issues were: {:?}",
        validation.issues
    );
}
