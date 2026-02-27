//! Leaderboard data structures and resources

use bevy::prelude::*;
use serde::{Deserialize, Serialize};

/// A single leaderboard entry
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LeaderboardEntry {
    /// Unique identifier from Supabase
    pub id: String,
    /// Player's display name
    pub player_name: String,
    /// Player's score
    pub score: i64,
    /// ISO 8601 timestamp when first submitted (may be null if not set in DB)
    #[serde(default)]
    pub submitted_at: Option<String>,
    /// ISO 8601 timestamp when last updated (may be null if not set in DB)
    #[serde(default)]
    pub updated_at: Option<String>,
}

/// Resource containing leaderboard data
///
/// Uses `f64` elapsed seconds tracked via Bevy's `Time` resource instead of
/// `std::time::Instant`, which panics on WASM.
#[derive(Resource, Default, Debug)]
pub struct LeaderboardData {
    /// List of leaderboard entries sorted by score (descending)
    pub entries: Vec<LeaderboardEntry>,
    /// Total elapsed seconds (from Bevy Time) when the leaderboard was last fetched
    pub last_fetched_at_secs: Option<f64>,
    /// Whether a fetch is currently in progress
    pub is_loading: bool,
    /// Error message if the last fetch failed
    pub error: Option<String>,
    /// Number of consecutive failed attempts
    pub failed_attempts: u32,
    /// Total elapsed seconds (from Bevy Time) when the last failure occurred
    pub last_failed_at_secs: Option<f64>,
}

impl LeaderboardData {
    /// Mark as loading
    pub fn start_loading(&mut self) {
        self.is_loading = true;
        self.error = None;
    }

    /// Update with successful fetch
    pub fn update_entries(&mut self, entries: Vec<LeaderboardEntry>, now_secs: f64) {
        self.entries = entries;
        self.last_fetched_at_secs = Some(now_secs);
        self.is_loading = false;
        self.error = None;
        self.failed_attempts = 0;
        self.last_failed_at_secs = None;
    }

    /// Mark as failed with error
    pub fn set_error(&mut self, error: impl Into<String>, now_secs: f64) {
        self.error = Some(error.into());
        self.is_loading = false;
        self.failed_attempts += 1;
        self.last_failed_at_secs = Some(now_secs);
    }

    /// Clear error
    pub fn clear_error(&mut self) {
        self.error = None;
    }

    /// Calculate backoff duration in seconds based on failed attempts.
    /// Uses exponential backoff: 2^attempts seconds, capped at 5 minutes.
    pub fn backoff_secs(&self) -> f64 {
        if self.failed_attempts == 0 {
            return 0.0;
        }
        // 2, 4, 8, 16, 32, 64, 128, 256 seconds, capped at 300 (5 min)
        let seconds = 2u64.pow(self.failed_attempts.min(8)).min(300);
        seconds as f64
    }

    /// Check if enough time has passed since the last failed attempt to retry
    pub fn can_retry(&self, now_secs: f64) -> bool {
        if self.failed_attempts == 0 {
            return true;
        }
        if let Some(failed_at) = self.last_failed_at_secs {
            (now_secs - failed_at) >= self.backoff_secs()
        } else {
            true
        }
    }

    /// Check if we should refresh (never fetched, or data older than `stale_secs`)
    pub fn should_fetch(&self, now_secs: f64, stale_secs: f64) -> bool {
        match self.last_fetched_at_secs {
            None => true,
            Some(fetched_at) => (now_secs - fetched_at) > stale_secs,
        }
    }

    /// Seconds remaining until the next retry is allowed (for UI display)
    pub fn retry_remaining_secs(&self, now_secs: f64) -> f64 {
        if let Some(failed_at) = self.last_failed_at_secs {
            let backoff = self.backoff_secs();
            let elapsed = now_secs - failed_at;
            (backoff - elapsed).max(0.0)
        } else {
            0.0
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_entry(name: &str, score: i64) -> LeaderboardEntry {
        LeaderboardEntry {
            id: format!("id-{}", name),
            player_name: name.to_string(),
            score,
            submitted_at: None,
            updated_at: None,
        }
    }

    // ---- Default state ----

    #[test]
    fn test_default_state() {
        let data = LeaderboardData::default();
        assert!(data.entries.is_empty());
        assert!(data.last_fetched_at_secs.is_none());
        assert!(!data.is_loading);
        assert!(data.error.is_none());
        assert_eq!(data.failed_attempts, 0);
        assert!(data.last_failed_at_secs.is_none());
    }

    // ---- start_loading ----

    #[test]
    fn test_start_loading_sets_flag_and_clears_error() {
        let mut data = LeaderboardData {
            error: Some("old error".to_string()),
            ..Default::default()
        };

        data.start_loading();

        assert!(data.is_loading);
        assert!(data.error.is_none());
    }

    // ---- update_entries ----

    #[test]
    fn test_update_entries_stores_data_and_resets_state() {
        let mut data = LeaderboardData {
            is_loading: true,
            error: Some("stale error".to_string()),
            failed_attempts: 3,
            last_failed_at_secs: Some(10.0),
            ..Default::default()
        };

        let entries = vec![make_entry("Alice", 100), make_entry("Bob", 50)];
        data.update_entries(entries, 42.0);

        assert_eq!(data.entries.len(), 2);
        assert_eq!(data.entries[0].player_name, "Alice");
        assert_eq!(data.last_fetched_at_secs, Some(42.0));
        assert!(!data.is_loading);
        assert!(data.error.is_none());
        assert_eq!(data.failed_attempts, 0);
        assert!(data.last_failed_at_secs.is_none());
    }

    // ---- set_error ----

    #[test]
    fn test_set_error_records_failure() {
        let mut data = LeaderboardData {
            is_loading: true,
            ..Default::default()
        };

        data.set_error("connection refused", 15.0);

        assert!(!data.is_loading);
        assert_eq!(data.error.as_deref(), Some("connection refused"));
        assert_eq!(data.failed_attempts, 1);
        assert_eq!(data.last_failed_at_secs, Some(15.0));
    }

    #[test]
    fn test_set_error_increments_failed_attempts() {
        let mut data = LeaderboardData::default();

        data.set_error("err1", 10.0);
        assert_eq!(data.failed_attempts, 1);

        data.set_error("err2", 20.0);
        assert_eq!(data.failed_attempts, 2);

        data.set_error("err3", 30.0);
        assert_eq!(data.failed_attempts, 3);
        assert_eq!(data.last_failed_at_secs, Some(30.0));
    }

    // ---- clear_error ----

    #[test]
    fn test_clear_error_only_clears_error() {
        let mut data = LeaderboardData::default();
        data.set_error("something broke", 5.0);

        data.clear_error();

        assert!(data.error.is_none());
        // failed_attempts and last_failed_at_secs are NOT cleared
        assert_eq!(data.failed_attempts, 1);
        assert_eq!(data.last_failed_at_secs, Some(5.0));
    }

    // ---- backoff_secs ----

    #[test]
    fn test_backoff_secs_zero_failures() {
        let data = LeaderboardData::default();
        assert_eq!(data.backoff_secs(), 0.0);
    }

    #[test]
    fn test_backoff_secs_exponential() {
        let mut data = LeaderboardData::default();

        let expected = [
            (1, 2.0),
            (2, 4.0),
            (3, 8.0),
            (4, 16.0),
            (5, 32.0),
            (6, 64.0),
        ];
        for (attempts, expected_backoff) in expected {
            data.failed_attempts = attempts;
            assert_eq!(
                data.backoff_secs(),
                expected_backoff,
                "backoff for {} attempts",
                attempts
            );
        }
    }

    #[test]
    fn test_backoff_secs_capped_by_exponent_limit() {
        // Exponent is clamped to 8 via .min(8), so max is 2^8 = 256
        let data = LeaderboardData {
            failed_attempts: 9,
            ..Default::default()
        };
        assert_eq!(data.backoff_secs(), 256.0);

        // Very high attempt count also produces 256 (min(100, 8) = 8, 2^8 = 256)
        let data = LeaderboardData {
            failed_attempts: 100,
            ..Default::default()
        };
        assert_eq!(data.backoff_secs(), 256.0);
    }

    // ---- can_retry ----

    #[test]
    fn test_can_retry_no_failures() {
        let data = LeaderboardData::default();
        assert!(data.can_retry(0.0));
        assert!(data.can_retry(1000.0));
    }

    #[test]
    fn test_can_retry_within_backoff_window() {
        let data = LeaderboardData {
            failed_attempts: 1, // backoff = 2s
            last_failed_at_secs: Some(10.0),
            ..Default::default()
        };

        // 1 second later — still within 2s backoff
        assert!(!data.can_retry(11.0));
    }

    #[test]
    fn test_can_retry_at_backoff_boundary() {
        let data = LeaderboardData {
            failed_attempts: 1, // backoff = 2s
            last_failed_at_secs: Some(10.0),
            ..Default::default()
        };

        // Exactly at backoff boundary
        assert!(data.can_retry(12.0));
    }

    #[test]
    fn test_can_retry_after_backoff() {
        let data = LeaderboardData {
            failed_attempts: 2, // backoff = 4s
            last_failed_at_secs: Some(10.0),
            ..Default::default()
        };

        assert!(!data.can_retry(13.0)); // 3s elapsed < 4s
        assert!(data.can_retry(14.0)); // 4s elapsed = 4s
        assert!(data.can_retry(100.0)); // well past
    }

    #[test]
    fn test_can_retry_with_failures_but_no_timestamp() {
        let data = LeaderboardData {
            failed_attempts: 5,
            ..Default::default()
        };

        // Edge case: failures recorded but no timestamp — should allow retry
        assert!(data.can_retry(0.0));
    }

    // ---- should_fetch ----

    #[test]
    fn test_should_fetch_never_fetched() {
        let data = LeaderboardData::default();
        assert!(data.should_fetch(0.0, 30.0));
        assert!(data.should_fetch(9999.0, 30.0));
    }

    #[test]
    fn test_should_fetch_within_stale_window() {
        let data = LeaderboardData {
            last_fetched_at_secs: Some(100.0),
            ..Default::default()
        };

        // 20s later — still fresh (stale after 30s)
        assert!(!data.should_fetch(120.0, 30.0));
    }

    #[test]
    fn test_should_fetch_at_stale_boundary() {
        let data = LeaderboardData {
            last_fetched_at_secs: Some(100.0),
            ..Default::default()
        };

        // Exactly at 30s — not stale yet (uses > not >=)
        assert!(!data.should_fetch(130.0, 30.0));
    }

    #[test]
    fn test_should_fetch_after_stale_window() {
        let data = LeaderboardData {
            last_fetched_at_secs: Some(100.0),
            ..Default::default()
        };

        // 31s later — stale
        assert!(data.should_fetch(131.0, 30.0));
    }

    // ---- retry_remaining_secs ----

    #[test]
    fn test_retry_remaining_no_failure() {
        let data = LeaderboardData::default();
        assert_eq!(data.retry_remaining_secs(50.0), 0.0);
    }

    #[test]
    fn test_retry_remaining_during_backoff() {
        let data = LeaderboardData {
            failed_attempts: 1, // backoff = 2s
            last_failed_at_secs: Some(10.0),
            ..Default::default()
        };

        // 0.5s into a 2s backoff → 1.5s remaining
        assert!((data.retry_remaining_secs(10.5) - 1.5).abs() < f64::EPSILON);
    }

    #[test]
    fn test_retry_remaining_after_backoff_expired() {
        let data = LeaderboardData {
            failed_attempts: 1, // backoff = 2s
            last_failed_at_secs: Some(10.0),
            ..Default::default()
        };

        // 5s into a 2s backoff → 0 remaining (clamped)
        assert_eq!(data.retry_remaining_secs(15.0), 0.0);
    }

    // ---- Full lifecycle ----

    #[test]
    fn test_loading_then_success_resets_everything() {
        let mut data = LeaderboardData::default();

        // Simulate a failed attempt first
        data.set_error("timeout", 5.0);
        assert_eq!(data.failed_attempts, 1);

        // Then a successful fetch
        data.start_loading();
        assert!(data.is_loading);

        data.update_entries(vec![make_entry("Winner", 999)], 10.0);
        assert!(!data.is_loading);
        assert!(data.error.is_none());
        assert_eq!(data.failed_attempts, 0);
        assert!(data.last_failed_at_secs.is_none());
        assert_eq!(data.entries.len(), 1);
    }

    #[test]
    fn test_multiple_failures_then_success() {
        let mut data = LeaderboardData::default();

        // Three consecutive failures
        data.set_error("err1", 10.0);
        data.set_error("err2", 20.0);
        data.set_error("err3", 30.0);
        assert_eq!(data.failed_attempts, 3);
        assert_eq!(data.backoff_secs(), 8.0); // 2^3

        // Then success resets everything
        data.update_entries(vec![], 50.0);
        assert_eq!(data.failed_attempts, 0);
        assert_eq!(data.backoff_secs(), 0.0);
        assert!(data.can_retry(50.0));
    }
}
