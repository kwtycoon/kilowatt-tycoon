//! Driver and technician emotion and speech bubble components

use bevy::prelude::*;

use crate::components::driver::DriverState;

/// Driver emotion state with context
#[derive(Component, Debug, Clone)]
pub struct DriverEmotion {
    /// Current emotional state
    pub mood: EmotionMood,
    /// Reason for this mood (for speech bubble)
    pub reason: EmotionReason,
    /// Time when this emotion was set (total_real_time from GameClock)
    pub set_at: f32,
    /// Duration to show this emotion (real seconds)
    pub duration: f32,
    /// Selected speech bubble text for this emotion
    pub speech_text: Option<&'static str>,
    /// Last known driver state (to detect state changes and force emotion updates)
    pub last_driver_state: Option<DriverState>,
    /// Last frustration reason (preserved when driver leaves angry)
    pub last_frustration_reason: Option<EmotionReason>,
}

impl Default for DriverEmotion {
    fn default() -> Self {
        Self {
            mood: EmotionMood::Neutral,
            reason: EmotionReason::JustArrived,
            set_at: 0.0,
            duration: 3.0,
            speech_text: None,
            last_driver_state: None,
            last_frustration_reason: None,
        }
    }
}

impl DriverEmotion {
    /// Set a new emotion. Returns true if something actually changed.
    pub fn set_emotion(
        &mut self,
        mood: EmotionMood,
        reason: EmotionReason,
        game_time: f32,
    ) -> bool {
        let reason_changed = self.reason != reason;
        let mood_changed = self.mood != mood;

        // If nothing changed, don't update (avoids triggering change detection)
        if !reason_changed && !mood_changed {
            return false;
        }

        // Track frustration reasons so we can show them when driver leaves angry
        if reason.is_frustration_reason() {
            self.last_frustration_reason = Some(reason);
        }

        self.mood = mood;
        self.reason = reason;
        self.set_at = game_time;
        self.duration = reason.display_duration();

        // Only pick new text if reason actually changed
        if reason_changed {
            let variations = reason.speech_variations();
            if !variations.is_empty() {
                let idx =
                    (rand::random::<f32>() * variations.len() as f32) as usize % variations.len();
                self.speech_text = Some(variations[idx]);
            } else {
                self.speech_text = None;
            }
        }

        true
    }

    pub fn is_expired(&self, current_time: f32) -> bool {
        current_time - self.set_at > self.duration
    }
}

/// Emotional mood levels
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum EmotionMood {
    VeryHappy,
    Happy,
    Neutral,
    Skeptical,
    Frustrated,
    Angry,
}

/// Reasons for emotions (triggers speech bubbles)
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum EmotionReason {
    JustArrived,
    PriceTooHigh,
    PriceFair,
    PriceGreat,
    FoundCharger,
    MustWait,
    WaitingTooLong,
    ChargingStarted,
    ChargingAlmostDone,
    ChargingComplete,
    ChargerBroken,
    LeavingAngry,
    /// Driver found and switched to an alternative charger at the site
    SwitchedCharger,
    /// Driver is plugged in but receiving 0 kW (Rule 2: direct experience)
    NoPower,
    /// Driver is heading to a waiting tile (all bays occupied, queuing)
    HeadingToWait,
    // Frustration reasons (short texts for speech bubbles)
    /// Station/chargers busy - driver must wait or leave
    FrustrationBusy,
    /// Charger broken or stopped working mid-session
    FrustrationDidntWork,
    /// Price is too high for the driver
    FrustrationTooExpensive,
    /// Charger delivered zero energy — driver gave up (Rule 2)
    FrustrationNoPower,
}

impl EmotionReason {
    /// Check if this is a frustration reason we want to track
    pub fn is_frustration_reason(&self) -> bool {
        matches!(
            self,
            EmotionReason::FrustrationBusy
                | EmotionReason::FrustrationDidntWork
                | EmotionReason::FrustrationTooExpensive
                | EmotionReason::FrustrationNoPower
                | EmotionReason::WaitingTooLong
                | EmotionReason::ChargerBroken
                | EmotionReason::NoPower
                | EmotionReason::PriceTooHigh
        )
    }

    /// Get a short label describing this frustration reason
    pub fn frustration_label(&self) -> Option<&'static str> {
        match self {
            EmotionReason::FrustrationBusy
            | EmotionReason::MustWait
            | EmotionReason::WaitingTooLong => Some("Too busy"),
            EmotionReason::FrustrationDidntWork | EmotionReason::ChargerBroken => Some("Broken"),
            EmotionReason::FrustrationTooExpensive | EmotionReason::PriceTooHigh => {
                Some("Too expensive")
            }
            EmotionReason::FrustrationNoPower | EmotionReason::NoPower => Some("No power"),
            _ => None,
        }
    }

    /// Get speech bubble text variations
    pub fn speech_variations(&self) -> &'static [&'static str] {
        match self {
            EmotionReason::JustArrived => &[
                "Let's charge up!",
                "Need some juice.",
                "Hope there's a spot.",
                "Time to top off.",
                "Running on fumes.",
                "Don't let me down.",
                "Pray there's a plug.",
                "Battery's begging.",
                "1% and a dream.",
                "Made it. Barely.",
            ],
            EmotionReason::PriceTooHigh => &[
                "$0.50/kWh?!",
                "Expensive...",
                "Highway robbery!",
                "Is this gold-plated?",
                "Who set these rates?!",
                "Gas is cheaper!",
                "My wallet flinched.",
                "Nope. Just nope.",
                "I want a manager.",
                "For electricity?!",
            ],
            EmotionReason::PriceFair => &[
                "Reasonable price",
                "Fair enough.",
                "Normal rates.",
                "Seems okay.",
                "Can't complain.",
                "I've seen worse.",
                "That'll do.",
                "Acceptable.",
                "Not bad, not great.",
            ],
            EmotionReason::PriceGreat => &[
                "Great deal!",
                "Cheap charging!",
                "Love these prices!",
                "Steal!",
                "Don't mind if I do!",
                "Is this real?!",
                "Take my money!",
                "Basically free!",
                "My lucky day!",
                "Tell no one.",
            ],
            EmotionReason::FoundCharger => &[
                "Perfect!",
                "Found one!",
                "Locked in.",
                "Starting now.",
                "Chef's kiss.",
                "Dibs!",
                "Mine. All mine.",
                "Smooth operator.",
                "Like a glove.",
            ],
            EmotionReason::MustWait => &[
                "I'll wait...",
                "There's a queue.",
                "Sigh, a line.",
                "Waiting my turn.",
                "Of course. A line.",
                "Story of my life.",
                "Cool. Cool cool cool.",
                "Deep breaths...",
                "Patience is a virtue?",
            ],
            EmotionReason::WaitingTooLong => &[
                "This is taking forever...",
                "Still waiting...",
                "My battery is dying!",
                "Any day now...",
                "I've aged a year.",
                "Walk home faster.",
                "Hello?? Anyone??",
                "I have gray hairs now.",
                "Is time even real?",
                "I could build one faster.",
            ],
            EmotionReason::ChargingStarted => &[
                "Finally!",
                "Charging at last.",
                "Here we go.",
                "Flowing now.",
                "It's alive!",
                "Electrons, baby!",
                "Sweet, sweet power.",
                "About time!",
                "We're in business.",
            ],
            EmotionReason::ChargingAlmostDone => &[
                "Almost there...",
                "Nearly full.",
                "Just a bit more.",
                "Wrapping up.",
                "Soooo close.",
                "Come on come on...",
                "Last few percent!",
                "The home stretch.",
                "Don't stop now!",
            ],
            EmotionReason::ChargingComplete => &[
                "All set!",
                "Full charge!",
                "Ready to roll!",
                "Good to go!",
                "Topped off. Later!",
                "100%. Let's ride.",
                "Fully juiced!",
                "That hit the spot.",
                "Back in action!",
            ],
            EmotionReason::ChargerBroken => &[
                "Are you kidding me?!",
                "Broken again?",
                "Out of order?!",
                "This is useless!",
                "Not literally shocking.",
                "A paperweight.",
                "Anyone work here?",
                "Cute decoration.",
                "Fix your stuff!",
                "Classic.",
            ],
            EmotionReason::LeavingAngry => &[
                "Never coming back!",
                "Worst station ever.",
                "I'm out of here!",
                "Forget this!",
                "One star. Done.",
                "Telling everyone.",
                "Uninstalling the app.",
                "See you never.",
                "What a waste of time.",
                "You lost a customer.",
            ],
            EmotionReason::SwitchedCharger => &[
                "Found another one!",
                "This one works.",
                "Switching over.",
                "Plan B it is.",
                "Okay, trying this one.",
                "Second time's the charm.",
                "New plug, who dis?",
                "Let's try next door.",
                "Musical chargers!",
                "Backup plan: engaged.",
            ],
            EmotionReason::HeadingToWait => &[
                "Getting in line.",
                "Waiting for a spot.",
                "I'll queue up.",
                "All full, I'll wait.",
                "No rush... kinda.",
                "Joining the queue.",
                "Guess I'm waiting.",
                "Next in line!",
                "In the queue.",
            ],
            EmotionReason::NoPower => &[
                "Zero kilowatts?!",
                "Nothing's flowing...",
                "Hello? Power?",
                "It says 0 kW!",
                "Am I even plugged in?",
                "My car says zero.",
                "Is this thing on?",
                "No juice at all!",
                "The meter won't move.",
                "Getting nothing here.",
            ],
            // Short frustration texts (fit in bubble)
            EmotionReason::FrustrationNoPower => &[
                "Zero power!",
                "No charge!",
                "0 kW!!",
                "Nothing!",
                "Dead outlet!",
                "Zilch. Nada.",
                "Paying for air.",
                "Phantom charger.",
                "Power? What power?",
                "Just a nightlight.",
            ],
            EmotionReason::FrustrationBusy => &[
                "So busy!",
                "No spots?!",
                "All taken...",
                "Ugh, packed!",
                "Full up!",
                "Is this Costco?!",
                "Sardine station.",
                "Build more!",
                "Every. Single. Time.",
                "This is a joke.",
            ],
            EmotionReason::FrustrationDidntWork => &[
                "Broken!",
                "It died!",
                "Won't charge!",
                "Busted!",
                "Dead!",
                "Useless!",
                "Fancy doorstop.",
                "Technology, huh?",
                "Unbelievable.",
                "Shocking quality.",
            ],
            EmotionReason::FrustrationTooExpensive => &[
                "Too pricey!",
                "Rip-off!",
                "So costly!",
                "Overpriced!",
                "Yikes, $$$!",
                "Mortgage is cheaper.",
                "Robbery!",
                "Need a second job.",
                "Hard pass.",
                "Nah. I'm good.",
            ],
        }
    }

    /// How long to display this emotion (real seconds).
    /// These are actual wall-clock seconds, independent of simulation speed.
    pub fn display_duration(&self) -> f32 {
        match self {
            EmotionReason::JustArrived => 5.0,
            EmotionReason::PriceTooHigh => 8.0, // Important info - show longer
            EmotionReason::PriceFair => 6.0,
            EmotionReason::PriceGreat => 7.0,
            EmotionReason::FoundCharger => 5.0,
            EmotionReason::MustWait => 6.0,
            EmotionReason::WaitingTooLong => 8.0, // Emphasize frustration
            EmotionReason::ChargingStarted => 6.0,
            EmotionReason::ChargingAlmostDone => 5.0,
            EmotionReason::ChargingComplete => 6.0,
            EmotionReason::ChargerBroken => 8.0, // Critical issue
            EmotionReason::LeavingAngry => 7.0,  // Important feedback
            EmotionReason::SwitchedCharger => 6.0,
            EmotionReason::HeadingToWait => 6.0,
            EmotionReason::NoPower => 8.0, // Important — player needs to see this
            // Frustration reasons - show longer for feedback
            EmotionReason::FrustrationBusy => 7.0,
            EmotionReason::FrustrationDidntWork => 8.0,
            EmotionReason::FrustrationTooExpensive => 7.0,
            EmotionReason::FrustrationNoPower => 8.0,
        }
    }
}

// ============ Technician Emotions ============

/// Technician emotion state
#[derive(Component, Debug, Clone)]
pub struct TechnicianEmotion {
    /// Reason for this emotion (determines speech text)
    pub reason: TechnicianEmotionReason,
    /// Time when this emotion was set (total_real_time from GameClock)
    pub set_at: f32,
    /// Duration to show this emotion (real seconds)
    pub duration: f32,
    /// Selected speech bubble text for this emotion
    pub speech_text: Option<&'static str>,
}

impl Default for TechnicianEmotion {
    fn default() -> Self {
        Self {
            reason: TechnicianEmotionReason::ArrivingAtSite,
            set_at: 0.0,
            duration: 5.0, // Real seconds
            speech_text: None,
        }
    }
}

impl TechnicianEmotion {
    /// Create with a specific reason and pick random speech text
    pub fn new(reason: TechnicianEmotionReason, game_time: f32) -> Self {
        let variations = reason.speech_variations();
        let speech_text = if !variations.is_empty() {
            let idx = (rand::random::<f32>() * variations.len() as f32) as usize % variations.len();
            Some(variations[idx])
        } else {
            None
        };

        Self {
            reason,
            set_at: game_time,
            duration: reason.display_duration(),
            speech_text,
        }
    }

    /// Update to a new emotion reason
    pub fn set_reason(&mut self, reason: TechnicianEmotionReason, game_time: f32) {
        if self.reason != reason {
            let variations = reason.speech_variations();
            self.speech_text = if !variations.is_empty() {
                let idx =
                    (rand::random::<f32>() * variations.len() as f32) as usize % variations.len();
                Some(variations[idx])
            } else {
                None
            };
        }
        self.reason = reason;
        self.set_at = game_time;
        self.duration = reason.display_duration();
    }

    /// Check if this emotion has expired
    pub fn is_expired(&self, current_time: f32) -> bool {
        current_time - self.set_at > self.duration
    }
}

/// Reasons for technician emotions (triggers speech bubbles)
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum TechnicianEmotionReason {
    #[default]
    ArrivingAtSite,
    StartingRepair,
    Repairing,
    RepairComplete,
    RepairFailed,
    LeavingSite,
    NextJob,
}

impl TechnicianEmotionReason {
    /// Get speech bubble text variations
    pub fn speech_variations(&self) -> &'static [&'static str] {
        match self {
            TechnicianEmotionReason::ArrivingAtSite => &[
                "On my way!",
                "Heading in.",
                "Let's see the issue.",
                "I'm on it!",
            ],
            TechnicianEmotionReason::StartingRepair => &[
                "Let's see what we got.",
                "Time to fix this.",
                "Alright, diagnosing...",
                "Found the problem.",
            ],
            TechnicianEmotionReason::Repairing => &[
                "Working on it...",
                "Almost there...",
                "Just a bit more.",
                "Tightening up...",
            ],
            TechnicianEmotionReason::RepairComplete => &[
                "Good as new!",
                "All fixed!",
                "Back in action!",
                "Done and done!",
            ],
            TechnicianEmotionReason::RepairFailed => &[
                "Don't have the right parts...",
                "Need better diagnostics.",
                "Missing the schematics.",
                "Need specialized equipment.",
            ],
            TechnicianEmotionReason::LeavingSite => &[
                "Job complete.",
                "Off to the next one.",
                "See ya!",
                "Call if you need me.",
            ],
            TechnicianEmotionReason::NextJob => &[
                "Another one here!",
                "While I'm here...",
                "Next one's close by.",
                "Back at it.",
            ],
        }
    }

    /// How long to display this emotion (real seconds).
    /// These are actual wall-clock seconds, independent of simulation speed.
    pub fn display_duration(&self) -> f32 {
        match self {
            TechnicianEmotionReason::ArrivingAtSite => 5.0,
            TechnicianEmotionReason::StartingRepair => 6.0,
            TechnicianEmotionReason::Repairing => 8.0,
            TechnicianEmotionReason::RepairComplete => 7.0,
            TechnicianEmotionReason::RepairFailed => 8.0, // Show frustration longer
            TechnicianEmotionReason::LeavingSite => 5.0,
            TechnicianEmotionReason::NextJob => 5.0,
        }
    }
}
