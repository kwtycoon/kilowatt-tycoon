//! Click-to-select interaction system

use bevy::prelude::*;
use bevy::window::PrimaryWindow;

use crate::components::charger::{Charger, ChargerSprite};
use crate::helpers::GamePointer;
use crate::resources::{BuildState, BuildTool, SelectedChargerEntity};
use crate::systems::WorldCamera;
use crate::ui::radial_menu::RadialMenuDismissLayer;

/// Handle clicks/taps to select chargers.
/// Left-click or single tap: Select chargers when no UI is blocking.
/// Right-click (desktop only): Always select chargers (RTS-style interaction).
pub fn click_to_select_charger(
    mouse: Res<ButtonInput<MouseButton>>,
    pointer: Res<GamePointer>,
    cameras: Query<(&Camera, &GlobalTransform), With<WorldCamera>>,
    charger_sprites: Query<(&ChargerSprite, &GlobalTransform, &Sprite)>,
    chargers: Query<Entity, With<Charger>>,
    mut selected: ResMut<SelectedChargerEntity>,
    mut build_state: ResMut<BuildState>,
    // Exclude radial menu dismiss layer - it handles its own click-to-deselect
    ui_interactions: Query<&Interaction, Without<RadialMenuDismissLayer>>,
    images: Res<Assets<Image>>,
    windows: Query<&Window, With<PrimaryWindow>>,
) {
    // Pointer tap (mouse left-click or single touch) OR desktop right-click
    let is_pointer_tap = pointer.just_pressed;
    let is_right_click = mouse.just_pressed(MouseButton::Right);

    if !is_pointer_tap && !is_right_click {
        return;
    }

    // Right-click dismisses build tool selection (chargers/infra/amenities/sell)
    if is_right_click {
        build_state.selected_tool = BuildTool::Select;
    }

    // Block taps if any UI element is being hovered/pressed.
    // Allow right-clicks through (RTS-style, desktop only).
    if is_pointer_tap && !is_right_click {
        for interaction in &ui_interactions {
            if *interaction != Interaction::None {
                return;
            }
        }
    }

    // Resolve the screen position: pointer state for taps, mouse cursor for right-clicks.
    let cursor_position = if is_right_click && !pointer.is_touch {
        // Desktop right-click: use actual cursor position from the window.
        windows.single().ok().and_then(|w| w.cursor_position())
    } else {
        pointer.screen_position
    };

    let Some(cursor_position) = cursor_position else {
        return;
    };

    let Ok((camera, camera_transform)) = cameras.single() else {
        return;
    };

    // Convert screen position to world position
    let Ok(world_position) = camera.viewport_to_world_2d(camera_transform, cursor_position) else {
        return;
    };

    // Check if clicking on any charger sprite
    let mut clicked_charger: Option<Entity> = None;

    for (charger_sprite, global_transform, sprite) in &charger_sprites {
        // Get the base size from custom_size, or fall back to the image's native size
        let base_size = if let Some(custom) = sprite.custom_size {
            custom
        } else {
            // Look up the image asset to get its native pixel size
            let Some(image) = images.get(&sprite.image) else {
                continue; // Image not loaded yet, skip this sprite
            };
            image.size().as_vec2()
        };

        // Apply global scale to get world-space size
        let (global_scale, _, _) = global_transform.to_scale_rotation_translation();
        let world_size = base_size * global_scale.truncate().abs();

        let half_size = world_size / 2.0;
        // Use GlobalTransform for proper world position
        let pos = global_transform.translation().truncate();

        // Simple AABB collision
        if world_position.x >= pos.x - half_size.x
            && world_position.x <= pos.x + half_size.x
            && world_position.y >= pos.y - half_size.y
            && world_position.y <= pos.y + half_size.y
        {
            // Verify the charger entity still exists
            if chargers.get(charger_sprite.charger_entity).is_ok() {
                clicked_charger = Some(charger_sprite.charger_entity);
                break;
            }
        }
    }

    if let Some(entity) = clicked_charger {
        if selected.0 == Some(entity) {
            // Clicking same charger - deselect
            selected.0 = None;
        } else {
            selected.0 = Some(entity);
        }
    } else {
        // Clicked on nothing - deselect
        if selected.0.is_some() {
            selected.0 = None;
        }
    }
}

/// Handle keyboard shortcuts for speed (keep these working)
pub fn keyboard_shortcuts(
    keyboard: Res<ButtonInput<KeyCode>>,
    mut game_clock: ResMut<crate::resources::GameClock>,
) {
    if keyboard.just_pressed(KeyCode::Space) {
        game_clock.toggle_pause();
        info!("Pause toggled via spacebar");
    }
}
