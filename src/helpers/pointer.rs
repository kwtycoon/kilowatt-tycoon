//! Unified pointer abstraction that works for both mouse and touch input.
//!
//! On desktop: uses `window.cursor_position()` + `ButtonInput<MouseButton>`.
//! On touch: uses `Res<Touches>` for single-finger interactions.
//!
//! When 2+ fingers are active, game interactions are suppressed so that
//! two-finger camera gestures (pan/pinch-to-zoom) don't accidentally place tiles.

use bevy::input::mouse::MouseButton;
use bevy::input::touch::Touches;
use bevy::prelude::*;
use bevy::window::PrimaryWindow;

/// Plugin that registers the `PointerState` resource and the system that updates it.
pub struct PointerPlugin;

impl Plugin for PointerPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<GamePointer>()
            .add_systems(PreUpdate, update_pointer_state);
    }
}

/// Unified input state representing a single pointer (mouse cursor or primary touch finger).
///
/// Updated every frame in `PreUpdate` so all `Update` systems see a consistent snapshot.
#[derive(Resource, Default, Debug)]
pub struct GamePointer {
    /// Current screen-space position of the pointer, or `None` when no pointer is active.
    pub screen_position: Option<Vec2>,
    /// True on the frame the pointer was first pressed / finger first touched.
    pub just_pressed: bool,
    /// True while the pointer button is held / finger is on the screen.
    pub pressed: bool,
    /// True on the frame the pointer was released / finger lifted.
    pub just_released: bool,
    /// True when input is coming from a touch screen rather than a mouse.
    pub is_touch: bool,
}

/// Updates `GamePointer` from mouse or touch inputs each frame.
///
/// Priority:
/// 1. If 2+ fingers are touching, clear all state (two-finger gestures handled by camera system).
/// 2. Mouse cursor position + left mouse button (desktop).
/// 3. Primary touch finger (single touch on mobile/tablet).
pub fn update_pointer_state(
    mut pointer: ResMut<GamePointer>,
    windows: Query<&Window, With<PrimaryWindow>>,
    mouse_button: Res<ButtonInput<MouseButton>>,
    touches: Res<Touches>,
) {
    let touch_count = touches.iter().count();

    // Two-finger gestures: suppress game interactions, camera handles this separately.
    if touch_count >= 2 {
        *pointer = GamePointer::default();
        return;
    }

    // Try mouse first (desktop path).
    let mouse_position = windows.single().ok().and_then(|w| w.cursor_position());

    if let Some(pos) = mouse_position {
        *pointer = GamePointer {
            screen_position: Some(pos),
            just_pressed: mouse_button.just_pressed(MouseButton::Left),
            pressed: mouse_button.pressed(MouseButton::Left),
            just_released: mouse_button.just_released(MouseButton::Left),
            is_touch: false,
        };
        return;
    }

    // Fall back to single touch finger.
    if touch_count == 1
        && let Some(finger) = touches.iter().next()
    {
        let id = finger.id();
        *pointer = GamePointer {
            screen_position: Some(finger.position()),
            just_pressed: touches.just_pressed(id),
            // A finger is "pressed" whenever it is on the screen.
            pressed: true,
            just_released: false,
            is_touch: true,
        };
        return;
    }

    // Check for just-released touch (finger lifted; touch no longer in iter()).
    let just_released_touch = touches.iter_just_released().next().is_some();
    if just_released_touch {
        // Keep last known position but signal release.
        *pointer = GamePointer {
            screen_position: pointer.screen_position,
            just_pressed: false,
            pressed: false,
            just_released: true,
            is_touch: true,
        };
        return;
    }

    // No active input.
    *pointer = GamePointer::default();
}
