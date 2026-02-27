//! Leaderboard API operations

use bevy::prelude::*;
use serde::{Deserialize, Serialize};

use crate::api::SupabaseConfig;
use crate::resources::LeaderboardEntry;

/// Request to submit or update a score
#[derive(Debug, Clone, Serialize)]
pub struct SubmitScoreRequest {
    pub player_name: String,
    pub score: i64,
}

/// Response from submitting a score
#[derive(Debug, Clone, Deserialize)]
pub struct SubmitScoreResponse {
    pub id: String,
    pub player_name: String,
    pub score: i64,
    #[serde(default)]
    pub submitted_at: Option<String>,
    #[serde(default)]
    pub updated_at: Option<String>,
}

/// Result of a leaderboard operation
pub type LeaderboardResult<T> = Result<T, LeaderboardError>;

/// Errors that can occur during leaderboard operations
#[derive(Debug, Clone)]
pub enum LeaderboardError {
    /// Network error
    Network(String),
    /// Supabase not configured
    NotConfigured,
    /// Invalid response from server
    InvalidResponse(String),
    /// Server error
    ServerError(u16, String),
}

impl std::fmt::Display for LeaderboardError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            LeaderboardError::Network(msg) => write!(f, "Network error: {}", msg),
            LeaderboardError::NotConfigured => write!(f, "Supabase not configured"),
            LeaderboardError::InvalidResponse(msg) => write!(f, "Invalid response: {}", msg),
            LeaderboardError::ServerError(code, msg) => {
                write!(f, "Server error {}: {}", code, msg)
            }
        }
    }
}

impl std::error::Error for LeaderboardError {}

/// Fetch the leaderboard from Supabase
pub async fn fetch_leaderboard(
    config: &SupabaseConfig,
    limit: usize,
) -> LeaderboardResult<Vec<LeaderboardEntry>> {
    use bevy::log::{debug, warn};

    let url = format!(
        "{}?order=score.desc&limit={}",
        config.table_url("leaderboard"),
        limit
    );

    debug!("Fetching leaderboard from: {}", url);

    let client = reqwest::Client::new();
    let response = client
        .get(&url)
        .header("apikey", &config.anon_key)
        .header("Authorization", config.auth_header())
        .send()
        .await
        .map_err(|e| {
            warn!("Network error fetching leaderboard from {}: {:?}", url, e);
            LeaderboardError::Network(format!("{:?}", e))
        })?;

    let status = response.status();
    debug!("Leaderboard response status: {}", status);

    if !status.is_success() {
        let status_code = status.as_u16();
        let body = response
            .text()
            .await
            .unwrap_or_else(|_| "Unknown error".to_string());
        warn!(
            "Leaderboard fetch failed with status {}: {}",
            status_code, body
        );
        return Err(LeaderboardError::ServerError(status_code, body));
    }

    // Get the raw text first for debugging
    let body_text = response.text().await.map_err(|e| {
        warn!("Failed to read response body: {}", e);
        LeaderboardError::Network(format!("Failed to read response: {}", e))
    })?;

    debug!("Leaderboard response body: {}", body_text);

    // Now parse it
    let entries: Vec<LeaderboardEntry> = serde_json::from_str(&body_text).map_err(|e| {
        warn!("Failed to parse leaderboard JSON: {}", e);
        warn!("Response body was: {}", body_text);
        LeaderboardError::InvalidResponse(format!("{} (body: {})", e, body_text))
    })?;

    debug!("Successfully parsed {} leaderboard entries", entries.len());
    Ok(entries)
}

/// Submit a new score to the leaderboard
pub async fn submit_score(
    config: &SupabaseConfig,
    player_id: Option<&str>,
    player_name: &str,
    score: i64,
) -> LeaderboardResult<SubmitScoreResponse> {
    use bevy::log::{info, warn};

    info!(
        "submit_score called: player_id={:?}, player_name={}, score={}",
        player_id, player_name, score
    );

    let client = reqwest::Client::new();

    // If we have a player_id, try to update existing record
    if let Some(id) = player_id {
        info!("Player has existing ID, checking for update: {}", id);
        // First, fetch the existing record to check if the new score is higher
        let get_url = format!("{}?id=eq.{}", config.table_url("leaderboard"), id);
        let existing = client
            .get(&get_url)
            .header("apikey", &config.anon_key)
            .header("Authorization", config.auth_header())
            .send()
            .await
            .map_err(|e| LeaderboardError::Network(e.to_string()))?;

        if existing.status().is_success() {
            let records: Vec<SubmitScoreResponse> = existing
                .json()
                .await
                .map_err(|e| LeaderboardError::InvalidResponse(e.to_string()))?;

            if let Some(existing_record) = records.first() {
                // Only update if new score is higher
                if score > existing_record.score {
                    let update_url = format!("{}?id=eq.{}", config.table_url("leaderboard"), id);
                    let response = client
                        .patch(&update_url)
                        .header("apikey", &config.anon_key)
                        .header("Authorization", config.auth_header())
                        .header("Content-Type", "application/json")
                        .header("Prefer", "return=representation")
                        .json(&serde_json::json!({
                            "score": score,
                            "updated_at": chrono::Utc::now().to_rfc3339(),
                        }))
                        .send()
                        .await
                        .map_err(|e| LeaderboardError::Network(e.to_string()))?;

                    if !response.status().is_success() {
                        let status = response.status().as_u16();
                        let body = response
                            .text()
                            .await
                            .unwrap_or_else(|_| "Unknown error".to_string());
                        return Err(LeaderboardError::ServerError(status, body));
                    }

                    let updated: Vec<SubmitScoreResponse> = response
                        .json()
                        .await
                        .map_err(|e| LeaderboardError::InvalidResponse(e.to_string()))?;

                    return updated.into_iter().next().ok_or_else(|| {
                        LeaderboardError::InvalidResponse("Empty response".to_string())
                    });
                } else {
                    // Score is not higher, return existing record
                    return Ok(existing_record.clone());
                }
            }
        }
    }

    // Insert new record
    let url = config.table_url("leaderboard");
    let request = SubmitScoreRequest {
        player_name: player_name.to_string(),
        score,
    };

    info!(
        "Inserting new leaderboard entry: POST {} with {:?}",
        url, request
    );

    let response = client
        .post(&url)
        .header("apikey", &config.anon_key)
        .header("Authorization", config.auth_header())
        .header("Content-Type", "application/json")
        .header("Prefer", "return=representation")
        .json(&request)
        .send()
        .await
        .map_err(|e| {
            warn!("Network error during POST: {}", e);
            LeaderboardError::Network(e.to_string())
        })?;

    info!("POST response status: {}", response.status());

    if !response.status().is_success() {
        let status = response.status().as_u16();
        let body = response
            .text()
            .await
            .unwrap_or_else(|_| "Unknown error".to_string());
        return Err(LeaderboardError::ServerError(status, body));
    }

    let result: Vec<SubmitScoreResponse> = response
        .json()
        .await
        .map_err(|e| LeaderboardError::InvalidResponse(e.to_string()))?;

    result
        .into_iter()
        .next()
        .ok_or_else(|| LeaderboardError::InvalidResponse("Empty response".to_string()))
}
