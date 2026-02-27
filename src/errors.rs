//! Error types and result handling for ChargeOps Simulator.
//!
//! This module provides error types for fallible operations
//! and utilities for error propagation in systems.
//!
//! # Usage
//!
//! ## Custom Error Types
//!
//! ```rust,ignore
//! use chargeopssim::errors::{ChargeOpsResult, ChargeOpsError};
//!
//! fn load_scenario(path: &str) -> ChargeOpsResult<ScenarioData> {
//!     let contents = std::fs::read_to_string(path)
//!         .map_err(|e| ChargeOpsError::IoError(e.to_string()))?;
//!     
//!     serde_json::from_str(&contents)
//!         .map_err(|e| ChargeOpsError::DataParseError(e.to_string()))
//! }
//! ```
//!
//! ## Fallible Systems
//!
//! Systems can return `Result<(), BevyError>` and errors will be handled by the
//! global error handler (configured to warn instead of panic):
//!
//! ```rust,ignore
//! use bevy::prelude::*;
//!
//! fn fallible_system(query: Query<&Transform>) -> Result {
//!     let transform = query.get_single()?;
//!     info!("Transform: {:?}", transform);
//!     Ok(())
//! }
//! ```
//!
//! ## System Piping
//!
//! Systems can be piped together to transform or handle their results:
//!
//! ```rust,ignore
//! app.add_systems(Update,
//!     my_fallible_system.pipe(|In(result): In<Result>| {
//!         if let Err(e) = result {
//!             warn!("System failed: {}", e);
//!         }
//!     })
//! );
//! ```

use bevy::prelude::*;
use std::fmt;

/// Plugin that configures the global error handling strategy.
///
/// By default, Bevy panics when a fallible system returns an error.
/// This plugin sets the error handler to log warnings instead,
/// which is more appropriate for a game where we want to continue
/// running even if something fails.
pub struct ErrorsPlugin;

impl Plugin for ErrorsPlugin {
    fn build(&self, app: &mut App) {
        // Set the global error handler to warn instead of panic.
        // This affects:
        // - Fallible systems that return Result
        // - Failed command operations (e.g., entity not found)
        // - System parameter validation failures
        app.set_error_handler(bevy::ecs::error::warn);

        info!("ErrorsPlugin initialized with warn error handler");
    }
}

/// Result type alias for ChargeOps operations.
pub type ChargeOpsResult<T> = Result<T, ChargeOpsError>;

/// Main error type for ChargeOps Simulator.
#[derive(Debug, Clone)]
pub enum ChargeOpsError {
    /// An entity was not found.
    EntityNotFound {
        entity_type: &'static str,
        id: String,
    },

    /// A component was not found on an entity.
    ComponentNotFound {
        component_type: &'static str,
        entity_id: String,
    },

    /// A resource was not found.
    ResourceNotFound { resource_type: &'static str },

    /// Data parsing failed.
    DataParseError(String),

    /// IO operation failed.
    IoError(String),

    /// Asset loading failed.
    AssetError { path: String, reason: String },

    /// Invalid game state for an operation.
    InvalidState { expected: String, actual: String },

    /// Invalid configuration.
    ConfigError(String),

    /// An operation failed validation.
    ValidationError { field: String, reason: String },

    /// A remote action failed.
    ActionFailed { action: String, reason: String },

    /// Generic error with message.
    Other(String),
}

impl fmt::Display for ChargeOpsError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ChargeOpsError::EntityNotFound { entity_type, id } => {
                write!(f, "{entity_type} not found: {id}")
            }
            ChargeOpsError::ComponentNotFound {
                component_type,
                entity_id,
            } => {
                write!(
                    f,
                    "{component_type} component not found on entity {entity_id}"
                )
            }
            ChargeOpsError::ResourceNotFound { resource_type } => {
                write!(f, "Resource not found: {resource_type}")
            }
            ChargeOpsError::DataParseError(msg) => {
                write!(f, "Data parse error: {msg}")
            }
            ChargeOpsError::IoError(msg) => {
                write!(f, "IO error: {msg}")
            }
            ChargeOpsError::AssetError { path, reason } => {
                write!(f, "Asset error loading '{path}': {reason}")
            }
            ChargeOpsError::InvalidState { expected, actual } => {
                write!(f, "Invalid state: expected {expected}, got {actual}")
            }
            ChargeOpsError::ConfigError(msg) => {
                write!(f, "Configuration error: {msg}")
            }
            ChargeOpsError::ValidationError { field, reason } => {
                write!(f, "Validation error for '{field}': {reason}")
            }
            ChargeOpsError::ActionFailed { action, reason } => {
                write!(f, "Action '{action}' failed: {reason}")
            }
            ChargeOpsError::Other(msg) => {
                write!(f, "{msg}")
            }
        }
    }
}

impl std::error::Error for ChargeOpsError {}

// Conversion from common error types

impl From<std::io::Error> for ChargeOpsError {
    fn from(err: std::io::Error) -> Self {
        ChargeOpsError::IoError(err.to_string())
    }
}

impl From<serde_json::Error> for ChargeOpsError {
    fn from(err: serde_json::Error) -> Self {
        ChargeOpsError::DataParseError(err.to_string())
    }
}

impl From<String> for ChargeOpsError {
    fn from(msg: String) -> Self {
        ChargeOpsError::Other(msg)
    }
}

impl From<&str> for ChargeOpsError {
    fn from(msg: &str) -> Self {
        ChargeOpsError::Other(msg.to_string())
    }
}

// Helper constructors

impl ChargeOpsError {
    /// Create an entity not found error.
    pub fn entity_not_found(entity_type: &'static str, id: impl Into<String>) -> Self {
        ChargeOpsError::EntityNotFound {
            entity_type,
            id: id.into(),
        }
    }

    /// Create a component not found error.
    pub fn component_not_found(component_type: &'static str, entity_id: impl Into<String>) -> Self {
        ChargeOpsError::ComponentNotFound {
            component_type,
            entity_id: entity_id.into(),
        }
    }

    /// Create a resource not found error.
    pub fn resource_not_found(resource_type: &'static str) -> Self {
        ChargeOpsError::ResourceNotFound { resource_type }
    }

    /// Create an asset error.
    pub fn asset_error(path: impl Into<String>, reason: impl Into<String>) -> Self {
        ChargeOpsError::AssetError {
            path: path.into(),
            reason: reason.into(),
        }
    }

    /// Create an invalid state error.
    pub fn invalid_state(expected: impl Into<String>, actual: impl Into<String>) -> Self {
        ChargeOpsError::InvalidState {
            expected: expected.into(),
            actual: actual.into(),
        }
    }

    /// Create a validation error.
    pub fn validation(field: impl Into<String>, reason: impl Into<String>) -> Self {
        ChargeOpsError::ValidationError {
            field: field.into(),
            reason: reason.into(),
        }
    }

    /// Create an action failed error.
    pub fn action_failed(action: impl Into<String>, reason: impl Into<String>) -> Self {
        ChargeOpsError::ActionFailed {
            action: action.into(),
            reason: reason.into(),
        }
    }
}

// Extension traits for Result handling in systems

/// Extension trait for logging errors from Results.
pub trait ResultExt<T> {
    /// Log an error if present and return None.
    fn log_error(self) -> Option<T>;

    /// Log an error at warn level if present and return None.
    fn log_warning(self) -> Option<T>;

    /// Log an error and use a default value.
    fn unwrap_or_log(self, default: T) -> T;
}

impl<T, E: fmt::Display> ResultExt<T> for Result<T, E> {
    fn log_error(self) -> Option<T> {
        match self {
            Ok(v) => Some(v),
            Err(e) => {
                bevy::log::error!("{}", e);
                None
            }
        }
    }

    fn log_warning(self) -> Option<T> {
        match self {
            Ok(v) => Some(v),
            Err(e) => {
                bevy::log::warn!("{}", e);
                None
            }
        }
    }

    fn unwrap_or_log(self, default: T) -> T {
        match self {
            Ok(v) => v,
            Err(e) => {
                bevy::log::error!("{}", e);
                default
            }
        }
    }
}

/// Extension trait for Option types in systems.
pub trait OptionExt<T> {
    /// Log an error message if None and return None.
    fn or_log(self, msg: &str) -> Option<T>;

    /// Convert to Result with an error message.
    fn ok_or_log(self, msg: &str) -> ChargeOpsResult<T>;
}

impl<T> OptionExt<T> for Option<T> {
    fn or_log(self, msg: &str) -> Option<T> {
        if self.is_none() {
            bevy::log::error!("{}", msg);
        }
        self
    }

    fn ok_or_log(self, msg: &str) -> ChargeOpsResult<T> {
        self.ok_or_else(|| {
            bevy::log::error!("{}", msg);
            ChargeOpsError::Other(msg.to_string())
        })
    }
}

// ============================================================================
// System Piping Utilities
// ============================================================================

/// A system that handles results by logging errors at the error level.
///
/// Use with `.pipe()` to handle errors from fallible systems:
/// ```rust,ignore
/// app.add_systems(Update, my_fallible_system.pipe(log_system_error));
/// ```
pub fn log_system_error(In(result): In<Result>) {
    if let Err(e) = result {
        bevy::log::error!("System error: {}", e);
    }
}

/// A system that handles results by logging errors at the warn level.
///
/// Use with `.pipe()` to handle errors from fallible systems:
/// ```rust,ignore
/// app.add_systems(Update, my_fallible_system.pipe(log_system_warning));
/// ```
pub fn log_system_warning(In(result): In<Result>) {
    if let Err(e) = result {
        bevy::log::warn!("System warning: {}", e);
    }
}

/// A system that silently ignores errors.
///
/// Use with `.pipe()` to suppress errors from fallible systems:
/// ```rust,ignore
/// app.add_systems(Update, my_fallible_system.pipe(ignore_error));
/// ```
pub fn ignore_error(In(_result): In<Result>) {
    // Intentionally empty - errors are silently ignored
}

/// Extension trait for Query results to provide better error messages.
pub trait QueryResultExt<T> {
    /// Convert a query result to a ChargeOpsResult with a descriptive error.
    fn ok_or_not_found(self, entity_type: &'static str) -> ChargeOpsResult<T>;
}

impl<T> QueryResultExt<T> for std::result::Result<T, bevy::ecs::query::QuerySingleError> {
    fn ok_or_not_found(self, entity_type: &'static str) -> ChargeOpsResult<T> {
        self.map_err(|e| match e {
            bevy::ecs::query::QuerySingleError::NoEntities(_) => ChargeOpsError::EntityNotFound {
                entity_type,
                id: "none".to_string(),
            },
            bevy::ecs::query::QuerySingleError::MultipleEntities(_) => ChargeOpsError::Other(
                format!("Multiple {entity_type} entities found, expected one"),
            ),
        })
    }
}

// ============================================================================
// Fallible System Helpers
// ============================================================================

/// Trait for safely accessing resources with proper error handling.
pub trait WorldExt {
    /// Get a resource or return an error.
    fn get_resource_or_err<R: Resource>(&self) -> ChargeOpsResult<&R>;
}

impl WorldExt for World {
    fn get_resource_or_err<R: Resource>(&self) -> ChargeOpsResult<&R> {
        self.get_resource::<R>()
            .ok_or_else(|| ChargeOpsError::resource_not_found(std::any::type_name::<R>()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_error_display() {
        let err = ChargeOpsError::entity_not_found("Charger", "CHG-001");
        assert_eq!(err.to_string(), "Charger not found: CHG-001");

        let err = ChargeOpsError::validation("power_kw", "must be positive");
        assert_eq!(
            err.to_string(),
            "Validation error for 'power_kw': must be positive"
        );
    }

    #[test]
    fn test_error_conversion() {
        let err: ChargeOpsError = "test error".into();
        assert!(matches!(err, ChargeOpsError::Other(_)));

        let err: ChargeOpsError = String::from("test error").into();
        assert!(matches!(err, ChargeOpsError::Other(_)));
    }

    #[test]
    fn test_result_ext() {
        let ok_result: Result<i32, &str> = Ok(42);
        assert_eq!(ok_result.log_error(), Some(42));

        // Error case would log, but we can't easily test logging
        let err_result: Result<i32, &str> = Err("error");
        assert_eq!(err_result.unwrap_or_log(0), 0);
    }

    #[test]
    fn test_option_ext() {
        let some_val: Option<i32> = Some(42);
        assert_eq!(some_val.or_log("not found"), Some(42));

        let none_val: Option<i32> = None;
        assert!(none_val.ok_or_log("not found").is_err());
    }

    #[test]
    fn test_error_constructors() {
        let err = ChargeOpsError::component_not_found("Transform", "entity-123");
        assert!(matches!(err, ChargeOpsError::ComponentNotFound { .. }));
        assert!(err.to_string().contains("Transform"));

        let err = ChargeOpsError::resource_not_found("GameClock");
        assert!(matches!(err, ChargeOpsError::ResourceNotFound { .. }));

        let err = ChargeOpsError::asset_error("sprites/charger.svg", "file not found");
        assert!(matches!(err, ChargeOpsError::AssetError { .. }));

        let err = ChargeOpsError::invalid_state("Playing", "Paused");
        assert!(matches!(err, ChargeOpsError::InvalidState { .. }));

        let err = ChargeOpsError::action_failed("SoftReboot", "on cooldown");
        assert!(matches!(err, ChargeOpsError::ActionFailed { .. }));
    }

    #[test]
    fn test_io_error_conversion() {
        let io_err = std::io::Error::new(std::io::ErrorKind::NotFound, "file not found");
        let err: ChargeOpsError = io_err.into();
        assert!(matches!(err, ChargeOpsError::IoError(_)));
        assert!(err.to_string().contains("file not found"));
    }

    #[test]
    fn test_error_is_std_error() {
        let err = ChargeOpsError::Other("test".to_string());
        let _: &dyn std::error::Error = &err; // Ensure it implements Error trait
    }
}
