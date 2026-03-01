//! Strategy panels: Summary, Pricing, Power, OPEX

use super::{ActivePanel, colors, panel::*};
use crate::resources::{GameState, ImageAssets};
use bevy::ecs::hierarchy::ChildSpawnerCommands;
use bevy::prelude::*;

// ============ Panel Markers ============

#[derive(Component)]
pub struct PricePanel;

#[derive(Component)]
pub struct PowerStrategyPanel;

#[derive(Component)]
pub struct RecipePanel;

#[derive(Component)]
pub struct SuppliesPanel;

#[derive(Component)]
pub struct SummaryPanel;

// Upgrades panel moved to build_panels.rs

// ============ Label Components ============

#[derive(Component)]
pub struct EnergyPriceLabel;

#[derive(Component)]
pub struct IdleFeeLabel;

#[derive(Component)]
pub struct VideoAdPriceLabel;

#[derive(Component)]
pub struct PowerDensityLabel;

#[derive(Component)]
pub struct MaintenanceLabel;

#[derive(Component)]
pub struct AmenityLevelLabel;

#[derive(Component)]
pub struct HourlyOpexLabel;

#[derive(Component)]
pub struct WarrantyTierLabel;

#[derive(Component)]
pub struct WarrantyPremiumLabel;

#[derive(Component)]
pub struct WarrantyCoverageLabel;

#[derive(Component)]
pub struct BessDischargeThresholdLabel;

#[derive(Component)]
pub struct BessChargeThresholdLabel;

#[derive(Component)]
pub struct SolarExportPolicyLabel;

#[derive(Component)]
pub struct SummaryCashLabel;

#[derive(Component)]
pub struct SummaryRevenueLabel;

#[derive(Component)]
pub struct SummaryRepLabel;

#[derive(Component)]
pub struct SummarySessionsLabel;

/// Container for session icons and numbers (rebuilt dynamically)
#[derive(Component)]
pub struct SessionsValueContainer;

#[derive(Component)]
pub struct SummaryUptimeLabel;

// ============ Dynamic Pricing Labels ============

#[derive(Component)]
pub struct PricingModeLabel;

#[derive(Component)]
pub struct TouOffPeakPriceLabel;

#[derive(Component)]
pub struct TouOnPeakPriceLabel;

#[derive(Component)]
pub struct CostPlusMarkupLabel;

#[derive(Component)]
pub struct CostPlusFloorLabel;

#[derive(Component)]
pub struct CostPlusCeilingLabel;

#[derive(Component)]
pub struct SurgeBasePriceLabel;

#[derive(Component)]
pub struct SurgeMultiplierLabel;

#[derive(Component)]
pub struct SurgeThresholdLabel;

#[derive(Component)]
pub struct EffectivePriceLabel;

// ============ Lock Overlay Markers ============

/// Marker for the power controls lock overlay (shown when Advanced Power Management not purchased)
#[derive(Component)]
pub struct PowerControlsLockOverlay;

/// Marker for the OPEX controls lock overlay (shown when no OEM tier purchased)
#[derive(Component)]
pub struct OpexControlsLockOverlay;

/// Marker for the dynamic pricing lock overlay (shown when Dynamic Pricing Engine not purchased)
#[derive(Component)]
pub struct DynamicPricingLockOverlay;

/// Marker for the effective-price indicator row (visible only when upgrade purchased)
#[derive(Component)]
pub struct EffectivePriceRow;

// UpgradeButton moved to build_panels.rs

// ============ Spawn Functions ============

/// Spawn all strategy panels
pub fn spawn_strategy_panels(parent: &mut ChildSpawnerCommands, image_assets: &ImageAssets) {
    spawn_summary_panel(parent, image_assets);
    spawn_price_panel(parent, image_assets);
    spawn_power_strategy_panel(parent, image_assets);
    spawn_opex_panel(parent, image_assets);
}

fn spawn_summary_panel(parent: &mut ChildSpawnerCommands, image_assets: &ImageAssets) {
    spawn_panel_container(parent, ActivePanel::Summary, SummaryPanel, true).with_children(
        |panel| {
            // Header with icon
            panel
                .spawn(Node {
                    flex_direction: FlexDirection::Row,
                    column_gap: Val::Px(6.0),
                    align_items: AlignItems::Center,
                    ..default()
                })
                .with_children(|header| {
                    header.spawn((
                        ImageNode::new(image_assets.icon_dashboard.clone()),
                        Node {
                            width: Val::Px(20.0),
                            height: Val::Px(20.0),
                            ..default()
                        },
                    ));
                    header.spawn((
                        Text::new("DASHBOARD"),
                        TextFont {
                            font_size: 16.0,
                            ..default()
                        },
                        TextColor(colors::TEXT_PRIMARY),
                    ));
                });

            spawn_labeled_row(panel, "Cash:", "$0.00", SummaryCashLabel);
            spawn_labeled_row(panel, "Net Revenue:", "$0.00", SummaryRevenueLabel);
            spawn_labeled_row(panel, "Reputation:", "50", SummaryRepLabel);

            // Sessions row with icons (custom structure for icon support)
            panel
                .spawn(Node {
                    flex_direction: FlexDirection::Row,
                    justify_content: JustifyContent::SpaceBetween,
                    width: Val::Percent(100.0),
                    ..default()
                })
                .with_children(|row| {
                    row.spawn((
                        Text::new("Sessions:"),
                        TextFont {
                            font_size: 12.0,
                            ..default()
                        },
                        TextColor(colors::TEXT_SECONDARY),
                    ));
                    // Value container that gets rebuilt with icons
                    row.spawn((
                        Node {
                            flex_direction: FlexDirection::Row,
                            column_gap: Val::Px(4.0),
                            align_items: AlignItems::Center,
                            ..default()
                        },
                        SessionsValueContainer,
                    ));
                });

            spawn_separator(panel);

            spawn_labeled_row(panel, "Est. Uptime:", "85%", SummaryUptimeLabel);

            panel.spawn((
                Text::new("Monitor your station's key performance indicators."),
                TextFont {
                    font_size: 11.0,
                    ..default()
                },
                TextColor(colors::TEXT_SECONDARY),
            ));
        },
    );
}

fn spawn_price_panel(parent: &mut ChildSpawnerCommands, image_assets: &ImageAssets) {
    spawn_panel_container(parent, ActivePanel::Pricing, PricePanel, false)
        .with_children(|panel| {
            // Header with icon
            panel.spawn(Node {
                flex_direction: FlexDirection::Row,
                column_gap: Val::Px(6.0),
                align_items: AlignItems::Center,
                ..default()
            }).with_children(|header| {
                header.spawn((
                    ImageNode::new(image_assets.icon_cash.clone()),
                    Node {
                        width: Val::Px(20.0),
                        height: Val::Px(20.0),
                        ..default()
                    },
                ));
                header.spawn((
                    Text::new("PRICING"),
                    TextFont { font_size: 16.0, ..default() },
                    TextColor(colors::TEXT_PRIMARY),
                ));
            });

            // Mode selector (first control — hidden until upgrade purchased)
            spawn_slider_control(panel, "Mode:", "Flat", StrategyControl::PricingMode, PricingModeLabel, image_assets);

            // Flat price slider (visible in Flat mode or when upgrade not purchased)
            spawn_slider_control(panel, "Energy $/kWh:", "$0.45", StrategyControl::EnergyPrice, EnergyPriceLabel, image_assets);

            // Dynamic pricing lock overlay
            panel.spawn((
                Node {
                    width: Val::Percent(100.0),
                    padding: UiRect::all(Val::Px(8.0)),
                    margin: UiRect::bottom(Val::Px(8.0)),
                    ..default()
                },
                BackgroundColor(Color::srgba(0.1, 0.1, 0.1, 0.8)),
                DynamicPricingLockOverlay,
            )).with_child((
                Text::new("Purchase Dynamic Pricing Engine ($15k) in Build > Upgrades to unlock TOU, cost-plus, and surge pricing modes."),
                TextFont { font_size: 11.0, ..default() },
                TextColor(Color::srgb(0.9, 0.7, 0.3)),
            ));

            // Effective price indicator
            panel.spawn((
                Node {
                    flex_direction: FlexDirection::Row,
                    justify_content: JustifyContent::SpaceBetween,
                    width: Val::Percent(100.0),
                    display: Display::None,
                    ..default()
                },
                EffectivePriceRow,
            )).with_children(|row| {
                row.spawn((
                    Text::new("Effective Price:"),
                    TextFont { font_size: 12.0, ..default() },
                    TextColor(colors::TEXT_SECONDARY),
                ));
                row.spawn((
                    Text::new("$0.45/kWh"),
                    TextFont { font_size: 12.0, ..default() },
                    TextColor(Color::srgb(0.3, 0.9, 0.3)),
                    EffectivePriceLabel,
                ));
            });

            spawn_separator(panel);

            // Demand-Responsive controls
            spawn_slider_control(panel, "Base Price:", "$0.35", StrategyControl::SurgeBasePrice, SurgeBasePriceLabel, image_assets);
            spawn_slider_control(panel, "Surge Mult:", "1.5x", StrategyControl::SurgeMultiplier, SurgeMultiplierLabel, image_assets);
            spawn_slider_control(panel, "Surge @:", "75%", StrategyControl::SurgeThreshold, SurgeThresholdLabel, image_assets);

            // TOU-Linked controls
            spawn_slider_control(panel, "Off-Peak $/kWh:", "$0.30", StrategyControl::TouOffPeakPrice, TouOffPeakPriceLabel, image_assets);
            spawn_slider_control(panel, "On-Peak $/kWh:", "$0.55", StrategyControl::TouOnPeakPrice, TouOnPeakPriceLabel, image_assets);

            // Cost-Plus controls
            spawn_slider_control(panel, "Markup %:", "200%", StrategyControl::CostPlusMarkup, CostPlusMarkupLabel, image_assets);
            spawn_slider_control(panel, "Price Floor:", "$0.20", StrategyControl::CostPlusFloor, CostPlusFloorLabel, image_assets);
            spawn_slider_control(panel, "Price Ceiling:", "$1.00", StrategyControl::CostPlusCeiling, CostPlusCeilingLabel, image_assets);

            spawn_separator(panel);

            // Common controls
            spawn_slider_control(panel, "Idle Fee $/min:", "$0.50", StrategyControl::IdleFee, IdleFeeLabel, image_assets);
            spawn_slider_control(panel, "Sell Video Ad Space:", "$2.00/hr", StrategyControl::VideoAdPrice, VideoAdPriceLabel, image_assets);

            panel.spawn((
                Text::new("Higher prices = more profit per session, but may scare away price-sensitive customers. Higher ad prices = more revenue but fewer advertisers."),
                TextFont { font_size: 11.0, ..default() },
                TextColor(colors::TEXT_SECONDARY),
            ));
        });
}

/// Power strategy panel - power density and battery controls
fn spawn_power_strategy_panel(parent: &mut ChildSpawnerCommands, image_assets: &ImageAssets) {
    spawn_panel_container(parent, ActivePanel::StrategyPower, PowerStrategyPanel, false)
        .with_children(|panel| {
            // Header with icon
            panel.spawn(Node {
                flex_direction: FlexDirection::Row,
                column_gap: Val::Px(6.0),
                align_items: AlignItems::Center,
                ..default()
            }).with_children(|header| {
                header.spawn((
                    ImageNode::new(image_assets.icon_power.clone()),
                    Node {
                        width: Val::Px(20.0),
                        height: Val::Px(20.0),
                        ..default()
                    },
                ));
                header.spawn((
                    Text::new("POWER MANAGEMENT"),
                    TextFont { font_size: 16.0, ..default() },
                    TextColor(colors::TEXT_PRIMARY),
                ));
            });

            // Locked overlay container - will be shown/hidden based on upgrade status
            panel.spawn((
                Node {
                    width: Val::Percent(100.0),
                    padding: UiRect::all(Val::Px(8.0)),
                    margin: UiRect::bottom(Val::Px(8.0)),
                    ..default()
                },
                BackgroundColor(Color::srgba(0.1, 0.1, 0.1, 0.8)),
                PowerControlsLockOverlay,
            )).with_child((
                Text::new("Purchase Advanced Power Management ($25k) in Build > Upgrades to unlock these controls."),
                TextFont { font_size: 11.0, ..default() },
                TextColor(Color::srgb(0.9, 0.7, 0.3)),
            ));

            // Power Density control
            spawn_slider_control(panel, "Power Density:", "100%", StrategyControl::PowerDensity, PowerDensityLabel, image_assets);

            // Power density explanation
            panel.spawn((
                Text::new("Power Density controls how aggressively chargers draw power. Higher density = faster charging but increases heat stress on equipment and higher demand charges. Lower density = slower charging but gentler on infrastructure."),
                TextFont { font_size: 10.0, ..default() },
                TextColor(colors::TEXT_SECONDARY),
            ));

            spawn_separator(panel);

            // Battery Controls section
            panel.spawn((
                Text::new("Battery Controls:"),
                TextFont { font_size: 12.0, ..default() },
                TextColor(colors::TEXT_SECONDARY),
            ));

            // Discharge threshold
            spawn_slider_control(panel, "Discharge @:", "65%", StrategyControl::BessDischargeThreshold, BessDischargeThresholdLabel, image_assets);

            // Charge threshold
            spawn_slider_control(panel, "Charge below:", "35%", StrategyControl::BessChargeThreshold, BessChargeThresholdLabel, image_assets);

            panel.spawn((
                Text::new("Discharge: Battery discharges when load exceeds this % of capacity to shave peak demand. Charge: Battery charges when load is below this % to store energy for later."),
                TextFont { font_size: 10.0, ..default() },
                TextColor(colors::TEXT_SECONDARY),
            ));

            spawn_separator(panel);

            // Solar Export section
            panel.spawn((
                Text::new("Solar Export:"),
                TextFont { font_size: 12.0, ..default() },
                TextColor(colors::TEXT_SECONDARY),
            ));

            spawn_slider_control(panel, "Grid Sellback:", "Never", StrategyControl::SolarExportPolicy, SolarExportPolicyLabel, image_assets);

            panel.spawn((
                Text::new("Never: excess solar is curtailed. Excess Only: export surplus after self-consumption and battery charging. Max Export: prioritize grid export over battery storage."),
                TextFont { font_size: 10.0, ..default() },
                TextColor(colors::TEXT_SECONDARY),
            ));
        });
}

/// OPEX panel - operations and maintenance controls
fn spawn_opex_panel(parent: &mut ChildSpawnerCommands, image_assets: &ImageAssets) {
    spawn_panel_container(parent, ActivePanel::Opex, RecipePanel, false)
        .with_children(|panel| {
            // Header with icon
            panel.spawn(Node {
                flex_direction: FlexDirection::Row,
                column_gap: Val::Px(6.0),
                align_items: AlignItems::Center,
                ..default()
            }).with_children(|header| {
                header.spawn((
                    ImageNode::new(image_assets.icon_briefcase.clone()),
                    Node {
                        width: Val::Px(20.0),
                        height: Val::Px(20.0),
                        ..default()
                    },
                ));
                header.spawn((
                    Text::new("OPERATIONS (OPEX)"),
                    TextFont { font_size: 16.0, ..default() },
                    TextColor(colors::TEXT_PRIMARY),
                ));
            });

            // Locked overlay container - will be shown/hidden based on upgrade status
            panel.spawn((
                Node {
                    width: Val::Percent(100.0),
                    padding: UiRect::all(Val::Px(8.0)),
                    margin: UiRect::bottom(Val::Px(8.0)),
                    ..default()
                },
                BackgroundColor(Color::srgba(0.1, 0.1, 0.1, 0.8)),
                OpexControlsLockOverlay,
            )).with_child((
                Text::new("Purchase O&M: Detect in Build > Upgrades to unlock these controls."),
                TextFont { font_size: 11.0, ..default() },
                TextColor(Color::srgb(0.9, 0.7, 0.3)),
            ));

            // Maintenance
            spawn_slider_control(panel, "Maintenance $/hr:", "$10", StrategyControl::Maintenance, MaintenanceLabel, image_assets);

            // Warranty tier selector
            spawn_slider_control(panel, "Warranty:", "None", StrategyControl::WarrantyTier, WarrantyTierLabel, image_assets);

            // Warranty premium (read-only)
            spawn_labeled_row(panel, "Warranty Premium:", "$0/mo", WarrantyPremiumLabel);

            // Warranty coverage description
            panel.spawn((
                Text::new("No warranty coverage"),
                TextFont { font_size: 11.0, ..default() },
                TextColor(colors::TEXT_SECONDARY),
                WarrantyCoverageLabel,
            ));

            spawn_separator(panel);

            // Hourly OPEX summary
            spawn_labeled_row(panel, "Hourly OPEX:", "$10/hr", HourlyOpexLabel);

            // Amenity level (read-only, set by buildings)
            panel.spawn((
                Text::new("Amenity Building:"),
                TextFont { font_size: 12.0, ..default() },
                TextColor(colors::TEXT_SECONDARY),
            ));
            panel.spawn((
                Text::new("None (build via Build Mode)"),
                TextFont { font_size: 11.0, ..default() },
                TextColor(colors::TYCOON_GREEN),
                AmenityLevelLabel,
            ));

            // Help text
            panel.spawn((
                Text::new("Higher maintenance investment = better uptime and faster repairs. Amenities attract more customers but increase hourly costs."),
                TextFont { font_size: 11.0, ..default() },
                TextColor(colors::TEXT_SECONDARY),
            ));
        });
}

// ============ Update Systems ============

pub fn update_strategy_panel_values(
    multi_site: Res<crate::resources::MultiSiteManager>,
    mut energy_price: Query<&mut Text, With<EnergyPriceLabel>>,
    mut idle_fee: Query<&mut Text, (With<IdleFeeLabel>, Without<EnergyPriceLabel>)>,
    mut video_ad_price: Query<
        &mut Text,
        (
            With<VideoAdPriceLabel>,
            Without<EnergyPriceLabel>,
            Without<IdleFeeLabel>,
        ),
    >,
    mut power_density: Query<
        &mut Text,
        (
            With<PowerDensityLabel>,
            Without<EnergyPriceLabel>,
            Without<IdleFeeLabel>,
            Without<VideoAdPriceLabel>,
        ),
    >,
    mut maintenance: Query<
        &mut Text,
        (
            With<MaintenanceLabel>,
            Without<EnergyPriceLabel>,
            Without<IdleFeeLabel>,
            Without<VideoAdPriceLabel>,
            Without<PowerDensityLabel>,
        ),
    >,
    mut amenity: Query<
        &mut Text,
        (
            With<AmenityLevelLabel>,
            Without<EnergyPriceLabel>,
            Without<IdleFeeLabel>,
            Without<VideoAdPriceLabel>,
            Without<PowerDensityLabel>,
            Without<MaintenanceLabel>,
        ),
    >,
    mut opex: Query<
        &mut Text,
        (
            With<HourlyOpexLabel>,
            Without<EnergyPriceLabel>,
            Without<IdleFeeLabel>,
            Without<VideoAdPriceLabel>,
            Without<PowerDensityLabel>,
            Without<MaintenanceLabel>,
            Without<AmenityLevelLabel>,
        ),
    >,
    mut bess_discharge: Query<
        &mut Text,
        (
            With<BessDischargeThresholdLabel>,
            Without<EnergyPriceLabel>,
            Without<IdleFeeLabel>,
            Without<VideoAdPriceLabel>,
            Without<PowerDensityLabel>,
            Without<MaintenanceLabel>,
            Without<AmenityLevelLabel>,
            Without<HourlyOpexLabel>,
        ),
    >,
    mut bess_charge: Query<
        &mut Text,
        (
            With<BessChargeThresholdLabel>,
            Without<EnergyPriceLabel>,
            Without<IdleFeeLabel>,
            Without<VideoAdPriceLabel>,
            Without<PowerDensityLabel>,
            Without<MaintenanceLabel>,
            Without<AmenityLevelLabel>,
            Without<HourlyOpexLabel>,
            Without<BessDischargeThresholdLabel>,
            Without<SolarExportPolicyLabel>,
        ),
    >,
    mut solar_export: Query<
        &mut Text,
        (
            With<SolarExportPolicyLabel>,
            Without<EnergyPriceLabel>,
            Without<IdleFeeLabel>,
            Without<VideoAdPriceLabel>,
            Without<PowerDensityLabel>,
            Without<MaintenanceLabel>,
            Without<AmenityLevelLabel>,
            Without<HourlyOpexLabel>,
            Without<BessDischargeThresholdLabel>,
            Without<BessChargeThresholdLabel>,
        ),
    >,
) {
    // Get active site data
    let Some(site_state) = multi_site.active_site() else {
        return;
    };

    for mut text in &mut energy_price {
        **text = format!("${:.2}", site_state.service_strategy.pricing.flat.price_kwh);
    }
    for mut text in &mut idle_fee {
        **text = format!("${:.2}", site_state.service_strategy.idle_fee_min);
    }
    for mut text in &mut video_ad_price {
        let prob = site_state
            .service_strategy
            .advertiser_interest_probability()
            * 100.0;
        **text = format!(
            "${:.2}/hr ({:.0}%)",
            site_state.service_strategy.ad_space_price_per_hour, prob
        );
    }
    for mut text in &mut power_density {
        **text = format!(
            "{:.0}%",
            site_state.service_strategy.target_power_density * 100.0
        );
    }
    for mut text in &mut maintenance {
        **text = format!("${:.0}", site_state.service_strategy.maintenance_investment);
    }
    for mut text in &mut amenity {
        **text = site_state.service_strategy.amenity_name();
    }
    for mut text in &mut opex {
        **text = format!(
            "${:.0}/hr",
            site_state.service_strategy.hourly_maintenance_cost()
        );
    }
    // BESS threshold labels
    for mut text in &mut bess_discharge {
        **text = format!("{:.0}%", site_state.bess_state.peak_shave_threshold * 100.0);
    }
    for mut text in &mut bess_charge {
        **text = format!("{:.0}%", site_state.bess_state.charge_threshold * 100.0);
    }
    for mut text in &mut solar_export {
        **text = site_state
            .service_strategy
            .solar_export_policy
            .display_name()
            .to_string();
    }
}

/// Update warranty-related labels (separate system to avoid query filter explosion in
/// `update_strategy_panel_values`).
pub fn update_warranty_labels(
    multi_site: Res<crate::resources::MultiSiteManager>,
    mut tier_label: Query<&mut Text, With<WarrantyTierLabel>>,
    mut premium_label: Query<&mut Text, (With<WarrantyPremiumLabel>, Without<WarrantyTierLabel>)>,
    mut coverage_label: Query<
        &mut Text,
        (
            With<WarrantyCoverageLabel>,
            Without<WarrantyTierLabel>,
            Without<WarrantyPremiumLabel>,
        ),
    >,
    chargers: Query<(
        &crate::components::charger::Charger,
        &crate::components::BelongsToSite,
    )>,
) {
    let Some(site_state) = multi_site.active_site() else {
        return;
    };

    for mut text in &mut tier_label {
        **text = site_state
            .service_strategy
            .warranty_tier
            .display_name()
            .to_string();
    }

    let viewed_site_id = multi_site.viewed_site_id;
    let monthly_premium: f32 =
        if site_state.service_strategy.warranty_tier != crate::resources::WarrantyTier::None {
            chargers
                .iter()
                .filter(|(_, b)| Some(b.site_id) == viewed_site_id)
                .map(|(c, _)| c.warranty_premium(site_state.service_strategy.warranty_tier))
                .sum()
        } else {
            0.0
        };
    for mut text in &mut premium_label {
        **text = format!("${:.0}/mo", monthly_premium);
    }

    for mut text in &mut coverage_label {
        **text = site_state
            .service_strategy
            .warranty_tier
            .description()
            .to_string();
    }
}

/// Update dynamic pricing label text values (separate system to avoid query filter explosion).
pub fn update_dynamic_pricing_labels(
    multi_site: Res<crate::resources::MultiSiteManager>,
    game_clock: Res<crate::resources::GameClock>,
    mut pricing_mode: Query<&mut Text, With<PricingModeLabel>>,
    mut tou_off_peak: Query<&mut Text, (With<TouOffPeakPriceLabel>, Without<PricingModeLabel>)>,
    mut tou_on_peak: Query<
        &mut Text,
        (
            With<TouOnPeakPriceLabel>,
            Without<PricingModeLabel>,
            Without<TouOffPeakPriceLabel>,
        ),
    >,
    mut cost_markup: Query<
        &mut Text,
        (
            With<CostPlusMarkupLabel>,
            Without<PricingModeLabel>,
            Without<TouOffPeakPriceLabel>,
            Without<TouOnPeakPriceLabel>,
        ),
    >,
    mut cost_floor: Query<
        &mut Text,
        (
            With<CostPlusFloorLabel>,
            Without<PricingModeLabel>,
            Without<TouOffPeakPriceLabel>,
            Without<TouOnPeakPriceLabel>,
            Without<CostPlusMarkupLabel>,
        ),
    >,
    mut cost_ceiling: Query<
        &mut Text,
        (
            With<CostPlusCeilingLabel>,
            Without<PricingModeLabel>,
            Without<TouOffPeakPriceLabel>,
            Without<TouOnPeakPriceLabel>,
            Without<CostPlusMarkupLabel>,
            Without<CostPlusFloorLabel>,
        ),
    >,
    mut surge_base: Query<
        &mut Text,
        (
            With<SurgeBasePriceLabel>,
            Without<PricingModeLabel>,
            Without<TouOffPeakPriceLabel>,
            Without<TouOnPeakPriceLabel>,
            Without<CostPlusMarkupLabel>,
            Without<CostPlusFloorLabel>,
            Without<CostPlusCeilingLabel>,
        ),
    >,
    mut surge_mult: Query<
        &mut Text,
        (
            With<SurgeMultiplierLabel>,
            Without<PricingModeLabel>,
            Without<TouOffPeakPriceLabel>,
            Without<TouOnPeakPriceLabel>,
            Without<CostPlusMarkupLabel>,
            Without<CostPlusFloorLabel>,
            Without<CostPlusCeilingLabel>,
            Without<SurgeBasePriceLabel>,
        ),
    >,
    mut surge_thresh: Query<
        &mut Text,
        (
            With<SurgeThresholdLabel>,
            Without<PricingModeLabel>,
            Without<TouOffPeakPriceLabel>,
            Without<TouOnPeakPriceLabel>,
            Without<CostPlusMarkupLabel>,
            Without<CostPlusFloorLabel>,
            Without<CostPlusCeilingLabel>,
            Without<SurgeBasePriceLabel>,
            Without<SurgeMultiplierLabel>,
        ),
    >,
    mut effective: Query<
        &mut Text,
        (
            With<EffectivePriceLabel>,
            Without<PricingModeLabel>,
            Without<TouOffPeakPriceLabel>,
            Without<TouOnPeakPriceLabel>,
            Without<CostPlusMarkupLabel>,
            Without<CostPlusFloorLabel>,
            Without<CostPlusCeilingLabel>,
            Without<SurgeBasePriceLabel>,
            Without<SurgeMultiplierLabel>,
            Without<SurgeThresholdLabel>,
        ),
    >,
) {
    let Some(site) = multi_site.active_site() else {
        return;
    };
    let strat = &site.service_strategy;

    for mut t in &mut pricing_mode {
        **t = strat.pricing.mode.display_name().to_string();
    }
    for mut t in &mut tou_off_peak {
        **t = format!("${:.2}", strat.pricing.tou.off_peak_price);
    }
    for mut t in &mut tou_on_peak {
        **t = format!("${:.2}", strat.pricing.tou.on_peak_price);
    }
    for mut t in &mut cost_markup {
        **t = format!("{:.0}%", strat.pricing.cost_plus.markup_pct);
    }
    for mut t in &mut cost_floor {
        **t = format!("${:.2}", strat.pricing.cost_plus.floor);
    }
    for mut t in &mut cost_ceiling {
        **t = format!("${:.2}", strat.pricing.cost_plus.ceiling);
    }
    for mut t in &mut surge_base {
        **t = format!("${:.2}", strat.pricing.surge.base_price);
    }
    for mut t in &mut surge_mult {
        **t = format!("{:.1}x", strat.pricing.surge.multiplier);
    }
    for mut t in &mut surge_thresh {
        **t = format!("{:.0}%", strat.pricing.surge.threshold * 100.0);
    }
    for mut t in &mut effective {
        let price = strat.pricing.effective_price(
            game_clock.game_time,
            &site.site_energy_config,
            site.charger_utilization,
        );
        **t = format!("${:.2}/kWh", price);
    }
}

pub fn handle_strategy_panel_buttons(
    mut multi_site: ResMut<crate::resources::MultiSiteManager>,
    mut interaction_query: Query<
        (&Interaction, &StrategyControl, Option<&MinusButton>),
        Changed<Interaction>,
    >,
) {
    // Get active site data
    let Some(site_state) = multi_site.active_site_mut() else {
        return;
    };

    // Cache upgrade status for gating
    let has_power_management = site_state.site_upgrades.has_power_management();
    let has_oem = site_state.site_upgrades.has_om_software();

    for (interaction, control, is_minus) in &mut interaction_query {
        if *interaction != Interaction::Pressed {
            continue;
        }

        let delta = if is_minus.is_some() { -0.05 } else { 0.05 };

        match control {
            StrategyControl::EnergyPrice => {
                site_state.service_strategy.pricing.flat.price_kwh =
                    (site_state.service_strategy.pricing.flat.price_kwh + delta).clamp(0.10, 2.00);
            }
            StrategyControl::IdleFee => {
                site_state.service_strategy.idle_fee_min =
                    (site_state.service_strategy.idle_fee_min + delta).clamp(0.0, 2.0);
            }
            StrategyControl::VideoAdPrice => {
                // Delta is 0.05 by default, but we want $0.50 increments for ad price
                let ad_delta = if is_minus.is_some() { -0.50 } else { 0.50 };
                site_state.service_strategy.ad_space_price_per_hour =
                    (site_state.service_strategy.ad_space_price_per_hour + ad_delta)
                        .clamp(0.50, 10.0);
            }
            StrategyControl::PowerDensity => {
                // Requires Advanced Power Management upgrade
                if !has_power_management {
                    continue;
                }
                site_state.service_strategy.target_power_density =
                    (site_state.service_strategy.target_power_density + delta).clamp(0.5, 1.2);
            }
            StrategyControl::Maintenance => {
                // Requires any OEM tier
                if !has_oem {
                    continue;
                }
                let maint_delta = if is_minus.is_some() { -5.0 } else { 5.0 };
                site_state.service_strategy.maintenance_investment =
                    (site_state.service_strategy.maintenance_investment + maint_delta)
                        .clamp(0.0, 50.0);
            }
            StrategyControl::WarrantyTier => {
                if !has_oem {
                    continue;
                }
                site_state.service_strategy.warranty_tier = if is_minus.is_some() {
                    site_state.service_strategy.warranty_tier.prev()
                } else {
                    site_state.service_strategy.warranty_tier.next()
                };
            }
            StrategyControl::BessDischargeThreshold => {
                // Requires Advanced Power Management upgrade
                if !has_power_management {
                    continue;
                }
                // Range: 50% - 90%
                site_state.bess_state.peak_shave_threshold =
                    (site_state.bess_state.peak_shave_threshold + delta).clamp(0.50, 0.90);
            }
            StrategyControl::BessChargeThreshold => {
                // Requires Advanced Power Management upgrade
                if !has_power_management {
                    continue;
                }
                // Range: 20% - 50%
                site_state.bess_state.charge_threshold =
                    (site_state.bess_state.charge_threshold + delta).clamp(0.20, 0.50);
            }
            StrategyControl::SolarExportPolicy => {
                if !has_power_management {
                    continue;
                }
                site_state.service_strategy.solar_export_policy = if is_minus.is_some() {
                    site_state.service_strategy.solar_export_policy.prev()
                } else {
                    site_state.service_strategy.solar_export_policy.next()
                };
            }
            StrategyControl::PricingMode => {
                if !site_state.site_upgrades.has_dynamic_pricing() {
                    continue;
                }
                site_state.service_strategy.pricing.mode = if is_minus.is_some() {
                    site_state.service_strategy.pricing.mode.prev()
                } else {
                    site_state.service_strategy.pricing.mode.next()
                };
            }
            StrategyControl::TouOffPeakPrice => {
                if !site_state.site_upgrades.has_dynamic_pricing() {
                    continue;
                }
                site_state.service_strategy.pricing.tou.off_peak_price =
                    (site_state.service_strategy.pricing.tou.off_peak_price + delta)
                        .clamp(0.10, 2.00);
            }
            StrategyControl::TouOnPeakPrice => {
                if !site_state.site_upgrades.has_dynamic_pricing() {
                    continue;
                }
                site_state.service_strategy.pricing.tou.on_peak_price =
                    (site_state.service_strategy.pricing.tou.on_peak_price + delta)
                        .clamp(0.10, 2.00);
            }
            StrategyControl::CostPlusMarkup => {
                if !site_state.site_upgrades.has_dynamic_pricing() {
                    continue;
                }
                let markup_delta = if is_minus.is_some() { -25.0 } else { 25.0 };
                site_state.service_strategy.pricing.cost_plus.markup_pct =
                    (site_state.service_strategy.pricing.cost_plus.markup_pct + markup_delta)
                        .clamp(50.0, 2000.0);
            }
            StrategyControl::CostPlusFloor => {
                if !site_state.site_upgrades.has_dynamic_pricing() {
                    continue;
                }
                site_state.service_strategy.pricing.cost_plus.floor =
                    (site_state.service_strategy.pricing.cost_plus.floor + delta).clamp(0.10, 1.00);
            }
            StrategyControl::CostPlusCeiling => {
                if !site_state.site_upgrades.has_dynamic_pricing() {
                    continue;
                }
                let ceil_delta = if is_minus.is_some() { -0.10 } else { 0.10 };
                site_state.service_strategy.pricing.cost_plus.ceiling =
                    (site_state.service_strategy.pricing.cost_plus.ceiling + ceil_delta)
                        .clamp(0.30, 3.00);
            }
            StrategyControl::SurgeBasePrice => {
                if !site_state.site_upgrades.has_dynamic_pricing() {
                    continue;
                }
                site_state.service_strategy.pricing.surge.base_price =
                    (site_state.service_strategy.pricing.surge.base_price + delta)
                        .clamp(0.10, 1.50);
            }
            StrategyControl::SurgeMultiplier => {
                if !site_state.site_upgrades.has_dynamic_pricing() {
                    continue;
                }
                let mult_delta = if is_minus.is_some() { -0.1 } else { 0.1 };
                site_state.service_strategy.pricing.surge.multiplier =
                    (site_state.service_strategy.pricing.surge.multiplier + mult_delta)
                        .clamp(1.0, 3.0);
            }
            StrategyControl::SurgeThreshold => {
                if !site_state.site_upgrades.has_dynamic_pricing() {
                    continue;
                }
                site_state.service_strategy.pricing.surge.threshold =
                    (site_state.service_strategy.pricing.surge.threshold + delta).clamp(0.50, 0.90);
            }
        }
    }
}

/// Update slider fill bar widths based on ServiceStrategy values
pub fn update_slider_fill_widths(
    multi_site: Res<crate::resources::MultiSiteManager>,
    mut slider_query: Query<(&SliderFill, &mut Node)>,
) {
    // Get active site data
    let Some(site_state) = multi_site.active_site() else {
        return;
    };

    for (slider_fill, mut node) in &mut slider_query {
        let percentage = match slider_fill.0 {
            StrategyControl::EnergyPrice => {
                // Range: 0.10 - 2.00
                let normalized =
                    (site_state.service_strategy.pricing.flat.price_kwh - 0.10) / (2.00 - 0.10);
                (normalized * 100.0).clamp(0.0, 100.0)
            }
            StrategyControl::IdleFee => {
                // Range: 0.0 - 2.0
                let normalized = site_state.service_strategy.idle_fee_min / 2.0;
                (normalized * 100.0).clamp(0.0, 100.0)
            }
            StrategyControl::VideoAdPrice => {
                // Range: 0.50 - 10.0
                let normalized =
                    (site_state.service_strategy.ad_space_price_per_hour - 0.50) / (10.0 - 0.50);
                (normalized * 100.0).clamp(0.0, 100.0)
            }
            StrategyControl::PowerDensity => {
                // Range: 0.5 - 1.2
                let normalized =
                    (site_state.service_strategy.target_power_density - 0.5) / (1.2 - 0.5);
                (normalized * 100.0).clamp(0.0, 100.0)
            }
            StrategyControl::Maintenance => {
                // Range: 0.0 - 50.0
                let normalized = site_state.service_strategy.maintenance_investment / 50.0;
                (normalized * 100.0).clamp(0.0, 100.0)
            }
            StrategyControl::BessDischargeThreshold => {
                // Range: 0.50 - 0.90
                let normalized =
                    (site_state.bess_state.peak_shave_threshold - 0.50) / (0.90 - 0.50);
                (normalized * 100.0).clamp(0.0, 100.0)
            }
            StrategyControl::BessChargeThreshold => {
                // Range: 0.20 - 0.50
                let normalized = (site_state.bess_state.charge_threshold - 0.20) / (0.50 - 0.20);
                (normalized * 100.0).clamp(0.0, 100.0)
            }
            StrategyControl::PricingMode => {
                use crate::resources::PricingMode;
                match site_state.service_strategy.pricing.mode {
                    PricingMode::Flat => 0.0,
                    PricingMode::TouLinked => 33.0,
                    PricingMode::CostPlus => 66.0,
                    PricingMode::DemandResponsive => 100.0,
                }
            }
            StrategyControl::TouOffPeakPrice => {
                let normalized =
                    (site_state.service_strategy.pricing.tou.off_peak_price - 0.10) / (2.00 - 0.10);
                (normalized * 100.0).clamp(0.0, 100.0)
            }
            StrategyControl::TouOnPeakPrice => {
                let normalized =
                    (site_state.service_strategy.pricing.tou.on_peak_price - 0.10) / (2.00 - 0.10);
                (normalized * 100.0).clamp(0.0, 100.0)
            }
            StrategyControl::CostPlusMarkup => {
                let normalized = (site_state.service_strategy.pricing.cost_plus.markup_pct - 50.0)
                    / (2000.0 - 50.0);
                (normalized * 100.0).clamp(0.0, 100.0)
            }
            StrategyControl::CostPlusFloor => {
                let normalized =
                    (site_state.service_strategy.pricing.cost_plus.floor - 0.10) / (1.00 - 0.10);
                (normalized * 100.0).clamp(0.0, 100.0)
            }
            StrategyControl::CostPlusCeiling => {
                let normalized =
                    (site_state.service_strategy.pricing.cost_plus.ceiling - 0.30) / (3.00 - 0.30);
                (normalized * 100.0).clamp(0.0, 100.0)
            }
            StrategyControl::SurgeBasePrice => {
                let normalized =
                    (site_state.service_strategy.pricing.surge.base_price - 0.10) / (1.50 - 0.10);
                (normalized * 100.0).clamp(0.0, 100.0)
            }
            StrategyControl::SurgeMultiplier => {
                let normalized =
                    (site_state.service_strategy.pricing.surge.multiplier - 1.0) / (3.0 - 1.0);
                (normalized * 100.0).clamp(0.0, 100.0)
            }
            StrategyControl::SurgeThreshold => {
                let normalized =
                    (site_state.service_strategy.pricing.surge.threshold - 0.50) / (0.90 - 0.50);
                (normalized * 100.0).clamp(0.0, 100.0)
            }
            StrategyControl::SolarExportPolicy => {
                use crate::resources::SolarExportPolicy;
                match site_state.service_strategy.solar_export_policy {
                    SolarExportPolicy::Never => 0.0,
                    SolarExportPolicy::ExcessOnly => 50.0,
                    SolarExportPolicy::MaxExport => 100.0,
                }
            }
            StrategyControl::WarrantyTier => {
                use crate::resources::WarrantyTier;
                match site_state.service_strategy.warranty_tier {
                    WarrantyTier::None => 0.0,
                    WarrantyTier::Standard => 33.0,
                    WarrantyTier::Comprehensive => 66.0,
                    WarrantyTier::Premium => 100.0,
                }
            }
        };

        node.width = Val::Percent(percentage);
    }
}

pub fn update_summary_panel_values(
    mut commands: Commands,
    game_state: Res<GameState>,
    multi_site: Res<crate::resources::MultiSiteManager>,
    image_assets: Res<ImageAssets>,
    mut cash: Query<&mut Text, With<SummaryCashLabel>>,
    mut revenue: Query<&mut Text, (With<SummaryRevenueLabel>, Without<SummaryCashLabel>)>,
    mut rep: Query<
        &mut Text,
        (
            With<SummaryRepLabel>,
            Without<SummaryCashLabel>,
            Without<SummaryRevenueLabel>,
        ),
    >,
    sessions_container: Query<Entity, With<SessionsValueContainer>>,
    children_query: Query<&Children>,
    mut uptime: Query<
        &mut Text,
        (
            With<SummaryUptimeLabel>,
            Without<SummaryCashLabel>,
            Without<SummaryRevenueLabel>,
            Without<SummaryRepLabel>,
        ),
    >,
) {
    for mut text in &mut cash {
        **text = format!("${:.0}", game_state.cash);
    }
    for mut text in &mut revenue {
        let display_revenue = {
            let rounded = game_state.ledger.net_revenue_f32().round();
            if rounded == 0.0 { 0.0 } else { rounded }
        };
        **text = format!("${display_revenue:.0}");
    }
    for mut text in &mut rep {
        **text = format!("{}", game_state.reputation);
    }

    // Rebuild sessions display with icons
    for container_entity in &sessions_container {
        // Clear existing children
        if let Ok(children) = children_query.get(container_entity) {
            for &child in children {
                commands.entity(child).try_despawn();
            }
        }

        // Rebuild with icons: [success_icon] completed / [fault_icon] failed
        let completed = game_state.sessions_completed;
        let failed = game_state.sessions_failed;

        commands.entity(container_entity).with_children(|parent| {
            // Completed count with success icon
            parent.spawn((
                ImageNode::new(image_assets.icon_success.clone()),
                Node {
                    width: Val::Px(12.0),
                    height: Val::Px(12.0),
                    ..default()
                },
            ));
            parent.spawn((
                Text::new(format!("{completed}")),
                TextFont {
                    font_size: 12.0,
                    ..default()
                },
                TextColor(Color::srgb(0.3, 0.8, 0.3)),
            ));

            // Separator
            parent.spawn((
                Text::new(" / "),
                TextFont {
                    font_size: 12.0,
                    ..default()
                },
                TextColor(colors::TEXT_SECONDARY),
            ));

            // Failed count with fault icon
            parent.spawn((
                ImageNode::new(image_assets.icon_fault.clone()),
                Node {
                    width: Val::Px(12.0),
                    height: Val::Px(12.0),
                    ..default()
                },
            ));
            parent.spawn((
                Text::new(format!("{failed}")),
                TextFont {
                    font_size: 12.0,
                    ..default()
                },
                TextColor(Color::srgb(1.0, 0.4, 0.4)),
            ));
        });
    }

    for mut text in &mut uptime {
        // Get active site data
        let uptime_pct = if let Some(site_state) = multi_site.active_site() {
            site_state.site_upgrades.estimated_uptime_percent()
        } else {
            85.0 // Default base uptime
        };
        **text = format!("{uptime_pct:.0}%");
    }
}

// handle_upgrade_purchases moved to build_panels.rs

// ============ Lock Overlay Systems ============

/// Update Power panel lock overlay visibility based on Advanced Power Management upgrade
pub fn update_power_lock_overlay(
    multi_site: Res<crate::resources::MultiSiteManager>,
    mut overlay_query: Query<&mut Node, With<PowerControlsLockOverlay>>,
) {
    let has_upgrade = multi_site
        .active_site()
        .is_some_and(|site| site.site_upgrades.has_power_management());

    for mut node in &mut overlay_query {
        node.display = if has_upgrade {
            Display::None
        } else {
            Display::Flex
        };
    }
}

/// Update OPEX panel lock overlay visibility based on OEM tier upgrade
pub fn update_opex_lock_overlay(
    multi_site: Res<crate::resources::MultiSiteManager>,
    mut overlay_query: Query<&mut Node, With<OpexControlsLockOverlay>>,
) {
    let has_upgrade = multi_site
        .active_site()
        .is_some_and(|site| site.site_upgrades.has_om_software());

    for mut node in &mut overlay_query {
        node.display = if has_upgrade {
            Display::None
        } else {
            Display::Flex
        };
    }
}

/// Update maintenance slider visual state based on OEM tier upgrade
/// Dims buttons, slider, and text when no OEM tier is purchased
pub fn update_maintenance_control_visual_state(
    multi_site: Res<crate::resources::MultiSiteManager>,
    mut button_query: Query<(&StrategyControl, &mut BackgroundColor), With<Button>>,
    mut slider_fill_query: Query<
        (&SliderFill, &mut BackgroundColor),
        (Without<Button>, Without<SliderTrack>),
    >,
    mut slider_track_query: Query<
        (&SliderTrack, &mut BackgroundColor),
        (Without<Button>, Without<SliderFill>),
    >,
    mut label_text_query: Query<(&SliderLabelText, &mut TextColor)>,
    mut value_text_query: Query<&mut TextColor, (With<MaintenanceLabel>, Without<SliderLabelText>)>,
) {
    let has_upgrade = multi_site
        .active_site()
        .is_some_and(|site| site.site_upgrades.has_om_software());

    // Update button colors for maintenance controls
    for (control, mut bg) in &mut button_query {
        if matches!(control, StrategyControl::Maintenance) {
            *bg = if has_upgrade {
                BackgroundColor(colors::BUTTON_NORMAL)
            } else {
                BackgroundColor(colors::BUTTON_DISABLED)
            };
        }
    }

    // Update slider fill color for maintenance control
    for (slider_fill, mut bg) in &mut slider_fill_query {
        if matches!(slider_fill.0, StrategyControl::Maintenance) {
            *bg = if has_upgrade {
                BackgroundColor(colors::SLIDER_FILL)
            } else {
                // Dimmed version of slider fill
                BackgroundColor(Color::srgba(0.3, 0.5, 0.3, 0.5))
            };
        }
    }

    // Update slider track color for maintenance control
    for (slider_track, mut bg) in &mut slider_track_query {
        if matches!(slider_track.0, StrategyControl::Maintenance) {
            *bg = if has_upgrade {
                BackgroundColor(colors::SLIDER_TRACK)
            } else {
                // Dimmed version of slider track
                BackgroundColor(Color::srgba(0.15, 0.18, 0.15, 0.5))
            };
        }
    }

    // Update label text color for maintenance control
    for (label_text, mut text_color) in &mut label_text_query {
        if matches!(label_text.0, StrategyControl::Maintenance) {
            *text_color = if has_upgrade {
                TextColor(colors::TEXT_SECONDARY)
            } else {
                TextColor(colors::TEXT_DISABLED)
            };
        }
    }

    // Update value text color for maintenance control
    for mut text_color in &mut value_text_query {
        *text_color = if has_upgrade {
            TextColor(colors::TYCOON_GREEN)
        } else {
            TextColor(colors::TEXT_DISABLED)
        };
    }
}

/// Update power control visual state based on Advanced Power Management upgrade
/// Dims buttons, sliders, and text when the upgrade is not purchased
pub fn update_power_control_visual_state(
    multi_site: Res<crate::resources::MultiSiteManager>,
    mut button_query: Query<(&StrategyControl, &mut BackgroundColor), With<Button>>,
    mut slider_fill_query: Query<
        (&SliderFill, &mut BackgroundColor),
        (Without<Button>, Without<SliderTrack>),
    >,
    mut slider_track_query: Query<
        (&SliderTrack, &mut BackgroundColor),
        (Without<Button>, Without<SliderFill>),
    >,
    mut label_text_query: Query<(&SliderLabelText, &mut TextColor)>,
    mut power_density_value: Query<
        &mut TextColor,
        (
            With<PowerDensityLabel>,
            Without<SliderLabelText>,
            Without<BessDischargeThresholdLabel>,
            Without<BessChargeThresholdLabel>,
            Without<SolarExportPolicyLabel>,
        ),
    >,
    mut discharge_threshold_value: Query<
        &mut TextColor,
        (
            With<BessDischargeThresholdLabel>,
            Without<SliderLabelText>,
            Without<PowerDensityLabel>,
            Without<BessChargeThresholdLabel>,
            Without<SolarExportPolicyLabel>,
        ),
    >,
    mut charge_threshold_value: Query<
        &mut TextColor,
        (
            With<BessChargeThresholdLabel>,
            Without<SliderLabelText>,
            Without<PowerDensityLabel>,
            Without<BessDischargeThresholdLabel>,
            Without<SolarExportPolicyLabel>,
        ),
    >,
    mut solar_export_value: Query<
        &mut TextColor,
        (
            With<SolarExportPolicyLabel>,
            Without<SliderLabelText>,
            Without<PowerDensityLabel>,
            Without<BessDischargeThresholdLabel>,
            Without<BessChargeThresholdLabel>,
        ),
    >,
) {
    let has_upgrade = multi_site
        .active_site()
        .is_some_and(|site| site.site_upgrades.has_power_management());

    let is_power_control = |control: &StrategyControl| {
        matches!(
            control,
            StrategyControl::PowerDensity
                | StrategyControl::BessDischargeThreshold
                | StrategyControl::BessChargeThreshold
                | StrategyControl::SolarExportPolicy
        )
    };

    // Update button colors for power controls
    for (control, mut bg) in &mut button_query {
        if is_power_control(control) {
            *bg = if has_upgrade {
                BackgroundColor(colors::BUTTON_NORMAL)
            } else {
                BackgroundColor(colors::BUTTON_DISABLED)
            };
        }
    }

    // Update slider fill color for power controls
    for (slider_fill, mut bg) in &mut slider_fill_query {
        if is_power_control(&slider_fill.0) {
            *bg = if has_upgrade {
                BackgroundColor(colors::SLIDER_FILL)
            } else {
                BackgroundColor(Color::srgba(0.3, 0.5, 0.3, 0.5))
            };
        }
    }

    // Update slider track color for power controls
    for (slider_track, mut bg) in &mut slider_track_query {
        if is_power_control(&slider_track.0) {
            *bg = if has_upgrade {
                BackgroundColor(colors::SLIDER_TRACK)
            } else {
                BackgroundColor(Color::srgba(0.15, 0.18, 0.15, 0.5))
            };
        }
    }

    // Update label text color for power controls
    for (label_text, mut text_color) in &mut label_text_query {
        if is_power_control(&label_text.0) {
            *text_color = if has_upgrade {
                TextColor(colors::TEXT_SECONDARY)
            } else {
                TextColor(colors::TEXT_DISABLED)
            };
        }
    }

    // Update value text color for power controls
    let value_color = if has_upgrade {
        TextColor(colors::TYCOON_GREEN)
    } else {
        TextColor(colors::TEXT_DISABLED)
    };

    for mut text_color in &mut power_density_value {
        *text_color = value_color;
    }
    for mut text_color in &mut discharge_threshold_value {
        *text_color = value_color;
    }
    for mut text_color in &mut charge_threshold_value {
        *text_color = value_color;
    }
    for mut text_color in &mut solar_export_value {
        *text_color = value_color;
    }
}

/// Update dynamic pricing lock overlay and mode-specific control visibility.
///
/// Uses `SliderContainer` to toggle entire slider sections at once (hiding
/// the parent container hides all children: labels, value text, buttons, bars).
pub fn update_dynamic_pricing_visibility(
    multi_site: Res<crate::resources::MultiSiteManager>,
    mut lock_overlay: Query<&mut Node, With<DynamicPricingLockOverlay>>,
    mut containers: Query<(&SliderContainer, &mut Node), Without<DynamicPricingLockOverlay>>,
    mut effective_price_row: Query<
        &mut Node,
        (
            With<EffectivePriceRow>,
            Without<DynamicPricingLockOverlay>,
            Without<SliderContainer>,
        ),
    >,
) {
    let Some(site) = multi_site.active_site() else {
        return;
    };
    let has_upgrade = site.site_upgrades.has_dynamic_pricing();
    let mode = site.service_strategy.pricing.mode;

    for mut node in &mut lock_overlay {
        node.display = if has_upgrade {
            Display::None
        } else {
            Display::Flex
        };
    }

    let should_show = |control: &StrategyControl| -> bool {
        use crate::resources::PricingMode;
        match control {
            StrategyControl::PricingMode => has_upgrade,
            StrategyControl::TouOffPeakPrice | StrategyControl::TouOnPeakPrice => {
                has_upgrade && mode == PricingMode::TouLinked
            }
            StrategyControl::CostPlusMarkup
            | StrategyControl::CostPlusFloor
            | StrategyControl::CostPlusCeiling => has_upgrade && mode == PricingMode::CostPlus,
            StrategyControl::SurgeBasePrice
            | StrategyControl::SurgeMultiplier
            | StrategyControl::SurgeThreshold => {
                has_upgrade && mode == PricingMode::DemandResponsive
            }
            StrategyControl::EnergyPrice => !has_upgrade || mode == PricingMode::Flat,
            _ => true,
        }
    };

    for (container, mut node) in &mut containers {
        if is_dynamic_pricing_control(&container.0) {
            node.display = if should_show(&container.0) {
                Display::Flex
            } else {
                Display::None
            };
        }
    }

    for mut node in &mut effective_price_row {
        node.display = if has_upgrade {
            Display::Flex
        } else {
            Display::None
        };
    }
}

fn is_dynamic_pricing_control(control: &StrategyControl) -> bool {
    matches!(
        control,
        StrategyControl::PricingMode
            | StrategyControl::TouOffPeakPrice
            | StrategyControl::TouOnPeakPrice
            | StrategyControl::CostPlusMarkup
            | StrategyControl::CostPlusFloor
            | StrategyControl::CostPlusCeiling
            | StrategyControl::SurgeBasePrice
            | StrategyControl::SurgeMultiplier
            | StrategyControl::SurgeThreshold
            | StrategyControl::EnergyPrice
    )
}
