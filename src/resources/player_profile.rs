//! Player profile resource for storing character selection, name, and leaderboard data

use bevy::prelude::*;

/// Character perk variants with embedded multipliers
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum CharacterPerk {
    /// Demand charges reduced by 15%
    UtilityInsider { demand_charge_multiplier: f32 },
    /// +20% more walk-in customers
    CustomerMagnet { demand_multiplier: f32 },
    /// Downtime reduced by 30%
    EfficiencyFreak { downtime_multiplier: f32 },
}

impl CharacterPerk {
    /// Get the display name for this perk
    pub fn name(&self) -> &'static str {
        match self {
            CharacterPerk::UtilityInsider { .. } => "Utility Insider",
            CharacterPerk::CustomerMagnet { .. } => "Customer Magnet",
            CharacterPerk::EfficiencyFreak { .. } => "Efficiency Freak",
        }
    }

    /// Get the full description for this perk
    pub fn description(&self) -> &'static str {
        match self {
            CharacterPerk::UtilityInsider { .. } => {
                "Demand charges are reduced by 15%. Years of working at the utility company taught this operator how to smooth out peak loads before the meter even notices."
            }
            CharacterPerk::CustomerMagnet { .. } => "+20% More Walk-in Customers",
            CharacterPerk::EfficiencyFreak { .. } => "Downtime reduced by 30%",
        }
    }
}

/// Available character types for player selection
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum CharacterKind {
    Ant,
    Mallard,
    Raccoon,
}

impl CharacterKind {
    /// Get the full display name for this operator
    pub fn display_name(&self) -> &'static str {
        match self {
            CharacterKind::Ant => "Optimus Ant",
            CharacterKind::Mallard => "Mallard McCharge",
            CharacterKind::Raccoon => "Doc Volt Raccoon",
        }
    }

    /// Get the role/title for this operator
    pub fn role(&self) -> &'static str {
        match self {
            CharacterKind::Ant => "The Fleet (Efficiency Freak)",
            CharacterKind::Mallard => "The Commercial (Greedy Tycoon)",
            CharacterKind::Raccoon => "The CPO (Tech Junkie)",
        }
    }

    /// Get the bio description for this operator
    pub fn bio(&self) -> &'static str {
        match self {
            CharacterKind::Ant => {
                "She organized her birth by minute and second. To her, a 5-minute delay is a declaration of war."
            }
            CharacterKind::Mallard => {
                "He doesn't sell electricity; he sells the $9 coffee you buy while waiting. If it doesn't have a gift shop, he's not interested."
            }
            CharacterKind::Raccoon => {
                "A caffeinated genius who thinks 'Safety Standards' are just suggestions. He's one short-circuit away from a Nobel Prize—or a blackout."
            }
        }
    }

    /// Get the character perk with its multiplier
    pub fn perk(&self) -> CharacterPerk {
        match self {
            CharacterKind::Ant => CharacterPerk::EfficiencyFreak {
                downtime_multiplier: 0.70,
            },
            CharacterKind::Mallard => CharacterPerk::CustomerMagnet {
                demand_multiplier: 1.20,
            },
            CharacterKind::Raccoon => CharacterPerk::UtilityInsider {
                demand_charge_multiplier: 0.85,
            },
        }
    }

    /// Get all available character kinds
    pub fn all() -> [CharacterKind; 3] {
        [
            CharacterKind::Ant,
            CharacterKind::Mallard,
            CharacterKind::Raccoon,
        ]
    }
}

/// Resource tracking the player's character selection, name, and leaderboard data
#[derive(Resource, Debug, Clone, Default)]
pub struct PlayerProfile {
    /// Selected character (None until chosen)
    pub character: Option<CharacterKind>,
    /// Player's entered name (defaults to "Player" if not set)
    pub name: String,
    /// UUID from Supabase after first leaderboard submission
    pub player_id: Option<String>,
}

impl PlayerProfile {
    /// Create a new player profile with a default name
    pub fn new() -> Self {
        Self {
            character: None,
            name: "Player".to_string(),
            player_id: None,
        }
    }

    /// Create a player profile with a custom name
    pub fn with_name(name: impl Into<String>) -> Self {
        Self {
            character: None,
            name: name.into(),
            player_id: None,
        }
    }

    /// Check if the profile is complete (character and name set)
    pub fn is_complete(&self) -> bool {
        self.character.is_some() && !self.name.is_empty()
    }

    /// Get the active perk for the selected character
    pub fn active_perk(&self) -> Option<CharacterPerk> {
        self.character.map(|c| c.perk())
    }
}
