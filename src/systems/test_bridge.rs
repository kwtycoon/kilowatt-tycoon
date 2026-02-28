//! Read-only test bridge for browser automation.
//!
//! Exposes game state and UI element positions to JavaScript via
//! `window.__kwtycoon_bridge`, updated each frame by a Bevy system.
//!
//! **This module never mutates game state or injects input events.**
//! All test interactions still go through real browser input (touch, mouse, keyboard).

use std::collections::HashMap;

use bevy::prelude::*;
use bevy::ui::UiGlobalTransform;
use serde::Serialize;
use wasm_bindgen::JsValue;

use crate::resources::{
    BuildState, BuildTool, GameClock, GameState, MultiSiteManager, SiteGrid, TileContent,
    TutorialState,
};
use crate::states::AppState;
use crate::systems::WorldCamera;
use crate::ui::hud::SpeedButton;
use crate::ui::sidebar::{BuildToolButton, StartDayButton};
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
/// `UiGlobalTransform.translation` is the node center in logical (CSS) pixels.
/// `ComputedNode::size()` is in physical pixels; divide by scale factor to get logical.
fn to_rect(ugt: &UiGlobalTransform, cn: &ComputedNode) -> ElementRect {
    let center = ugt.translation;
    let phys_size = cn.size();
    let isf = cn.inverse_scale_factor();
    let w = phys_size.x * isf;
    let h = phys_size.y * isf;
    ElementRect {
        x: center.x - w / 2.0,
        y: center.y - h / 2.0,
        width: w,
        height: h,
    }
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
    mut bridge: ResMut<TestBridgeState>,
    next_btns: Query<(&UiGlobalTransform, &ComputedNode), With<crate::states::NextButton>>,
    start_btns: Query<(&UiGlobalTransform, &ComputedNode), With<crate::states::StartButton>>,
    tut_btns: Query<
        (
            &UiGlobalTransform,
            &ComputedNode,
            Option<&TutorialNextButton>,
        ),
        Or<(With<TutorialNextButton>, With<TutorialSkipButton>)>,
    >,
    start_day: Query<(&UiGlobalTransform, &ComputedNode), With<StartDayButton>>,
    speed_btns: Query<(&UiGlobalTransform, &ComputedNode, &SpeedButton)>,
    day_end_continue: Query<
        (&UiGlobalTransform, &ComputedNode),
        With<crate::states::DayEndContinueButton>,
    >,
    build_tool_btns: Query<(&UiGlobalTransform, &ComputedNode, &BuildToolButton)>,
    world_camera: Query<(&Camera, &GlobalTransform), With<WorldCamera>>,
) {
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
        elements: HashMap::new(),
    };

    // Character setup buttons
    for (ugt, cn) in &next_btns {
        snapshot
            .elements
            .insert("NextButton".into(), to_rect(ugt, cn));
    }
    for (ugt, cn) in &start_btns {
        snapshot
            .elements
            .insert("StartButton".into(), to_rect(ugt, cn));
    }

    // Tutorial buttons (combined query, discriminated by marker presence)
    for (ugt, cn, is_next) in &tut_btns {
        let name = if is_next.is_some() {
            "TutorialNextButton"
        } else {
            "TutorialSkipButton"
        };
        snapshot.elements.insert(name.into(), to_rect(ugt, cn));
    }

    // HUD buttons
    for (ugt, cn) in &start_day {
        snapshot
            .elements
            .insert("StartDayButton".into(), to_rect(ugt, cn));
    }
    for (ugt, cn, speed) in &speed_btns {
        let name = match speed.0 {
            crate::resources::GameSpeed::Normal => "SpeedButton_Normal",
            crate::resources::GameSpeed::Fast => "SpeedButton_Fast",
            crate::resources::GameSpeed::Paused => "SpeedButton_Paused",
        };
        snapshot.elements.insert(name.into(), to_rect(ugt, cn));
    }
    for (ugt, cn) in &day_end_continue {
        snapshot
            .elements
            .insert("DayEndContinueButton".into(), to_rect(ugt, cn));
    }

    // Build tool buttons
    for (ugt, cn, tool) in &build_tool_btns {
        let name = format!("BuildTool_{:?}", tool.tool);
        snapshot.elements.insert(name, to_rect(ugt, cn));
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
