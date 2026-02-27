//! UI builder utilities for creating common widgets.
//!
//! Provides a fluent API for constructing UI elements consistently.
//!
//! # Usage
//!
//! ```rust,ignore
//! use chargeopssim::helpers::ui_builders::*;
//!
//! fn setup_ui(mut commands: Commands) {
//!     // Create a styled button
//!     let button = UiButton::primary("Start Game")
//!         .width(200.0)
//!         .height(50.0);
//!     commands.spawn(button.build());
//!
//!     // Create a panel
//!     let panel = UiPanel::new()
//!         .title("Settings")
//!         .build(&mut commands);
//! }
//! ```

use bevy::prelude::*;

/// Color palette for consistent UI styling
pub mod colors {
    use bevy::prelude::*;

    // Backgrounds
    pub const PANEL_BG: Color = Color::srgba(0.1, 0.12, 0.15, 0.95);
    pub const HEADER_BG: Color = Color::srgb(0.15, 0.17, 0.2);
    pub const OVERLAY_BG: Color = Color::srgba(0.0, 0.0, 0.0, 0.45);

    // Buttons
    pub const BUTTON_PRIMARY: Color = Color::srgb(0.15, 0.5, 0.25);
    pub const BUTTON_PRIMARY_HOVER: Color = Color::srgb(0.2, 0.6, 0.3);
    pub const BUTTON_SECONDARY: Color = Color::srgb(0.3, 0.32, 0.35);
    pub const BUTTON_SECONDARY_HOVER: Color = Color::srgb(0.4, 0.42, 0.45);
    pub const BUTTON_DANGER: Color = Color::srgb(0.6, 0.2, 0.2);
    pub const BUTTON_DANGER_HOVER: Color = Color::srgb(0.7, 0.3, 0.3);
    pub const BUTTON_DISABLED: Color = Color::srgb(0.2, 0.2, 0.22);

    // Text
    pub const TEXT_PRIMARY: Color = Color::WHITE;
    pub const TEXT_SECONDARY: Color = Color::srgb(0.7, 0.75, 0.8);
    pub const TEXT_MUTED: Color = Color::srgb(0.5, 0.55, 0.6);
    pub const TEXT_SUCCESS: Color = Color::srgb(0.2, 0.8, 0.4);
    pub const TEXT_WARNING: Color = Color::srgb(0.9, 0.7, 0.2);
    pub const TEXT_DANGER: Color = Color::srgb(0.9, 0.3, 0.3);

    // Modal / overlay accents
    /// Neon green modal border glow (greenish tint for setup & tutorial modals)
    pub const MODAL_BORDER_GLOW: Color = Color::srgb(0.0, 0.9, 0.7);
    /// Gold/amber for primary action buttons (START MISSION, NEXT)
    pub const BUTTON_GOLD: Color = Color::srgb(0.95, 0.75, 0.1);
    pub const BUTTON_GOLD_HOVER: Color = Color::srgb(1.0, 0.85, 0.2);
    /// Gold/yellow for prominent modal titles
    pub const TITLE_GOLD: Color = Color::srgb(1.0, 0.85, 0.0);

    // Status indicators
    pub const STATUS_AVAILABLE: Color = Color::srgb(0.2, 0.8, 0.3);
    pub const STATUS_CHARGING: Color = Color::srgb(0.3, 0.6, 0.9);
    pub const STATUS_WARNING: Color = Color::srgb(0.9, 0.7, 0.2);
    pub const STATUS_OFFLINE: Color = Color::srgb(0.5, 0.5, 0.5);
    pub const STATUS_ERROR: Color = Color::srgb(0.9, 0.3, 0.3);
}

/// Button style variants
#[derive(Debug, Clone, Copy, Default)]
pub enum ButtonStyle {
    #[default]
    Primary,
    Secondary,
    Danger,
}

impl ButtonStyle {
    pub fn background_color(&self) -> Color {
        match self {
            ButtonStyle::Primary => colors::BUTTON_PRIMARY,
            ButtonStyle::Secondary => colors::BUTTON_SECONDARY,
            ButtonStyle::Danger => colors::BUTTON_DANGER,
        }
    }

    pub fn hover_color(&self) -> Color {
        match self {
            ButtonStyle::Primary => colors::BUTTON_PRIMARY_HOVER,
            ButtonStyle::Secondary => colors::BUTTON_SECONDARY_HOVER,
            ButtonStyle::Danger => colors::BUTTON_DANGER_HOVER,
        }
    }
}

/// Builder for creating styled buttons
#[derive(Debug, Clone)]
pub struct UiButton {
    label: String,
    style: ButtonStyle,
    width: Val,
    height: Val,
    font_size: f32,
    enabled: bool,
}

impl UiButton {
    /// Create a new button with default primary style
    pub fn new(label: impl Into<String>) -> Self {
        Self {
            label: label.into(),
            style: ButtonStyle::Primary,
            width: Val::Px(150.0),
            height: Val::Px(40.0),
            font_size: 16.0,
            enabled: true,
        }
    }

    /// Create a primary styled button
    pub fn primary(label: impl Into<String>) -> Self {
        Self::new(label).style(ButtonStyle::Primary)
    }

    /// Create a secondary styled button
    pub fn secondary(label: impl Into<String>) -> Self {
        Self::new(label).style(ButtonStyle::Secondary)
    }

    /// Create a danger styled button
    pub fn danger(label: impl Into<String>) -> Self {
        Self::new(label).style(ButtonStyle::Danger)
    }

    /// Set the button style
    pub fn style(mut self, style: ButtonStyle) -> Self {
        self.style = style;
        self
    }

    /// Set the button width
    pub fn width(mut self, width: f32) -> Self {
        self.width = Val::Px(width);
        self
    }

    /// Set the button height
    pub fn height(mut self, height: f32) -> Self {
        self.height = Val::Px(height);
        self
    }

    /// Set the font size
    pub fn font_size(mut self, size: f32) -> Self {
        self.font_size = size;
        self
    }

    /// Set whether the button is enabled
    pub fn enabled(mut self, enabled: bool) -> Self {
        self.enabled = enabled;
        self
    }

    /// Build the button bundle
    pub fn build(self) -> impl Bundle {
        let bg_color = if self.enabled {
            self.style.background_color()
        } else {
            colors::BUTTON_DISABLED
        };

        (
            Button,
            Node {
                width: self.width,
                height: self.height,
                justify_content: JustifyContent::Center,
                align_items: AlignItems::Center,
                ..default()
            },
            BackgroundColor(bg_color),
            BorderRadius::all(Val::Px(6.0)),
        )
    }

    /// Build the button and spawn with text child
    pub fn spawn(self, commands: &mut Commands) -> Entity {
        let label = self.label.clone();
        let font_size = self.font_size;
        let enabled = self.enabled;

        commands
            .spawn(self.build())
            .with_children(|parent| {
                parent.spawn((
                    Text::new(label),
                    TextFont {
                        font_size,
                        ..default()
                    },
                    TextColor(if enabled {
                        colors::TEXT_PRIMARY
                    } else {
                        colors::TEXT_MUTED
                    }),
                ));
            })
            .id()
    }
}

/// Builder for creating styled panels
#[derive(Debug, Clone)]
pub struct UiPanel {
    title: Option<String>,
    width: Val,
    height: Val,
    padding: f32,
    closable: bool,
}

impl UiPanel {
    /// Create a new panel
    pub fn new() -> Self {
        Self {
            title: None,
            width: Val::Px(300.0),
            height: Val::Auto,
            padding: 12.0,
            closable: false,
        }
    }

    /// Set the panel title
    pub fn title(mut self, title: impl Into<String>) -> Self {
        self.title = Some(title.into());
        self
    }

    /// Set the panel width
    pub fn width(mut self, width: f32) -> Self {
        self.width = Val::Px(width);
        self
    }

    /// Set the panel height
    pub fn height(mut self, height: f32) -> Self {
        self.height = Val::Px(height);
        self
    }

    /// Set the panel padding
    pub fn padding(mut self, padding: f32) -> Self {
        self.padding = padding;
        self
    }

    /// Make the panel closable
    pub fn closable(mut self) -> Self {
        self.closable = true;
        self
    }

    /// Build the panel bundle
    pub fn build(self) -> impl Bundle {
        (
            Node {
                width: self.width,
                height: self.height,
                flex_direction: FlexDirection::Column,
                padding: UiRect::all(Val::Px(self.padding)),
                row_gap: Val::Px(8.0),
                ..default()
            },
            BackgroundColor(colors::PANEL_BG),
            BorderRadius::all(Val::Px(8.0)),
        )
    }
}

impl Default for UiPanel {
    fn default() -> Self {
        Self::new()
    }
}

/// Builder for creating progress bars
#[derive(Debug, Clone)]
pub struct UiProgressBar {
    progress: f32,
    width: f32,
    height: f32,
    fill_color: Color,
    bg_color: Color,
}

impl UiProgressBar {
    /// Create a new progress bar (progress 0.0 - 1.0)
    pub fn new(progress: f32) -> Self {
        Self {
            progress: progress.clamp(0.0, 1.0),
            width: 200.0,
            height: 16.0,
            fill_color: colors::STATUS_AVAILABLE,
            bg_color: colors::HEADER_BG,
        }
    }

    /// Set the progress bar width
    pub fn width(mut self, width: f32) -> Self {
        self.width = width;
        self
    }

    /// Set the progress bar height
    pub fn height(mut self, height: f32) -> Self {
        self.height = height;
        self
    }

    /// Set the fill color
    pub fn fill_color(mut self, color: Color) -> Self {
        self.fill_color = color;
        self
    }

    /// Build the progress bar container bundle
    pub fn build_container(self) -> impl Bundle {
        (
            Node {
                width: Val::Px(self.width),
                height: Val::Px(self.height),
                ..default()
            },
            BackgroundColor(self.bg_color),
            BorderRadius::all(Val::Px(self.height / 4.0)),
        )
    }

    /// Build the progress bar fill bundle
    pub fn build_fill(&self) -> impl Bundle {
        (
            Node {
                width: Val::Percent(self.progress * 100.0),
                height: Val::Percent(100.0),
                ..default()
            },
            BackgroundColor(self.fill_color),
            BorderRadius::all(Val::Px(self.height / 4.0)),
        )
    }
}

/// Helper function to create a labeled value display
pub fn spawn_labeled_value(commands: &mut ChildSpawner, label: &str, value: &str) -> Entity {
    commands
        .spawn((Node {
            flex_direction: FlexDirection::Row,
            justify_content: JustifyContent::SpaceBetween,
            width: Val::Percent(100.0),
            ..default()
        },))
        .with_children(|row| {
            row.spawn((
                Text::new(label),
                TextFont {
                    font_size: 14.0,
                    ..default()
                },
                TextColor(colors::TEXT_SECONDARY),
            ));
            row.spawn((
                Text::new(value),
                TextFont {
                    font_size: 14.0,
                    ..default()
                },
                TextColor(colors::TEXT_PRIMARY),
            ));
        })
        .id()
}

/// Helper function to create a section header
pub fn spawn_section_header(commands: &mut ChildSpawner, title: &str) -> Entity {
    commands
        .spawn((Node {
            width: Val::Percent(100.0),
            padding: UiRect::new(Val::Px(0.0), Val::Px(0.0), Val::Px(4.0), Val::Px(8.0)),
            ..default()
        },))
        .with_children(|header| {
            header.spawn((
                Text::new(title),
                TextFont {
                    font_size: 16.0,
                    ..default()
                },
                TextColor(colors::TEXT_SUCCESS),
            ));
        })
        .id()
}

/// Helper function to create a divider line
pub fn spawn_divider(commands: &mut ChildSpawner) -> Entity {
    commands
        .spawn((
            Node {
                width: Val::Percent(100.0),
                height: Val::Px(1.0),
                margin: UiRect::new(Val::Px(0.0), Val::Px(0.0), Val::Px(8.0), Val::Px(8.0)),
                ..default()
            },
            BackgroundColor(colors::HEADER_BG),
        ))
        .id()
}
