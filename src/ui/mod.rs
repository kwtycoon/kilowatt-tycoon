//! UI systems and components
//!
//! UI systems run during Playing/Paused states:
//! - HUD, overlay, builder panel: Run during Playing/Paused states
//! - Speech bubbles: Run during Playing state only
//! - Radial menu: Tower-defense style charger selection UI
//!
//! # Layout Architecture
//!
//! The main game UI uses a unified hierarchical layout rooted at `HudRoot`:
//! - Top Bar (cash, revenue, time, reputation, speed controls)
//! - Top Nav (panel switcher tabs) - uses Display toggling for layout flow
//! - Middle Area (Row)
//!   - Strategy Sidebar (left) - uses Display toggling for layout flow
//!   - Game View Spacer (center, flex_grow)
//!   - Right Toolbar (toggle buttons)
//! - Bottom Bar (power/utility info)
//!
//! Floating overlays (power panel, radial menu) use absolute positioning.

pub mod achievement_modal;
pub mod demand_toasts;
pub mod fire_toasts;
pub mod hud;
pub mod leaderboard_modal;
pub mod leaderboard_systems;
pub mod ledger_modal;
pub mod overlay;
pub mod power_panel;
pub mod radial_menu;
pub mod sidebar;
pub mod site_tabs;
pub mod speech_bubbles;
pub mod template_picker;
pub mod toast;
pub mod top_nav;
pub mod tutorial;

use bevy::prelude::*;

use crate::states::{AppState, is_game_visible};

pub use achievement_modal::*;
pub use hud::*;
pub use leaderboard_modal::*;
pub use leaderboard_systems::*;
pub use ledger_modal::*;
pub use overlay::*;
pub use power_panel::*;
pub use radial_menu::*;
pub use sidebar::*;
pub use site_tabs::*;
pub use speech_bubbles::*;
pub use template_picker::*;
pub use toast::*;
pub use top_nav::*;
pub use tutorial::*;

/// Plugin for UI systems
pub struct UiPlugin;

impl Plugin for UiPlugin {
    fn build(&self, app: &mut App) {
        // Initialize persistent UI resources
        app.init_resource::<ActivePanel>()
            .init_resource::<PowerPanelState>()
            .init_resource::<RentCarouselState>()
            .init_resource::<rent_panel::RentPanelDirty>()
            .init_resource::<TutorialFaultInjected>()
            .init_resource::<GifAnimationFrames>()
            .init_resource::<LeaderboardModalState>()
            .init_resource::<AchievementModalState>()
            .init_resource::<LedgerModalState>();

        // Main game UI setup - runs when entering Playing state
        // Initialization + HUD + character selection overlay (if first play)
        app.add_systems(
            OnEnter(AppState::Playing),
            (
                initialize_game_on_first_play,
                crate::systems::site_roots::spawn_missing_site_roots,
                setup_hud,
                setup_overlay,
                setup_power_panel,
                setup_top_nav,
                setup_tutorial,
                toast::setup_toast_container,
                crate::states::setup_character_selection,
            )
                .chain(),
        );

        // Game UI updates - run when game is visible (Playing, Paused, GameOver)
        // Split into multiple add_systems calls to avoid tuple size limits
        app.add_systems(
            Update,
            (
                update_hud,
                sync_player_name_label,
                sync_player_avatar_image,
                update_overlay,
                handle_speed_buttons,
                sync_speed_button_colors,
                update_effective_price_badge,
                update_spot_price_badge,
                handle_overlay_buttons,
                // Tutorial systems
                update_tutorial_visibility,
                position_tutorial_pointer,
                update_tutorial_arrow_direction,
                handle_tutorial_buttons,
                update_tutorial_button_colors,
                check_tutorial_progress,
                inject_tutorial_fault,
                update_tutorial_highlights,
                manage_tutorial_highlights,
            )
                .in_set(crate::systems::GameSystemSet::UiUpdate)
                .run_if(is_game_visible),
        );

        app.add_systems(
            Update,
            (
                // Speech bubble systems (drivers)
                spawn_speech_bubbles,
                update_speech_bubble_positions,
                cleanup_speech_bubbles,
                // Speech bubble systems (technicians)
                spawn_technician_speech_bubbles,
                update_technician_speech_bubble_positions,
                cleanup_technician_speech_bubbles,
                // Top nav systems
                update_top_nav_visibility,
                handle_primary_nav_clicks,
                top_nav::handle_leaderboard_button_click,
                top_nav::handle_ledger_button_click,
                sync_primary_nav_button_colors,
                update_weather_display,
                handle_temperature_tooltip,
                show_tooltip_on_cold_site_switch,
                update_tooltip_auto_hide,
                update_news_ticker,
                // Radial menu systems - ordered to prevent click-through bugs:
                // 1. Handle button clicks first (may set selected = None)
                // 2. Handle dismiss layer clicks (may set selected = None)
                // 3. Close menu if deselected
                // 4. Only then spawn new menu if something selected
                (
                    handle_radial_menu_buttons,
                    handle_dismiss_layer_click,
                    close_radial_menu_on_deselect,
                    spawn_radial_menu,
                )
                    .chain(),
                update_button_flash,
                update_action_pulse,
                update_radial_menu_data,
            )
                .in_set(crate::systems::GameSystemSet::UiUpdate)
                .run_if(is_game_visible),
        );

        // Unit toggle + Sound toggle + GIF animation systems
        app.add_systems(
            Update,
            (
                top_nav::handle_unit_toggle_button_click,
                top_nav::handle_sound_toggle_button_click,
                load_gif_frames,
                update_gif_animations,
            )
                .in_set(crate::systems::GameSystemSet::UiUpdate)
                .run_if(is_game_visible),
        );

        app.add_systems(
            Update,
            (
                // Toast notification systems
                spawn_fault_toasts,
                spawn_repair_failed_toasts,
                toast::spawn_achievement_toasts,
                update_toasts,
                handle_toast_clicks,
                // Demand warning toast systems
                demand_toasts::spawn_demand_burden_toast,
                demand_toasts::update_toast_action_button_styles,
                demand_toasts::handle_reduce_load_button,
                demand_toasts::handle_dismiss_warning_button,
                // Transformer fire toast systems
                fire_toasts::spawn_overload_warning_toast,
                fire_toasts::spawn_fire_started_toast,
                fire_toasts::handle_shed_load_button,
                fire_toasts::handle_dismiss_overload_button,
                fire_toasts::update_fire_toast_button_styles,
                // Site tabs systems
                update_site_tabs,
                handle_site_tab_clicks,
                handle_sell_site_clicks,
                // Rent panel systems
                handle_carousel_navigation,
                handle_rent_button,
            )
                .in_set(crate::systems::GameSystemSet::UiUpdate)
                .run_if(is_game_visible),
        );

        app.add_systems(
            Update,
            (
                // Leaderboard systems
                leaderboard_systems::fetch_leaderboard_on_modal_open,
                leaderboard_systems::poll_leaderboard_fetch_tasks,
                leaderboard_systems::poll_score_submit_tasks,
                // Leaderboard modal systems
                leaderboard_modal::spawn_leaderboard_modal,
                leaderboard_modal::despawn_leaderboard_modal,
                leaderboard_modal::handle_leaderboard_close_button,
                leaderboard_modal::update_leaderboard_modal_content,
            )
                .in_set(crate::systems::GameSystemSet::UiUpdate)
                .run_if(is_game_visible),
        );

        // Achievement modal systems - chained to guarantee ordering:
        // 1. Input handlers mutate AchievementModalState
        // 2. Spawn/despawn react to the updated state
        // 3. Content update runs last
        app.add_systems(
            Update,
            (
                (
                    hud::handle_achievement_badge_click,
                    achievement_modal::handle_achievement_close_button,
                    achievement_modal::handle_achievement_escape_key,
                ),
                (
                    achievement_modal::spawn_achievement_modal,
                    achievement_modal::despawn_achievement_modal,
                ),
                achievement_modal::update_achievement_modal_content,
            )
                .chain()
                .in_set(crate::systems::GameSystemSet::UiUpdate)
                .run_if(is_game_visible),
        );

        // Ledger modal systems - chained for correct ordering
        app.add_systems(
            Update,
            (
                (
                    ledger_modal::handle_ledger_close_button,
                    ledger_modal::handle_ledger_tab_buttons,
                    ledger_modal::handle_ledger_keyboard,
                ),
                (
                    ledger_modal::spawn_ledger_modal,
                    ledger_modal::despawn_ledger_modal,
                ),
                (
                    ledger_modal::update_tab_visuals,
                    ledger_modal::update_ledger_content,
                ),
            )
                .chain()
                .in_set(crate::systems::GameSystemSet::UiUpdate)
                .run_if(is_game_visible),
        );

        // Sidebar UI (replaces old strategy_panel and builder_panel)
        app.add_plugins(SidebarPlugin);
    }
}
