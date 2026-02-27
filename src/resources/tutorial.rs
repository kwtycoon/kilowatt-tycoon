//! Tutorial state resource for tracking player progress through the walkthrough

use bevy::prelude::*;

/// Tutorial state resource
#[derive(Resource, Debug, Clone, Default)]
pub struct TutorialState {
    pub current_step: Option<TutorialStep>,
    pub completed: bool,
    pub skipped: bool,
}

impl TutorialState {
    /// Check if the tutorial is currently active
    pub fn is_active(&self) -> bool {
        self.current_step.is_some() && !self.completed && !self.skipped
    }

    /// Start the tutorial from the beginning
    pub fn start(&mut self) {
        self.current_step = Some(TutorialStep::Welcome);
        self.completed = false;
        self.skipped = false;
    }

    /// Skip the tutorial
    pub fn skip(&mut self) {
        self.current_step = None;
        self.skipped = true;
    }

    /// Mark tutorial as completed
    pub fn complete(&mut self) {
        self.current_step = None;
        self.completed = true;
    }

    /// Advance to the next step
    pub fn advance_to(&mut self, step: TutorialStep) {
        self.current_step = Some(step);
    }
}

/// Tutorial step enum
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum TutorialStep {
    Welcome,
    PlaceCharger,
    PlaceTransformer,
    StartDay,
    FixCharger,
    SwitchSite,
}

impl TutorialStep {
    /// Get the title for this tutorial step
    pub fn title(&self) -> &'static str {
        match self {
            TutorialStep::Welcome => "Welcome to Kilowatt Tycoon!",
            TutorialStep::PlaceCharger => "Step 1: Place a Charger",
            TutorialStep::PlaceTransformer => "Step 2: Add Power",
            TutorialStep::StartDay => "Step 3: Open for Business",
            TutorialStep::FixCharger => "Step 4: Handle Problems",
            TutorialStep::SwitchSite => "Expand Your Empire",
        }
    }

    /// Get the description for this tutorial step
    pub fn description(&self) -> &'static str {
        match self {
            TutorialStep::Welcome => {
                "Your goal is to build and operate a profitable EV charging station. Let's walk through the basics."
            }
            TutorialStep::PlaceCharger => {
                "Click a charger from the BUILD menu, then click a green spot on the parking lot to place it."
            }
            TutorialStep::PlaceTransformer => {
                "Chargers need electricity! Select a Transformer from INFRASTRUCTURE and place it on the grass."
            }
            TutorialStep::StartDay => {
                "Click START DAY to begin operations. Vehicles will arrive and charge at your station."
            }
            TutorialStep::FixCharger => {
                "Chargers sometimes fail! Click the broken charger, then press REBOOT to fix it remotely."
            }
            TutorialStep::SwitchSite => {
                "Ready for more? Use the location tabs at the top to switch between sites or rent new ones."
            }
        }
    }

    /// Get the next step in the tutorial sequence
    pub fn next(&self) -> Option<TutorialStep> {
        match self {
            TutorialStep::Welcome => Some(TutorialStep::PlaceCharger),
            TutorialStep::PlaceCharger => Some(TutorialStep::PlaceTransformer),
            TutorialStep::PlaceTransformer => Some(TutorialStep::StartDay),
            TutorialStep::StartDay => Some(TutorialStep::FixCharger),
            TutorialStep::FixCharger => Some(TutorialStep::SwitchSite),
            TutorialStep::SwitchSite => None,
        }
    }

    /// Check if this step has a "Next" button (vs auto-advance)
    pub fn has_next_button(&self) -> bool {
        matches!(self, TutorialStep::Welcome)
    }

    /// Check if this step auto-advances on a condition
    pub fn auto_advances(&self) -> bool {
        matches!(
            self,
            TutorialStep::PlaceCharger
                | TutorialStep::PlaceTransformer
                | TutorialStep::StartDay
                | TutorialStep::FixCharger
        )
    }

    /// Get the step number for display (1-based)
    pub fn step_number(&self) -> Option<usize> {
        match self {
            TutorialStep::Welcome => None,
            TutorialStep::PlaceCharger => Some(1),
            TutorialStep::PlaceTransformer => Some(2),
            TutorialStep::StartDay => Some(3),
            TutorialStep::FixCharger => Some(4),
            TutorialStep::SwitchSite => None,
        }
    }

    /// Get total number of steps
    pub fn total_steps() -> usize {
        6
    }

    /// Get the current step index (0-based)
    pub fn index(&self) -> usize {
        match self {
            TutorialStep::Welcome => 0,
            TutorialStep::PlaceCharger => 1,
            TutorialStep::PlaceTransformer => 2,
            TutorialStep::StartDay => 3,
            TutorialStep::FixCharger => 4,
            TutorialStep::SwitchSite => 5,
        }
    }

    /// Check if this step should show the modal overlay
    pub fn shows_modal(&self) -> bool {
        matches!(self, TutorialStep::Welcome | TutorialStep::SwitchSite)
    }

    /// Check if this step should show a pointer
    pub fn shows_pointer(&self) -> bool {
        matches!(
            self,
            TutorialStep::PlaceCharger
                | TutorialStep::PlaceTransformer
                | TutorialStep::StartDay
                | TutorialStep::FixCharger
        )
    }
}
