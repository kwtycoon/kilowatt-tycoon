//! Unified left sidebar UI - combines strategy panels and build panels
//!
//! The sidebar displays different panels based on the top nav selection:
//! - Strategy panels: Summary, Price, Recipe, Supplies, Upgrades
//! - Build panels: Chargers, Infrastructure, Amenities
//! - Start Day button: Always visible at the bottom

pub mod build_panels;
pub mod operations_panel;
pub mod panel;
pub mod power_panel_inline;
pub mod rent_panel;
pub mod start_day;
pub mod strategy_panels;

use crate::states::is_game_visible;
use bevy::prelude::*;

// Re-export commonly used types
pub use build_panels::*;
pub use operations_panel::*;
pub use panel::*;
pub use power_panel_inline::*;
pub use rent_panel::*;
pub use start_day::*;
pub use strategy_panels::*;

use bevy::ecs::hierarchy::ChildSpawnerCommands;

// ============ Core Types ============

/// Primary (top-level) navigation tabs
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum PrimaryNav {
    #[default]
    Build,
    Strategy,
    Stats,
    Rent,
}

impl PrimaryNav {
    /// Get the default secondary tab for this primary
    pub fn default_secondary(&self) -> SecondaryNav {
        SecondaryNav::tabs_for_primary(*self)[0]
    }
}

/// Secondary (sub) navigation tabs
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SecondaryNav {
    // Build sub-tabs
    BuildChargers,
    BuildInfra,
    BuildAmenities,
    BuildUpgrades,
    // Strategy sub-tabs
    StrategyPricing,
    StrategyPower,
    StrategyOpex,
    // Stats sub-tabs
    StatsSummary,
    StatsPower,
    StatsOperations,
    // Rent sub-tabs
    RentLocations,
}

impl SecondaryNav {
    /// Get the primary nav that this secondary nav belongs to
    pub fn primary(&self) -> PrimaryNav {
        match self {
            SecondaryNav::BuildChargers
            | SecondaryNav::BuildInfra
            | SecondaryNav::BuildAmenities
            | SecondaryNav::BuildUpgrades => PrimaryNav::Build,
            SecondaryNav::StrategyPricing
            | SecondaryNav::StrategyPower
            | SecondaryNav::StrategyOpex => PrimaryNav::Strategy,
            SecondaryNav::StatsSummary
            | SecondaryNav::StatsPower
            | SecondaryNav::StatsOperations => PrimaryNav::Stats,
            SecondaryNav::RentLocations => PrimaryNav::Rent,
        }
    }

    /// Get all secondary tabs for a given primary
    pub fn tabs_for_primary(primary: PrimaryNav) -> &'static [SecondaryNav] {
        match primary {
            PrimaryNav::Build => &[
                SecondaryNav::BuildChargers,
                SecondaryNav::BuildInfra,
                SecondaryNav::BuildAmenities,
                SecondaryNav::BuildUpgrades,
            ],
            PrimaryNav::Strategy => &[
                SecondaryNav::StrategyPricing,
                SecondaryNav::StrategyPower,
                SecondaryNav::StrategyOpex,
            ],
            PrimaryNav::Stats => &[
                SecondaryNav::StatsSummary,
                SecondaryNav::StatsPower,
                SecondaryNav::StatsOperations,
            ],
            PrimaryNav::Rent => &[SecondaryNav::RentLocations],
        }
    }

    /// Display name for the tab button
    pub fn display_name(&self) -> &'static str {
        match self {
            SecondaryNav::BuildChargers => "Chargers",
            SecondaryNav::BuildInfra => "Infra",
            SecondaryNav::BuildAmenities => "Amenities",
            SecondaryNav::BuildUpgrades => "Upgrades",
            SecondaryNav::StrategyPricing => "Pricing",
            SecondaryNav::StrategyPower => "Power",
            SecondaryNav::StrategyOpex => "OPEX",
            SecondaryNav::StatsSummary => "Summary",
            SecondaryNav::StatsPower => "Power",
            SecondaryNav::StatsOperations => "Operations",
            SecondaryNav::RentLocations => "Locations",
        }
    }
}

/// Navigation state - tracks current selection
#[derive(Resource, Debug, Clone, Copy)]
pub struct NavigationState {
    pub primary: PrimaryNav,
    pub secondary: SecondaryNav,
}

impl Default for NavigationState {
    fn default() -> Self {
        let primary = PrimaryNav::default();
        Self {
            primary,
            secondary: primary.default_secondary(),
        }
    }
}

impl NavigationState {
    /// Switch to a new primary tab, selecting its default secondary
    pub fn set_primary(&mut self, primary: PrimaryNav) {
        self.primary = primary;
        self.secondary = SecondaryNav::tabs_for_primary(primary)[0];
    }

    /// Switch to a specific secondary tab (also updates primary if needed)
    pub fn set_secondary(&mut self, secondary: SecondaryNav) {
        self.primary = secondary.primary();
        self.secondary = secondary;
    }
}

// Legacy compatibility - kept for gradual migration
/// Which panel is currently active in the sidebar (legacy enum)
#[derive(Resource, Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ActivePanel {
    #[default]
    Summary,
    Power,
    Pricing,
    StrategyPower,
    Opex,
    Supplies,
    Upgrades,
    BuildChargers,
    BuildInfra,
    BuildAmenities,
    Rent,
    Operations,
}

impl ActivePanel {
    /// Returns true if this is one of the build panels
    pub fn is_build_panel(&self) -> bool {
        matches!(
            self,
            ActivePanel::BuildChargers | ActivePanel::BuildInfra | ActivePanel::BuildAmenities
        )
    }

    /// Convert from NavigationState to legacy ActivePanel
    pub fn from_nav(nav: &NavigationState) -> Self {
        match nav.secondary {
            SecondaryNav::BuildChargers => ActivePanel::BuildChargers,
            SecondaryNav::BuildInfra => ActivePanel::BuildInfra,
            SecondaryNav::BuildAmenities => ActivePanel::BuildAmenities,
            SecondaryNav::BuildUpgrades => ActivePanel::Upgrades,
            SecondaryNav::StrategyPricing => ActivePanel::Pricing,
            SecondaryNav::StrategyPower => ActivePanel::StrategyPower,
            SecondaryNav::StrategyOpex => ActivePanel::Opex,
            SecondaryNav::StatsSummary => ActivePanel::Summary,
            SecondaryNav::StatsPower => ActivePanel::Power,
            SecondaryNav::StatsOperations => ActivePanel::Operations,
            SecondaryNav::RentLocations => ActivePanel::Rent,
        }
    }
}

// ============ Secondary Tab Components ============

#[derive(Component)]
pub struct SecondaryTabRow;

#[derive(Component)]
pub struct SecondaryTabButton {
    pub tab: SecondaryNav,
}

// ============ Colors ============

pub(crate) mod colors {
    use bevy::prelude::Color;

    pub const TYCOON_GREEN: Color = Color::srgb(0.196, 0.804, 0.196); // #32CD32
    pub const PANEL_BG: Color = Color::srgb(0.1, 0.15, 0.1);
    pub const BUTTON_NORMAL: Color = Color::srgb(0.25, 0.35, 0.25);
    pub const BUTTON_SELECTED: Color = Color::srgb(0.3, 0.7, 0.3);
    pub const BUTTON_DISABLED: Color = Color::srgb(0.15, 0.18, 0.15);
    pub const TEXT_PRIMARY: Color = Color::srgb(0.95, 0.95, 0.95);
    pub const TEXT_SECONDARY: Color = Color::srgb(0.7, 0.85, 0.7);
    pub const TEXT_DISABLED: Color = Color::srgb(0.4, 0.4, 0.4);
    #[allow(dead_code)]
    pub const TEXT_ERROR: Color = Color::srgb(0.9, 0.4, 0.4);
    pub const SLIDER_TRACK: Color = Color::srgb(0.2, 0.25, 0.2);
    pub const SLIDER_FILL: Color = Color::srgb(0.4, 0.8, 0.4);
    pub const START_DAY_BRIGHT: Color = Color::srgb(0.15, 0.85, 0.25); // Vivid green
    #[allow(dead_code)]
    pub const START_DAY_HOVER: Color = Color::srgb(0.2, 0.95, 0.3); // Even brighter on hover
    #[allow(dead_code)]
    pub const START_DAY_DISABLED: Color = Color::srgb(0.25, 0.3, 0.25);
    pub const START_DAY_ERROR: Color = Color::srgb(0.85, 0.35, 0.2); // Red-orange for errors
    pub const START_DAY_BORDER: Color = Color::srgba(0.1, 0.95, 0.2, 0.6); // Bright green border
}

// ============ Spawn Function ============

/// Spawn the complete sidebar content (call from HUD setup)
pub fn spawn_sidebar_content(
    parent: &mut ChildSpawnerCommands,
    image_assets: &crate::resources::ImageAssets,
) {
    parent
        .spawn((
            Node {
                width: Val::Px(340.0),
                flex_direction: FlexDirection::Column,
                padding: UiRect::all(Val::Px(12.0)),
                row_gap: Val::Px(10.0),
                overflow: Overflow::clip_y(),
                display: Display::Flex,
                ..default()
            },
            BackgroundColor(colors::PANEL_BG),
            crate::ui::hud::SidebarRoot,
        ))
        .with_children(|panel| {
            // Secondary tab row (tier 2 navigation)
            spawn_secondary_tab_row(panel);

            // Separator
            panel.spawn((
                Node {
                    width: Val::Percent(100.0),
                    height: Val::Px(2.0),
                    margin: UiRect::vertical(Val::Px(4.0)),
                    ..default()
                },
                BackgroundColor(colors::TYCOON_GREEN),
            ));

            // Spawn all panels
            spawn_strategy_panels(panel, image_assets);
            spawn_build_panels(panel, image_assets);
            spawn_power_panel(panel, image_assets);
            spawn_operations_panel(panel, image_assets);
            spawn_rent_panel(panel);
        });
}

/// Spawn the secondary tab row (sub-tabs)
fn spawn_secondary_tab_row(parent: &mut ChildSpawnerCommands) {
    parent
        .spawn((
            Node {
                flex_direction: FlexDirection::Row,
                column_gap: Val::Px(2.0),
                width: Val::Percent(100.0),
                margin: UiRect::bottom(Val::Px(8.0)),
                ..default()
            },
            SecondaryTabRow,
        ))
        .with_children(|tabs| {
            // Start with default primary tabs
            for &tab in SecondaryNav::tabs_for_primary(PrimaryNav::default()) {
                spawn_secondary_tab_button(tabs, tab);
            }
        });
}

fn spawn_secondary_tab_button(parent: &mut ChildSpawnerCommands, tab: SecondaryNav) {
    let is_default = tab == PrimaryNav::default().default_secondary();

    parent
        .spawn((
            Button,
            Node {
                flex_grow: 1.0,
                height: Val::Px(32.0),
                justify_content: JustifyContent::Center,
                align_items: AlignItems::Center,
                ..default()
            },
            BackgroundColor(if is_default {
                colors::BUTTON_SELECTED
            } else {
                colors::BUTTON_NORMAL
            }),
            SecondaryTabButton { tab },
        ))
        .with_child((
            Text::new(tab.display_name()),
            TextFont {
                font_size: 12.0,
                ..default()
            },
            TextColor(colors::TEXT_PRIMARY),
        ));
}

// ============ Plugin ============

// ============ Navigation Systems ============

/// Handle secondary tab button clicks
pub fn handle_secondary_tab_clicks(
    mut nav_state: ResMut<NavigationState>,
    mut interaction_query: Query<(&Interaction, &SecondaryTabButton), Changed<Interaction>>,
) {
    for (interaction, btn) in &mut interaction_query {
        if *interaction == Interaction::Pressed {
            nav_state.set_secondary(btn.tab);
        }
    }
}

/// Update secondary tab row when primary changes
pub fn update_secondary_tab_row(
    nav_state: Res<NavigationState>,
    mut commands: Commands,
    tab_row_query: Query<Entity, With<SecondaryTabRow>>,
    children_query: Query<&Children>,
    button_query: Query<Entity, With<SecondaryTabButton>>,
) {
    if !nav_state.is_changed() {
        return;
    }

    // Despawn old tabs and respawn for new primary
    for entity in &tab_row_query {
        // Despawn all button children
        if let Ok(children) = children_query.get(entity) {
            for &child in children {
                if button_query.contains(child) {
                    commands.entity(child).try_despawn();
                }
            }
        }

        // Spawn new tabs for current primary
        commands.entity(entity).with_children(|tabs| {
            for &tab in SecondaryNav::tabs_for_primary(nav_state.primary) {
                spawn_secondary_tab_button(tabs, tab);
            }
        });
    }
}

/// Sync secondary tab button colors with active state
pub fn sync_secondary_tab_colors(
    nav_state: Res<NavigationState>,
    mut button_query: Query<(&SecondaryTabButton, &mut BackgroundColor)>,
) {
    for (btn, mut bg) in &mut button_query {
        *bg = if btn.tab == nav_state.secondary {
            BackgroundColor(colors::BUTTON_SELECTED)
        } else {
            BackgroundColor(colors::BUTTON_NORMAL)
        };
    }
}

/// Update panel visibility based on NavigationState
pub fn update_panel_visibility_from_nav(
    nav_state: Res<NavigationState>,
    mut panels: Query<(&PanelContent, &mut Node)>,
) {
    if !nav_state.is_changed() {
        return;
    }

    // Map SecondaryNav to ActivePanel
    let active = match nav_state.secondary {
        SecondaryNav::BuildChargers => ActivePanel::BuildChargers,
        SecondaryNav::BuildInfra => ActivePanel::BuildInfra,
        SecondaryNav::BuildAmenities => ActivePanel::BuildAmenities,
        SecondaryNav::BuildUpgrades => ActivePanel::Upgrades,
        SecondaryNav::StrategyPricing => ActivePanel::Pricing,
        SecondaryNav::StrategyPower => ActivePanel::StrategyPower,
        SecondaryNav::StrategyOpex => ActivePanel::Opex,
        SecondaryNav::StatsSummary => ActivePanel::Summary,
        SecondaryNav::StatsPower => ActivePanel::Power,
        SecondaryNav::RentLocations => ActivePanel::Rent,
        SecondaryNav::StatsOperations => ActivePanel::Operations,
    };

    for (content, mut node) in &mut panels {
        node.display = if content.0 == active {
            Display::Flex
        } else {
            Display::None
        };
    }
}

// ============ Plugin ============

/// Plugin that manages all sidebar UI
pub struct SidebarPlugin;

impl Plugin for SidebarPlugin {
    fn build(&self, app: &mut App) {
        // Initialize resources
        app.init_resource::<NavigationState>();
        app.init_resource::<ActivePanel>(); // Legacy - will be removed

        // Update systems run when game is visible
        app.add_systems(
            Update,
            (
                // Navigation
                handle_secondary_tab_clicks,
                update_secondary_tab_row,
                sync_secondary_tab_colors,
                update_panel_visibility_from_nav,
                // Strategy panel updates
                update_strategy_panel_values,
                update_slider_fill_widths,
                handle_strategy_panel_buttons,
                update_summary_panel_values,
                handle_upgrade_purchases,
                update_upgrade_button_states,
                // BESS mode / peak shave labels
                update_bess_mode_label,
                update_peak_shave_label,
                // Warranty labels
                update_warranty_labels,
                // Dynamic pricing
                update_dynamic_pricing_labels,
                update_dynamic_pricing_visibility,
                // Lock / hack overlay updates
                (
                    update_power_lock_overlay,
                    update_opex_lock_overlay,
                    update_hack_overlay_visibility,
                ),
                // Visual disabled state for upgrade-gated controls
                update_maintenance_control_visual_state,
                update_power_control_visual_state,
                // Info button toggle for help text
                handle_info_button_clicks,
            )
                .chain()
                .in_set(crate::systems::GameSystemSet::UiUpdate)
                .run_if(is_game_visible),
        );

        app.add_systems(
            Update,
            (
                // Build panel updates
                handle_build_tool_buttons,
                update_build_tool_button_colors,
                update_utility_max_label,
                // Operations panel updates
                update_operations_panel,
                handle_fault_row_clicks,
                operations_panel::handle_view_ledger_button,
                // Rent panel updates
                update_rent_panel,
                // Start Day
                handle_start_day_button,
                update_start_day_button,
                animate_start_day_pulse,
            )
                .chain()
                .in_set(crate::systems::GameSystemSet::UiUpdate)
                .run_if(is_game_visible),
        );

        // Power panel updates (split into multiple systems due to parameter limits)
        app.add_systems(
            Update,
            (
                update_power_panel_basic,
                update_power_panel_capacity,
                update_power_panel_resources,
                update_power_threshold_bar,
                update_solar_bar,
                update_battery_bar,
            )
                .in_set(crate::systems::GameSystemSet::UiUpdate)
                .run_if(is_game_visible),
        );
    }
}
