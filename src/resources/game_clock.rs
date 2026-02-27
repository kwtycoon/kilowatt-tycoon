//! Game clock resource for time management
//!
//! Time is measured in "game seconds" where 86400 game seconds = 1 day.
//! At Fast speed, 1 day takes 15 real seconds.

use bevy::prelude::*;

/// Seconds in a game day (24 hours)
pub const DAY_LENGTH_SECONDS: f32 = 86400.0;

/// Speed multiplier options
/// Fast mode: 15 real seconds = 1 day (86400 game seconds)
/// Normal mode: 150 real seconds = 1 day
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub enum GameSpeed {
    Paused,
    Normal, // 576x (150s per day)
    #[default]
    Fast, // 5760x (15s per day)
}

impl GameSpeed {
    pub fn multiplier(&self) -> f32 {
        match self {
            GameSpeed::Paused => 0.0,
            GameSpeed::Normal => 1440.0, // 86400 / 60 = 1440
            GameSpeed::Fast => 2880.0,   // 86400 / 30 = 2880
        }
    }

    /// Visual animation multiplier - decoupled from simulation speed for readability.
    /// Returns a small multiplier so vehicles move at a comfortable viewing pace
    /// regardless of how fast the simulation clock is running.
    pub fn visual_multiplier(&self) -> f32 {
        match self {
            GameSpeed::Paused => 0.0,
            GameSpeed::Normal => 3.0,
            GameSpeed::Fast => 6.0,
        }
    }

    pub fn display_name(&self) -> &'static str {
        match self {
            GameSpeed::Paused => "Paused",
            GameSpeed::Normal => "1x",
            GameSpeed::Fast => "10x",
        }
    }
}

/// Central game clock resource
#[derive(Resource, Debug, Clone)]
pub struct GameClock {
    /// In-game seconds elapsed within the current day (0 to 86400)
    pub game_time: f32,
    /// Total in-game seconds elapsed since game start. Monotonically increasing, never resets.
    /// Use this for timestamps that may span across day boundaries (e.g. fault occurred_at/resolved_at).
    pub total_game_time: f32,
    /// Real seconds elapsed this day (resets each day)
    pub real_time: f32,
    /// Total real seconds elapsed (paused-aware), never resets during gameplay.
    /// Used for visual timing like speech bubbles that should be independent of simulation speed.
    pub total_real_time: f32,
    /// Current speed setting
    pub speed: GameSpeed,
    /// Current day number (1-indexed, resets at 31)
    pub day: u32,
    /// Current month number (1-indexed, resets at 13)
    pub month: u32,
    /// Current year number (1-indexed)
    pub year: u32,
    /// Whether the day timer has completed and we're winding down (letting cars finish and leave).
    /// While true, no new drivers or ambient traffic spawn. The simulation continues running so
    /// active charging sessions complete naturally and vehicles drive off the map.
    /// Transition to `DayEnd` happens once all drivers have exited.
    pub day_ending: bool,
}

impl Default for GameClock {
    fn default() -> Self {
        Self {
            game_time: 0.0,
            total_game_time: 0.0,
            real_time: 0.0,
            total_real_time: 0.0,
            speed: GameSpeed::Normal,
            day: 1,
            month: 1,
            year: 1,
            day_ending: false,
        }
    }
}

impl GameClock {
    pub fn is_paused(&self) -> bool {
        matches!(self.speed, GameSpeed::Paused)
    }

    pub fn pause(&mut self) {
        self.speed = GameSpeed::Paused;
    }

    pub fn resume(&mut self) {
        if self.is_paused() {
            self.speed = GameSpeed::Fast;
        }
    }

    pub fn set_speed(&mut self, speed: GameSpeed) {
        self.speed = speed;
    }

    pub fn toggle_pause(&mut self) {
        if self.is_paused() {
            self.speed = GameSpeed::Fast;
        } else {
            self.speed = GameSpeed::Paused;
        }
    }

    /// Check if the current day has ended (game_time >= 86400)
    pub fn is_day_complete(&self) -> bool {
        self.game_time >= DAY_LENGTH_SECONDS
    }

    /// Get the current hour of day (0-23)
    pub fn hour(&self) -> u32 {
        ((self.game_time / 3600.0) as u32) % 24
    }

    /// Get the current minute of the hour (0-59)
    pub fn minute(&self) -> u32 {
        ((self.game_time % 3600.0) / 60.0) as u32
    }

    /// Get formatted time of day as "HH:MM"
    pub fn time_of_day(&self) -> String {
        format!("{:02}:{:02}", self.hour(), self.minute())
    }

    /// Get formatted time of day in 12-hour format with AM/PM
    pub fn time_of_day_12h(&self) -> String {
        let hour = self.hour();
        let minute = self.minute();
        let (hour_12, am_pm) = if hour == 0 {
            (12, "AM")
        } else if hour < 12 {
            (hour, "AM")
        } else if hour == 12 {
            (12, "PM")
        } else {
            (hour - 12, "PM")
        };
        format!("{hour_12}:{minute:02} {am_pm}")
    }

    /// Get formatted display string "Day X - HH:MM"
    pub fn formatted_time(&self) -> String {
        format!("Day {} - {}", self.day, self.time_of_day())
    }

    /// Get formatted date string "Year X - Month Y - Day Z"
    pub fn formatted_date(&self) -> String {
        format!(
            "Year {} - Month {} - Day {}",
            self.year, self.month, self.day
        )
    }

    /// Advance time by delta (real seconds)
    pub fn tick(&mut self, delta: f32) {
        if !self.is_paused() {
            // During day-ending wind-down, freeze the game clock so the UI shows
            // 11:59 PM. Real time still advances so `visual_multiplier()` keeps
            // vehicles moving.
            if self.day_ending {
                self.real_time += delta;
                self.total_real_time += delta;
                return;
            }

            let speed_delta = delta * self.speed.multiplier();
            self.real_time += delta; // Track real time separately (resets each day)
            self.total_real_time += delta; // Track total real time (paused-aware, never resets)
            self.game_time += speed_delta;
            self.total_game_time += speed_delta; // Monotonic game time (never resets)
        }
    }

    /// Reset for a new day (called when transitioning from DayEnd to Playing)
    pub fn start_new_day(&mut self) {
        self.day += 1;

        // Check if month should advance (after day 30)
        if self.day > 30 {
            self.day = 1;
            self.month += 1;

            // Check if year should advance (after month 12)
            if self.month > 12 {
                self.month = 1;
                self.year += 1;
            }
        }

        self.game_time = 0.0;
        self.real_time = 0.0;
        self.speed = GameSpeed::Fast;
        self.day_ending = false;
    }

    /// Full reset (for new game)
    pub fn reset(&mut self) {
        self.game_time = 0.0;
        self.total_game_time = 0.0;
        self.real_time = 0.0;
        self.total_real_time = 0.0;
        self.speed = GameSpeed::Fast;
        self.day = 1;
        self.month = 1;
        self.year = 1;
        self.day_ending = false;
    }
}
