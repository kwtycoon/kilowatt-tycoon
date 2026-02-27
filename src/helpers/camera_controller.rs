//! Camera controller for top-down strategy game view.
//!
//! Provides smooth pan and zoom controls for 2D cameras.
//!
//! # Usage
//!
//! ```rust,ignore
//! // Add the controller component to your camera
//! commands.spawn((
//!     Camera2d,
//!     CameraController::default(),
//! ));
//! ```

use bevy::input::touch::Touches;
use bevy::prelude::*;

use crate::states::AppState;

/// Plugin for camera controller functionality
pub struct CameraControllerPlugin;

impl Plugin for CameraControllerPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<TouchCameraState>().add_systems(
            Update,
            (camera_pan_system, touch_camera_system)
                .run_if(in_state(AppState::Playing).or(in_state(AppState::Paused))),
        );
    }
}

/// Tracks previous touch positions for delta-based pan/zoom.
#[derive(Resource, Default)]
struct TouchCameraState {
    /// Previous centroid of the two touch fingers (for pan delta).
    prev_centroid: Option<Vec2>,
    /// Previous distance between the two touch fingers (for pinch zoom).
    prev_distance: Option<f32>,
}

/// Camera controller component with configuration
#[derive(Component, Debug, Clone)]
pub struct CameraController {
    /// Pan speed in pixels per second
    pub pan_speed: f32,
    /// Keyboard pan speed multiplier
    pub keyboard_pan_speed: f32,
    /// Zoom speed (scale change per scroll unit)
    pub zoom_speed: f32,
    /// Minimum zoom level (most zoomed out)
    pub min_zoom: f32,
    /// Maximum zoom level (most zoomed in)
    pub max_zoom: f32,
    /// Enable keyboard panning (WASD/arrows)
    pub enable_keyboard_pan: bool,
    /// Enable scroll wheel zoom
    pub enable_scroll_zoom: bool,
    /// Bounds for camera position (min_x, min_y, max_x, max_y)
    pub bounds: Option<(f32, f32, f32, f32)>,
    /// Current zoom scale
    pub current_scale: f32,
}

impl Default for CameraController {
    fn default() -> Self {
        Self {
            pan_speed: 500.0,
            keyboard_pan_speed: 1.0,
            zoom_speed: 0.1,
            min_zoom: 0.5,
            max_zoom: 2.0,
            enable_keyboard_pan: true,
            enable_scroll_zoom: true,
            bounds: None,
            current_scale: 1.0,
        }
    }
}

impl CameraController {
    /// Create a controller with custom pan speed
    pub fn with_pan_speed(mut self, speed: f32) -> Self {
        self.pan_speed = speed;
        self
    }

    /// Set zoom limits
    pub fn with_zoom_limits(mut self, min: f32, max: f32) -> Self {
        self.min_zoom = min;
        self.max_zoom = max;
        self
    }

    /// Set camera bounds
    pub fn with_bounds(mut self, min_x: f32, min_y: f32, max_x: f32, max_y: f32) -> Self {
        self.bounds = Some((min_x, min_y, max_x, max_y));
        self
    }

    /// Disable all controls
    pub fn disabled() -> Self {
        Self {
            enable_keyboard_pan: false,
            enable_scroll_zoom: false,
            ..default()
        }
    }
}

/// System to handle camera panning via keyboard.
fn camera_pan_system(
    mut cameras: Query<(&mut Transform, &CameraController)>,
    keyboard: Res<ButtonInput<KeyCode>>,
    time: Res<Time>,
) {
    let delta_time = time.delta_secs();

    for (mut transform, controller) in &mut cameras {
        let mut movement = Vec2::ZERO;

        // Keyboard panning
        if controller.enable_keyboard_pan {
            if keyboard.pressed(KeyCode::KeyW) || keyboard.pressed(KeyCode::ArrowUp) {
                movement.y += 1.0;
            }
            if keyboard.pressed(KeyCode::KeyS) || keyboard.pressed(KeyCode::ArrowDown) {
                movement.y -= 1.0;
            }
            if keyboard.pressed(KeyCode::KeyA) || keyboard.pressed(KeyCode::ArrowLeft) {
                movement.x -= 1.0;
            }
            if keyboard.pressed(KeyCode::KeyD) || keyboard.pressed(KeyCode::ArrowRight) {
                movement.x += 1.0;
            }

            if movement != Vec2::ZERO {
                movement = movement.normalize()
                    * controller.pan_speed
                    * controller.keyboard_pan_speed
                    * delta_time
                    * controller.current_scale;
            }
        }

        // Apply movement
        if movement != Vec2::ZERO {
            transform.translation.x += movement.x;
            transform.translation.y += movement.y;

            // Clamp to bounds if set
            if let Some((min_x, min_y, max_x, max_y)) = controller.bounds {
                transform.translation.x = transform.translation.x.clamp(min_x, max_x);
                transform.translation.y = transform.translation.y.clamp(min_y, max_y);
            }
        }
    }
}

/// Two-finger pan and pinch-to-zoom for touch/tablet.
///
/// Only activates when exactly two fingers are on the screen.  The centroid
/// delta drives camera translation; the distance ratio drives zoom.
fn touch_camera_system(
    mut cameras: Query<(&mut Transform, &mut CameraController)>,
    touches: Res<Touches>,
    mut state: ResMut<TouchCameraState>,
) {
    let active: Vec<Vec2> = touches.iter().map(|t| t.position()).collect();

    if active.len() < 2 {
        // Reset tracking when fingers are lifted.
        state.prev_centroid = None;
        state.prev_distance = None;
        return;
    }

    // Use only the first two fingers.
    let a = active[0];
    let b = active[1];

    let centroid = (a + b) * 0.5;
    let distance = a.distance(b);

    if let (Some(prev_centroid), Some(prev_distance)) = (state.prev_centroid, state.prev_distance) {
        let centroid_delta = centroid - prev_centroid;
        let distance_ratio = if prev_distance > 0.0 {
            distance / prev_distance
        } else {
            1.0
        };

        for (mut transform, mut controller) in &mut cameras {
            // Pan: move camera opposite to finger movement (drag the world).
            // Scale by current zoom so that the pan speed feels consistent.
            transform.translation.x -= centroid_delta.x * controller.current_scale;
            transform.translation.y += centroid_delta.y * controller.current_scale;

            // Clamp to bounds if set
            if let Some((min_x, min_y, max_x, max_y)) = controller.bounds {
                transform.translation.x = transform.translation.x.clamp(min_x, max_x);
                transform.translation.y = transform.translation.y.clamp(min_y, max_y);
            }

            // Pinch zoom: scale camera inversely to finger spread.
            // Spreading fingers → zoom in (smaller scale); pinching → zoom out.
            if distance_ratio != 1.0 {
                let new_scale = (controller.current_scale / distance_ratio)
                    .clamp(controller.min_zoom, controller.max_zoom);
                controller.current_scale = new_scale;
                transform.scale = Vec3::splat(new_scale);
            }
        }
    }

    state.prev_centroid = Some(centroid);
    state.prev_distance = Some(distance);
}
