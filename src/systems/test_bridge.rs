//! Read-only test bridge for browser automation.
//!
//! Exposes game state and UI element positions to JavaScript via
//! `window.__kwtycoon_bridge`, updated each frame by a Bevy system.
//!
//! **This module never mutates game state or injects input events.**
//! All test interactions still go through real browser input (touch, mouse, keyboard).

use std::collections::HashMap;

use bevy::ecs::system::SystemParam;
use bevy::prelude::*;
use bevy::ui::UiGlobalTransform;
use serde::Serialize;
use wasm_bindgen::JsValue;

use crate::resources::{
    BuildState, BuildTool, GameClock, GameState, MultiSiteManager, SiteGrid, TileContent,
    TutorialState,
};
use crate::states::{AppState, DayEndScrollBody, KpiToggleButton};
use crate::systems::WorldCamera;
use crate::ui::hud::SpeedButton;
use crate::ui::sidebar::rent_panel::{CarouselButton, RentCarouselState, RentSiteButton};
use crate::ui::sidebar::{BuildToolButton, PrimaryNav, SecondaryTabButton, StartDayButton};
use crate::ui::site_tabs::SiteTab;
use crate::ui::top_nav::PrimaryNavButton;
use crate::ui::tutorial::{TutorialNextButton, TutorialSkipButton};

#[derive(Serialize, Clone, Debug)]
struct ElementRect {
    x: f32,
    y: f32,
    width: f32,
    height: f32,
}

#[derive(Serialize, Clone, Debug, Default)]
struct BridgeSnapshot {
    app_state: String,
    tutorial_step: Option<String>,
    day_number: u32,
    cash: f32,
    game_time: f32,
    selected_build_tool: Option<String>,
    day_end_scroll_y: Option<f32>,
    num_owned_sites: usize,
    viewed_site_id: Option<u32>,
    carousel_index: usize,
    elements: HashMap<String, ElementRect>,
}

#[derive(Resource, Default)]
pub struct TestBridgeState {
    json: String,
}

fn push_to_window(json: &str) {
    let Some(window) = web_sys::window() else {
        return;
    };
    let Ok(parsed) = js_sys::JSON::parse(json) else {
        return;
    };
    let _ = js_sys::Reflect::set(&window, &JsValue::from_str("__kwtycoon_bridge"), &parsed);
}

/// Convert a UI node's `UiGlobalTransform` + `ComputedNode` into a CSS-pixel rect.
///
/// Both `UiGlobalTransform.translation` and `ComputedNode::size()` are in
/// physical pixels. Playwright operates in CSS pixels, so we divide both by
/// the window DPR (`camera.target_scaling_factor()`).
///
/// This ensures the rect is a true CSS bounding box, which matters when
/// tests click at OFFSETS within a container (e.g. carousel buttons within
/// the rent panel). Using `inverse_scale_factor` (1 / (DPR * UiScale))
/// for size would produce Val::Px units -- correct for center clicks but
/// wrong for offset calculations on viewports with UiScale != 1.
fn to_rect(ugt: &UiGlobalTransform, cn: &ComputedNode, dpr: f32) -> ElementRect {
    let cx = ugt.translation.x / dpr;
    let cy = ugt.translation.y / dpr;
    let w = cn.size().x / dpr;
    let h = cn.size().y / dpr;
    ElementRect {
        x: cx - w / 2.0,
        y: cy - h / 2.0,
        width: w,
        height: h,
    }
}

/// Bundled UI element queries to keep the system under Bevy's 16-param limit.
#[derive(SystemParam)]
pub struct UiElementQueries<'w, 's> {
    next_btns: Query<
        'w,
        's,
        (&'static UiGlobalTransform, &'static ComputedNode),
        With<crate::states::NextButton>,
    >,
    start_btns: Query<
        'w,
        's,
        (&'static UiGlobalTransform, &'static ComputedNode),
        With<crate::states::StartButton>,
    >,
    tut_btns: Query<
        'w,
        's,
        (
            &'static UiGlobalTransform,
            &'static ComputedNode,
            Option<&'static TutorialNextButton>,
        ),
        Or<(With<TutorialNextButton>, With<TutorialSkipButton>)>,
    >,
    start_day:
        Query<'w, 's, (&'static UiGlobalTransform, &'static ComputedNode), With<StartDayButton>>,
    speed_btns: Query<
        'w,
        's,
        (
            &'static UiGlobalTransform,
            &'static ComputedNode,
            &'static SpeedButton,
        ),
    >,
    day_end_btns: Query<
        'w,
        's,
        (
            &'static UiGlobalTransform,
            &'static ComputedNode,
            Option<&'static crate::states::DayEndContinueButton>,
            Option<&'static KpiToggleButton>,
        ),
        Or<(
            With<crate::states::DayEndContinueButton>,
            With<KpiToggleButton>,
        )>,
    >,
    build_tool_btns: Query<
        'w,
        's,
        (
            &'static UiGlobalTransform,
            &'static ComputedNode,
            &'static BuildToolButton,
        ),
    >,
    secondary_tabs: Query<
        'w,
        's,
        (
            &'static UiGlobalTransform,
            &'static ComputedNode,
            &'static SecondaryTabButton,
        ),
    >,
    scroll_body: Query<
        'w,
        's,
        (
            &'static UiGlobalTransform,
            &'static ComputedNode,
            &'static ScrollPosition,
        ),
        With<DayEndScrollBody>,
    >,
    nav_btns: Query<
        'w,
        's,
        (
            &'static UiGlobalTransform,
            &'static ComputedNode,
            &'static PrimaryNavButton,
        ),
    >,
    carousel_btns: Query<
        'w,
        's,
        (
            &'static UiGlobalTransform,
            &'static ComputedNode,
            &'static CarouselButton,
        ),
    >,
    rent_btns:
        Query<'w, 's, (&'static UiGlobalTransform, &'static ComputedNode), With<RentSiteButton>>,
    rent_panel: Query<
        'w,
        's,
        (&'static UiGlobalTransform, &'static ComputedNode),
        With<crate::ui::sidebar::rent_panel::RentPanel>,
    >,
    site_tabs: Query<
        'w,
        's,
        (
            &'static UiGlobalTransform,
            &'static ComputedNode,
            &'static SiteTab,
        ),
    >,
}

/// Bevy system that collects game state and element positions each frame,
/// then writes the result to `window.__kwtycoon_bridge` as a JS object.
#[allow(clippy::too_many_arguments, clippy::type_complexity)]
pub fn update_test_bridge(
    app_state: Res<State<AppState>>,
    tutorial: Res<TutorialState>,
    game_clock: Res<GameClock>,
    game_state: Res<GameState>,
    build_state: Res<BuildState>,
    multi_site: Res<MultiSiteManager>,
    carousel: Res<RentCarouselState>,
    mut bridge: ResMut<TestBridgeState>,
    ui: UiElementQueries,
    world_camera: Query<(&Camera, &GlobalTransform), With<WorldCamera>>,
) {
    let dpr = world_camera
        .iter()
        .next()
        .and_then(|(cam, _)| cam.target_scaling_factor())
        .unwrap_or(1.0);
    let mut snapshot = BridgeSnapshot {
        app_state: format!("{:?}", *app_state.get()),
        tutorial_step: tutorial.current_step.map(|s| format!("{s:?}")),
        day_number: game_clock.day,
        cash: game_state.cash,
        game_time: game_clock.game_time,
        selected_build_tool: if build_state.selected_tool != BuildTool::Select {
            Some(format!("{:?}", build_state.selected_tool))
        } else {
            None
        },
        day_end_scroll_y: None,
        num_owned_sites: multi_site.owned_sites.len(),
        viewed_site_id: multi_site.viewed_site_id.map(|id| id.0),
        carousel_index: carousel.current_index,
        elements: HashMap::new(),
    };

    // Character setup buttons
    for (ugt, cn) in &ui.next_btns {
        snapshot
            .elements
            .insert("NextButton".into(), to_rect(ugt, cn, dpr));
    }
    for (ugt, cn) in &ui.start_btns {
        snapshot
            .elements
            .insert("StartButton".into(), to_rect(ugt, cn, dpr));
    }

    // Tutorial buttons (combined query, discriminated by marker presence)
    for (ugt, cn, is_next) in &ui.tut_btns {
        let name = if is_next.is_some() {
            "TutorialNextButton"
        } else {
            "TutorialSkipButton"
        };
        snapshot.elements.insert(name.into(), to_rect(ugt, cn, dpr));
    }

    // HUD buttons
    for (ugt, cn) in &ui.start_day {
        snapshot
            .elements
            .insert("StartDayButton".into(), to_rect(ugt, cn, dpr));
    }
    for (ugt, cn, speed) in &ui.speed_btns {
        let name = match speed.0 {
            crate::resources::GameSpeed::Normal => "SpeedButton_Normal",
            crate::resources::GameSpeed::Fast => "SpeedButton_Fast",
            crate::resources::GameSpeed::Paused => "SpeedButton_Paused",
        };
        snapshot.elements.insert(name.into(), to_rect(ugt, cn, dpr));
    }
    for (ugt, cn, is_continue, is_kpi_toggle) in &ui.day_end_btns {
        if is_continue.is_some() {
            snapshot
                .elements
                .insert("DayEndContinueButton".into(), to_rect(ugt, cn, dpr));
        }
        if is_kpi_toggle.is_some() {
            snapshot
                .elements
                .insert("KpiToggleButton".into(), to_rect(ugt, cn, dpr));
        }
    }

    // Build tool buttons
    for (ugt, cn, tool) in &ui.build_tool_btns {
        let name = format!("BuildTool_{:?}", tool.tool);
        snapshot.elements.insert(name, to_rect(ugt, cn, dpr));
    }

    // Secondary navigation tabs (Chargers, Infra, Amenities, Upgrades, etc.)
    for (ugt, cn, tab) in &ui.secondary_tabs {
        let name = format!("SubTab_{}", tab.tab.display_name());
        snapshot.elements.insert(name, to_rect(ugt, cn, dpr));
    }

    // Day-end scroll body
    for (ugt, cn, scroll_pos) in &ui.scroll_body {
        snapshot
            .elements
            .insert("DayEndScrollBody".into(), to_rect(ugt, cn, dpr));
        snapshot.day_end_scroll_y = Some(scroll_pos.y);
    }

    // Primary navigation buttons (Location, Build, Strategy, Stats)
    for (ugt, cn, nav_btn) in &ui.nav_btns {
        let name = match nav_btn.nav {
            PrimaryNav::Rent => "NavButton_Rent",
            PrimaryNav::Build => "NavButton_Build",
            PrimaryNav::Strategy => "NavButton_Strategy",
            PrimaryNav::Stats => "NavButton_Stats",
        };
        snapshot.elements.insert(name.into(), to_rect(ugt, cn, dpr));
    }

    // Rent panel carousel buttons
    for (ugt, cn, carousel) in &ui.carousel_btns {
        let name = match carousel {
            CarouselButton::Previous => "CarouselButton_Previous",
            CarouselButton::Next => "CarouselButton_Next",
        };
        snapshot.elements.insert(name.into(), to_rect(ugt, cn, dpr));
    }

    // Rent panel container (for debugging layout)
    for (ugt, cn) in &ui.rent_panel {
        snapshot
            .elements
            .insert("RentPanel".into(), to_rect(ugt, cn, dpr));
    }

    // Rent site button
    for (ugt, cn) in &ui.rent_btns {
        snapshot
            .elements
            .insert("RentSiteButton".into(), to_rect(ugt, cn, dpr));
    }

    // Site switcher tabs (sorted by SiteId for stable indexing)
    {
        let mut tabs: Vec<_> = ui.site_tabs.iter().collect();
        tabs.sort_by_key(|(_, _, tab)| tab.site_id);
        for (i, (ugt, cn, _)) in tabs.iter().enumerate() {
            snapshot
                .elements
                .insert(format!("SiteTab_{i}"), to_rect(ugt, cn, dpr));
        }
    }

    // Grid placement hints: expose valid charger/transformer positions as
    // screen-space rects in CSS pixels so Playwright can click them.
    // `world_to_viewport` already returns logical (CSS) window coordinates.
    if let Ok((camera, camera_gt)) = world_camera.single() {
        if let Some(site) = multi_site.active_site() {
            add_placement_hints(
                &mut snapshot.elements,
                &site.grid,
                site.world_offset(),
                camera,
                camera_gt,
            );
        }
    }

    if let Ok(json) = serde_json::to_string(&snapshot) {
        if json != bridge.json {
            bridge.json.clone_from(&json);
            push_to_window(&json);
        }
    }
}

/// Scan the active site grid for valid charger and transformer placement
/// positions and insert them as screen-space `ElementRect`s in CSS pixels.
fn add_placement_hints(
    elements: &mut HashMap<String, ElementRect>,
    grid: &SiteGrid,
    site_offset: Vec2,
    camera: &Camera,
    camera_gt: &GlobalTransform,
) {
    let hint_size = 20.0;
    let mut charger_idx: usize = 0;
    let mut transformer_idx: usize = 0;

    for ((x, y), tile) in grid.iter_tiles() {
        if let Some((dx, dy)) = tile.content.charger_offset() {
            let pad_x = x + dx;
            let pad_y = y + dy;
            if !grid.is_valid(pad_x, pad_y) {
                continue;
            }
            let pad_content = grid.get_content(pad_x, pad_y);
            let pad_free = matches!(
                pad_content,
                TileContent::ChargerPad
                    | TileContent::Lot
                    | TileContent::Grass
                    | TileContent::Empty
                    | TileContent::WheelStop
            );
            let bay_free = tile.linked_charger_pad.is_none() && !tile.has_adjacent_charger;
            if pad_free && bay_free {
                let world = SiteGrid::grid_to_world(pad_x, pad_y) + site_offset;
                if let Ok(css) = camera.world_to_viewport(camera_gt, world.extend(0.0)) {
                    let name = format!("PlacementHint_Charger_{charger_idx}");
                    elements.insert(
                        name,
                        ElementRect {
                            x: css.x - hint_size / 2.0,
                            y: css.y - hint_size / 2.0,
                            width: hint_size,
                            height: hint_size,
                        },
                    );
                    charger_idx += 1;
                }
            }
        }

        if tile.content == TileContent::Grass && !tile.is_locked {
            if grid
                .can_place_footprint(x, y, crate::resources::StructureSize::TwoByTwo)
                .is_ok()
            {
                let tile_size = crate::resources::TILE_SIZE;
                let center = SiteGrid::grid_to_world(x, y)
                    + Vec2::new(tile_size / 2.0, tile_size / 2.0)
                    + site_offset;
                if let Ok(css) = camera.world_to_viewport(camera_gt, center.extend(0.0)) {
                    let name = format!("PlacementHint_Transformer_{transformer_idx}");
                    elements.insert(
                        name,
                        ElementRect {
                            x: css.x - hint_size / 2.0,
                            y: css.y - hint_size / 2.0,
                            width: hint_size,
                            height: hint_size,
                        },
                    );
                    transformer_idx += 1;
                }
            }
        }
    }
}
