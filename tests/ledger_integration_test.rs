//! Integration tests for the double-entry accounting ledger.
//!
//! Verifies that all GameState financial operations produce balanced
//! transactions and that the ledger's cash balance matches GameState.cash.

use kilowatt_tycoon::resources::game_state::{GameState, STARTING_CASH};

#[test]
fn test_all_game_state_methods_record_to_ledger() {
    let mut gs = GameState::default();

    gs.add_charging_revenue(500.0);
    gs.add_ad_revenue(48.0);
    gs.add_carbon_credit_revenue(25.0);
    gs.add_solar_export_revenue(12.0);
    gs.add_energy_cost(200.0);
    gs.add_demand_charge(50.0);
    gs.add_opex(30.0);
    gs.add_cable_theft_cost(100.0);
    gs.add_warranty_cost(15.0);
    gs.add_refund(20.0);
    gs.add_penalty(10.0);
    gs.spend_rent(5000.0);
    gs.spend_upgrade(2000.0);

    assert!(
        gs.ledger.is_balanced(),
        "Ledger should be balanced after all operations"
    );
    assert!(
        gs.ledger.verify_cash(gs.cash).is_ok(),
        "Ledger cash should match GameState.cash"
    );

    let expected_cash = STARTING_CASH + 500.0 + 48.0 + 25.0 + 12.0
        - 200.0
        - 50.0
        - 30.0
        - 100.0
        - 15.0
        - 20.0
        - 10.0
        - 5000.0
        - 2000.0;
    assert!(
        (gs.cash - expected_cash).abs() < 0.01,
        "Cash mismatch: got {}, expected {}",
        gs.cash,
        expected_cash
    );
}

#[test]
fn test_build_and_bulldoze_ledger_tracking() {
    let mut gs = GameState::default();

    assert!(gs.try_spend_build(50_000));
    assert!(gs.try_spend_build(10_000));
    gs.refund_build(50_000); // 50% refund = 25000

    assert!(gs.ledger.is_balanced());
    assert!(gs.ledger.verify_cash(gs.cash).is_ok());

    let expected = STARTING_CASH - 50_000.0 - 10_000.0 + 25_000.0;
    assert!((gs.cash - expected).abs() < 0.01);
}

#[test]
fn test_site_sale_refund_ledger() {
    let mut gs = GameState::default();
    gs.try_spend_build(100_000);
    gs.refund_site_sale(80_000.0);

    assert!(gs.ledger.is_balanced());
    assert!(gs.ledger.verify_cash(gs.cash).is_ok());
    assert!((gs.cash - (STARTING_CASH - 100_000.0 + 80_000.0)).abs() < 0.01);
}

#[test]
fn test_reset_clears_ledger() {
    let mut gs = GameState::default();
    gs.add_charging_revenue(999.0);
    gs.add_energy_cost(100.0);

    gs.reset();

    assert_eq!(gs.ledger.transaction_count(), 1); // Only opening balance
    assert!(gs.ledger.is_balanced());
    assert_eq!(gs.ledger.gross_revenue_f32(), 0.0);
    assert!(gs.ledger.verify_cash(gs.cash).is_ok());
}

#[test]
fn test_reset_with_cash_sets_correct_opening_balance() {
    let mut gs = GameState::default();
    gs.add_charging_revenue(500.0);

    gs.reset_with_cash(500_000.0);

    assert_eq!(gs.cash, 500_000.0);
    assert_eq!(gs.ledger.cash_balance_f32(), 500_000.0);
    assert_eq!(gs.ledger.transaction_count(), 1);
    assert!(gs.ledger.is_balanced());
}

#[test]
fn test_full_day_simulation_with_all_transaction_types() {
    let mut gs = GameState::default();

    // Build phase
    assert!(gs.try_spend_build(80_000)); // DCFC charger
    assert!(gs.try_spend_build(3_000)); // L2 charger
    gs.spend_rent(15_000.0);

    // Operations phase - multiple charging sessions
    for _ in 0..20 {
        gs.add_charging_revenue(58.23);
    }
    gs.add_ad_revenue(48.0);
    gs.add_solar_export_revenue(12.50);
    gs.add_carbon_credit_revenue(8.75);

    // Costs
    gs.add_energy_cost(776.21);
    gs.add_demand_charge(216.66);
    gs.add_opex(120.0);
    gs.add_cable_theft_cost(350.0);
    gs.add_warranty_cost(15.0);
    gs.add_refund(25.0);
    gs.add_penalty(50.0);

    // Bulldoze one charger
    gs.refund_build(3_000); // 50% refund

    assert!(
        gs.ledger.is_balanced(),
        "Ledger must be balanced after full day"
    );
    assert!(
        gs.ledger.verify_cash(gs.cash).is_ok(),
        "Ledger cash must match GameState.cash after full day"
    );

    // Verify we have more transactions than just the opening balance
    assert!(
        gs.ledger.transaction_count() > 25,
        "Expected many transactions, got {}",
        gs.ledger.transaction_count()
    );
}

#[test]
fn test_daily_totals_match_tracker_revenue() {
    let mut gs = GameState::default();

    gs.add_charging_revenue(100.0);
    gs.add_ad_revenue(20.0);
    gs.add_solar_export_revenue(15.0);
    gs.add_energy_cost(40.0);
    gs.add_opex(10.0);

    let financials = gs.ledger.daily_totals(gs.ledger.current_date);
    let tracker = &gs.daily_history.current_day;

    // Revenue fields are kept on the tracker for real-time total_revenue() display
    assert!(
        (financials.charging_revenue - tracker.charging_revenue).abs() < 0.01,
        "Charging revenue: ledger={}, tracker={}",
        financials.charging_revenue,
        tracker.charging_revenue
    );
    assert!(
        (financials.ad_revenue - tracker.ad_revenue).abs() < 0.01,
        "Ad revenue: ledger={}, tracker={}",
        financials.ad_revenue,
        tracker.ad_revenue
    );
    assert!(
        (financials.solar_export_revenue - tracker.solar_export_revenue).abs() < 0.01,
        "Solar export: ledger={}, tracker={}",
        financials.solar_export_revenue,
        tracker.solar_export_revenue
    );

    // Expense fields live only in the ledger (not on tracker)
    assert!((financials.energy_cost - 40.0).abs() < 0.01);
    assert!((financials.opex - 10.0).abs() < 0.01);
}

#[test]
fn test_net_profit_includes_rent_and_capex() {
    use kilowatt_tycoon::resources::game_state::DailyRecord;
    use kilowatt_tycoon::resources::multi_site::SiteId;

    let mut gs = GameState::default();
    gs.add_charging_revenue(1000.0);
    gs.add_energy_cost(300.0);
    gs.spend_rent(200.0);
    gs.spend_upgrade(100.0);

    let financials = gs.ledger.daily_totals(gs.ledger.current_date);
    let record = DailyRecord::from_ledger(&financials, 1, 1, 2026, SiteId(1), 5, 0, 1, 0);

    // net_profit = 1000 - 300 - 200 - 100 = 400
    assert!(
        (record.net_profit() - 400.0).abs() < 0.01,
        "Net profit should be 400, got {}",
        record.net_profit()
    );
}
