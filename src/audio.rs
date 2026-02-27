//! Audio playback (background music, future SFX).
//!
//! Background music starts when the user clicks "Start Day" (`build_state.is_open`)
//! and is paused when the game is paused (pause menu or pause/resume button).
//!
//! Two background tracks exist – one for each gameplay speed:
//! - `background-sound.ogg` for Normal (1×)
//! - `background-sound-x10.ogg` for Fast (10×)
//!
//! When the player switches speed the track restarts from the beginning.
//!
//! On WASM, playback uses the browser's `HtmlAudioElement` because rodio/cpal
//! cannot obtain an audio output device in the browser.
//! On native, Bevy's built-in audio (rodio) handles playback.

use bevy::prelude::*;

use crate::resources::GameSpeed;
use crate::states::AppState;

// ════════════════════════════════════════════════════════════════════════
//  One-shot sound effects (cross-platform)
// ════════════════════════════════════════════════════════════════════════

/// Identifies which sound effect to play.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SfxKind {
    /// Alarm triggered when a robber arrives at a charger.
    AlarmTheft,
}

impl SfxKind {
    /// Asset path relative to `assets/` used by the WASM `HtmlAudioElement` path.
    #[cfg(target_arch = "wasm32")]
    fn asset_path(self) -> &'static str {
        match self {
            Self::AlarmTheft => "sounds/alarm_theft.wav",
        }
    }
}

/// Fire this event to play a one-shot sound effect on any platform.
///
/// On native, playback uses Bevy's `AudioPlayer` (rodio).
/// On WASM, playback uses the browser's `HtmlAudioElement`.
#[derive(Event, Message, Clone, Copy, Debug)]
pub struct PlaySfx(pub SfxKind);

/// Global toggle for game audio (on/off).
///
/// When disabled, background music is paused (native) or muted (WASM).
/// Toggled via the sound button in the top nav bar.
#[derive(Resource, Debug, Clone)]
pub struct SoundEnabled(pub bool);

impl Default for SoundEnabled {
    fn default() -> Self {
        Self(true)
    }
}

impl SoundEnabled {
    /// Toggle between on and off.
    pub fn toggle(&mut self) {
        self.0 = !self.0;
    }

    /// Short label for display on the toggle button.
    pub fn label(&self) -> &'static str {
        if self.0 { "Sound On" } else { "Sound Off" }
    }
}

/// Returns the asset path (relative to `assets/`) for the given speed.
fn background_track_path(speed: GameSpeed) -> &'static str {
    match speed {
        GameSpeed::Fast => "sounds/background-sound-x10.ogg",
        // Normal, and Paused as a fallback (should not happen in practice).
        _ => "sounds/background-sound.ogg",
    }
}

/// Maps `GameSpeed::Paused` to the default gameplay speed (`Fast`).
/// Normal and Fast pass through unchanged.
fn effective_speed(speed: GameSpeed) -> GameSpeed {
    match speed {
        GameSpeed::Paused => GameSpeed::Fast,
        other => other,
    }
}

// ════════════════════════════════════════════════════════════════════════
//  WASM implementation – HtmlAudioElement via web-sys
// ════════════════════════════════════════════════════════════════════════

#[cfg(target_arch = "wasm32")]
mod wasm_audio {
    use bevy::prelude::*;
    use wasm_bindgen::JsCast;

    use crate::resources::{BuildState, GameClock, GameSpeed};
    use crate::states::AppState;

    use super::{background_track_path, effective_speed};

    /// Holds the browser `<audio>` element used for background music.
    ///
    /// Registered as a **non-send** resource because `HtmlAudioElement` wraps
    /// `JsValue` which is not `Send + Sync`. This is safe on WASM where Bevy
    /// runs single-threaded.
    #[derive(Default)]
    pub struct WebBackgroundMusic {
        pub element: Option<web_sys::HtmlAudioElement>,
        /// The speed variant for which the current element was created.
        pub active_speed: Option<GameSpeed>,
    }

    /// Creates a looped `<audio>` element pointing at the correct background track.
    fn create_audio_element(speed: GameSpeed) -> Option<web_sys::HtmlAudioElement> {
        let document = web_sys::window()?.document()?;
        let element = document
            .create_element("audio")
            .ok()?
            .dyn_into::<web_sys::HtmlAudioElement>()
            .ok()?;
        // WASM assets are served from the `assets/` directory at the web root.
        let path = format!("assets/{}", background_track_path(speed));
        element.set_src(&path);
        element.set_loop(true);
        Some(element)
    }

    /// Starts looping background music when the user has started the day.
    pub fn start_background_music(
        mut web_music: NonSendMut<WebBackgroundMusic>,
        build_state: Res<BuildState>,
        app_state: Res<State<AppState>>,
        game_clock: Res<GameClock>,
        sound_enabled: Res<super::SoundEnabled>,
    ) {
        if !sound_enabled.0 {
            return;
        }
        if !build_state.is_open || !matches!(app_state.get(), AppState::Playing | AppState::Paused)
        {
            return;
        }
        if web_music.element.is_some() {
            return;
        }
        let speed = effective_speed(game_clock.speed);
        if let Some(element) = create_audio_element(speed) {
            // `play()` returns a JS Promise; fire-and-forget is fine here.
            let _ = element.play();
            web_music.element = Some(element);
            web_music.active_speed = Some(speed);
        }
    }

    /// Swaps the background track when the player changes speed (Normal ↔ Fast).
    pub fn swap_on_speed_change(
        mut web_music: NonSendMut<WebBackgroundMusic>,
        game_clock: Res<GameClock>,
    ) {
        if game_clock.is_paused() {
            return;
        }
        let Some(active) = web_music.active_speed else {
            return;
        };
        let target = effective_speed(game_clock.speed);
        if active == target {
            return;
        }
        // Tear down old element.
        if let Some(element) = web_music.element.take() {
            let _ = element.pause();
            element.set_src(""); // release the network resource
        }
        // Create and play the new track from the beginning.
        if let Some(element) = create_audio_element(target) {
            let _ = element.play();
            web_music.element = Some(element);
            web_music.active_speed = Some(target);
        }
    }

    /// Pauses / resumes the `<audio>` element to match the game pause state and sound toggle.
    pub fn sync_audio_to_pause(
        web_music: NonSend<WebBackgroundMusic>,
        app_state: Res<State<AppState>>,
        game_clock: Res<GameClock>,
        sound_enabled: Res<super::SoundEnabled>,
    ) {
        let Some(element) = web_music.element.as_ref() else {
            return;
        };
        let should_pause =
            !sound_enabled.0 || *app_state.get() == AppState::Paused || game_clock.is_paused();
        if should_pause {
            let _ = element.pause();
        } else if element.paused() {
            let _ = element.play();
        }
    }

    /// Stops and discards the `<audio>` element when leaving gameplay states.
    pub fn stop_background_music(
        mut web_music: NonSendMut<WebBackgroundMusic>,
        app_state: Res<State<AppState>>,
    ) {
        if !matches!(
            app_state.get(),
            AppState::MainMenu
                | AppState::DayEnd
                | AppState::GameOver
                | AppState::Loading
                | AppState::CharacterSetup
        ) {
            return;
        }
        if let Some(element) = web_music.element.take() {
            let _ = element.pause();
            element.set_src(""); // release the network resource
        }
        web_music.active_speed = None;
    }

    /// Plays one-shot sound effects via `HtmlAudioElement` on WASM.
    pub fn play_sfx(
        mut events: MessageReader<super::PlaySfx>,
        sound_enabled: Res<super::SoundEnabled>,
    ) {
        if !sound_enabled.0 {
            events.clear();
            return;
        }
        for event in events.read() {
            // WASM assets are served from the `assets/` directory at the web root.
            let path = format!("assets/{}", event.0.asset_path());
            if let Ok(audio) = web_sys::HtmlAudioElement::new_with_src(&path) {
                let _ = audio.play();
            }
        }
    }
}

// ════════════════════════════════════════════════════════════════════════
//  Native implementation – Bevy built-in audio (rodio)
// ════════════════════════════════════════════════════════════════════════

#[cfg(not(target_arch = "wasm32"))]
mod native_audio {
    use bevy::prelude::*;

    use crate::resources::{BuildState, GameClock, GameSpeed};
    use crate::states::AppState;

    use super::{background_track_path, effective_speed};

    /// Marker + metadata for the entity playing background music.
    /// Stores the [`GameSpeed`] for which the current track was loaded.
    #[derive(Component)]
    pub struct BackgroundMusic(GameSpeed);

    /// Spawns an `AudioPlayer` entity when the user has started the day.
    pub fn start_background_music(
        mut commands: Commands,
        asset_server: Res<AssetServer>,
        build_state: Res<BuildState>,
        music_query: Query<(), With<BackgroundMusic>>,
        app_state: Res<State<AppState>>,
        game_clock: Res<GameClock>,
        sound_enabled: Res<super::SoundEnabled>,
    ) {
        if !sound_enabled.0 {
            return;
        }
        if !build_state.is_open || !matches!(app_state.get(), AppState::Playing | AppState::Paused)
        {
            return;
        }
        if !music_query.is_empty() {
            return;
        }
        let speed = effective_speed(game_clock.speed);
        let handle = asset_server.load::<bevy::audio::AudioSource>(background_track_path(speed));
        commands.spawn((
            BackgroundMusic(speed),
            bevy::audio::AudioPlayer::new(handle),
            bevy::audio::PlaybackSettings::LOOP,
        ));
    }

    /// Swaps the background track when the player changes speed (Normal ↔ Fast).
    pub fn swap_on_speed_change(
        mut commands: Commands,
        asset_server: Res<AssetServer>,
        game_clock: Res<GameClock>,
        music_query: Query<(Entity, &BackgroundMusic)>,
    ) {
        if game_clock.is_paused() {
            return;
        }
        let target = effective_speed(game_clock.speed);
        for (entity, music) in &music_query {
            if music.0 == target {
                continue;
            }
            commands.entity(entity).despawn();
            let handle =
                asset_server.load::<bevy::audio::AudioSource>(background_track_path(target));
            commands.spawn((
                BackgroundMusic(target),
                bevy::audio::AudioPlayer::new(handle),
                bevy::audio::PlaybackSettings::LOOP,
            ));
        }
    }

    /// Pauses / resumes the audio sink to match the game pause state and sound toggle.
    pub fn sync_audio_to_pause(
        sink_query: Query<&bevy::audio::AudioSink, With<BackgroundMusic>>,
        app_state: Res<State<AppState>>,
        game_clock: Res<GameClock>,
        sound_enabled: Res<super::SoundEnabled>,
    ) {
        let should_pause =
            !sound_enabled.0 || *app_state.get() == AppState::Paused || game_clock.is_paused();
        for sink in &sink_query {
            if should_pause {
                sink.pause();
            } else {
                sink.play();
            }
        }
    }

    /// Despawns the background music entity when leaving gameplay states.
    pub fn stop_background_music(
        mut commands: Commands,
        music_query: Query<Entity, With<BackgroundMusic>>,
        app_state: Res<State<AppState>>,
    ) {
        if !matches!(
            app_state.get(),
            AppState::MainMenu
                | AppState::DayEnd
                | AppState::GameOver
                | AppState::Loading
                | AppState::CharacterSetup
        ) {
            return;
        }
        for entity in &music_query {
            commands.entity(entity).despawn();
        }
    }

    /// Plays one-shot sound effects via Bevy's `AudioPlayer` (rodio) on native.
    pub fn play_sfx(
        mut commands: Commands,
        mut events: MessageReader<super::PlaySfx>,
        audio_assets: Res<crate::resources::AudioAssets>,
        sound_enabled: Res<super::SoundEnabled>,
    ) {
        if !sound_enabled.0 {
            events.clear();
            return;
        }
        for event in events.read() {
            let handle = match event.0 {
                super::SfxKind::AlarmTheft => audio_assets.alarm_theft.clone(),
            };
            commands.spawn((
                bevy::audio::AudioPlayer::new(handle),
                bevy::audio::PlaybackSettings::DESPAWN,
            ));
        }
    }
}

// ════════════════════════════════════════════════════════════════════════
//  Plugin
// ════════════════════════════════════════════════════════════════════════

/// Plugin for game audio (background music and future sound effects).
pub struct AudioPlugin;

impl Plugin for AudioPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<SoundEnabled>().add_message::<PlaySfx>();

        let playing_or_paused = in_state(AppState::Playing).or(in_state(AppState::Paused));
        let outside_gameplay = in_state(AppState::MainMenu)
            .or(in_state(AppState::DayEnd))
            .or(in_state(AppState::GameOver))
            .or(in_state(AppState::Loading))
            .or(in_state(AppState::CharacterSetup));

        #[cfg(target_arch = "wasm32")]
        {
            app.insert_non_send_resource(wasm_audio::WebBackgroundMusic::default())
                .add_systems(
                    Update,
                    (
                        wasm_audio::start_background_music,
                        wasm_audio::swap_on_speed_change,
                        wasm_audio::sync_audio_to_pause,
                    )
                        .run_if(playing_or_paused),
                )
                .add_systems(
                    Update,
                    wasm_audio::stop_background_music.run_if(outside_gameplay),
                )
                .add_systems(Update, wasm_audio::play_sfx);
        }

        #[cfg(not(target_arch = "wasm32"))]
        {
            app.add_systems(
                Update,
                (
                    native_audio::start_background_music,
                    native_audio::swap_on_speed_change,
                    native_audio::sync_audio_to_pause,
                )
                    .run_if(playing_or_paused),
            )
            .add_systems(
                Update,
                native_audio::stop_background_music.run_if(outside_gameplay),
            )
            .add_systems(Update, native_audio::play_sfx);
        }
    }
}
