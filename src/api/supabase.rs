//! Supabase client configuration

use bevy::prelude::*;

/// Strip invisible/non-ASCII characters that can slip in via copy-paste into
/// GitHub secrets and are invalid in URLs and HTTP header values.
fn sanitize_credential(s: &str) -> String {
    s.chars()
        .filter(|c| c.is_ascii() && !c.is_ascii_control())
        .collect::<String>()
        .trim()
        .to_string()
}

/// Supabase configuration resource
#[derive(Resource, Clone, Debug)]
pub struct SupabaseConfig {
    /// Supabase project URL
    pub url: String,
    /// Supabase anonymous/public API key
    pub anon_key: String,
}

impl SupabaseConfig {
    /// Create a new Supabase configuration
    pub fn new(url: impl Into<String>, anon_key: impl Into<String>) -> Self {
        Self {
            url: url.into(),
            anon_key: anon_key.into(),
        }
    }

    /// Load configuration from compile-time env vars (baked into the binary via
    /// `option_env!`), falling back to runtime env vars for native dev builds.
    /// Returns `None` when neither source provides credentials.
    pub fn from_env() -> Option<Self> {
        let url = option_env!("SUPABASE_URL")
            .map(sanitize_credential)
            .or_else(|| {
                std::env::var("SUPABASE_URL")
                    .ok()
                    .map(|s| sanitize_credential(&s))
            })
            .filter(|s| !s.is_empty());
        let anon_key = option_env!("SUPABASE_ANON_KEY")
            .map(sanitize_credential)
            .or_else(|| {
                std::env::var("SUPABASE_ANON_KEY")
                    .ok()
                    .map(|s| sanitize_credential(&s))
            })
            .filter(|s| !s.is_empty());

        match (url, anon_key) {
            (Some(url), Some(anon_key)) => Some(Self { url, anon_key }),
            _ => None,
        }
    }

    /// Get the full URL for a table endpoint
    pub fn table_url(&self, table: &str) -> String {
        format!("{}/rest/v1/{}", self.url, table)
    }

    /// Get the authorization header value
    pub fn auth_header(&self) -> String {
        format!("Bearer {}", self.anon_key)
    }
}
