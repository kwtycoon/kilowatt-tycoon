//! Tests for leaderboard API operations with mocked Supabase server

use kilowatt_tycoon::api::{SupabaseConfig, fetch_leaderboard, submit_score};
use wiremock::matchers::{header, method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

/// Test fetching an empty leaderboard
#[tokio::test]
async fn test_fetch_empty_leaderboard() {
    // Start a mock server
    let mock_server = MockServer::start().await;

    // Configure the mock to return an empty array
    Mock::given(method("GET"))
        .and(path("/rest/v1/leaderboard"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!([])))
        .mount(&mock_server)
        .await;

    // Create config pointing to mock server
    let config = SupabaseConfig::new(mock_server.uri(), "test-key");

    // Fetch leaderboard
    let result = fetch_leaderboard(&config, 10).await;

    // Verify
    assert!(result.is_ok());
    let entries = result.unwrap();
    assert_eq!(entries.len(), 0);
}

/// Test fetching a leaderboard with entries
#[tokio::test]
async fn test_fetch_leaderboard_with_entries() {
    let mock_server = MockServer::start().await;

    // Mock response with three entries (timestamps can be null)
    let mock_entries = serde_json::json!([
        {
            "id": "00000000-0000-0000-0000-000000000001",
            "player_name": "Alice",
            "score": 100000,
            "submitted_at": null,
            "updated_at": null
        },
        {
            "id": "00000000-0000-0000-0000-000000000002",
            "player_name": "Bob",
            "score": 90000,
            "submitted_at": null,
            "updated_at": null
        },
        {
            "id": "00000000-0000-0000-0000-000000000003",
            "player_name": "Charlie",
            "score": 80000,
            "submitted_at": null,
            "updated_at": null
        }
    ]);

    Mock::given(method("GET"))
        .and(path("/rest/v1/leaderboard"))
        .respond_with(ResponseTemplate::new(200).set_body_json(mock_entries))
        .mount(&mock_server)
        .await;

    let config = SupabaseConfig::new(mock_server.uri(), "test-key");
    let result = fetch_leaderboard(&config, 10).await;

    assert!(result.is_ok());
    let entries = result.unwrap();
    assert_eq!(entries.len(), 3);
    assert_eq!(entries[0].player_name, "Alice");
    assert_eq!(entries[0].score, 100000);
    assert_eq!(entries[1].player_name, "Bob");
    assert_eq!(entries[2].player_name, "Charlie");
}

/// Test submitting a new score (first time submission)
#[tokio::test]
async fn test_submit_new_score() {
    let mock_server = MockServer::start().await;

    // Mock response for successful insertion (timestamps may be null)
    let mock_response = serde_json::json!([{
        "id": "00000000-0000-0000-0000-000000000123",
        "player_name": "NewPlayer",
        "score": 50000,
        "submitted_at": null,
        "updated_at": null
    }]);

    Mock::given(method("POST"))
        .and(path("/rest/v1/leaderboard"))
        .and(header("Content-Type", "application/json"))
        .and(header("Prefer", "return=representation"))
        .respond_with(ResponseTemplate::new(201).set_body_json(mock_response))
        .mount(&mock_server)
        .await;

    let config = SupabaseConfig::new(mock_server.uri(), "test-key");
    let result = submit_score(&config, None, "NewPlayer", 50000).await;

    assert!(result.is_ok());
    let response = result.unwrap();
    assert_eq!(response.player_name, "NewPlayer");
    assert_eq!(response.score, 50000);
    assert_eq!(response.id, "00000000-0000-0000-0000-000000000123");
}

/// Test updating an existing score with a higher score
#[tokio::test]
async fn test_update_existing_score_higher() {
    let mock_server = MockServer::start().await;
    let player_id = "00000000-0000-0000-0000-000000000456";

    // Mock GET request to fetch existing record
    let existing_record = serde_json::json!([{
        "id": player_id,
        "player_name": "ExistingPlayer",
        "score": 40000,
        "submitted_at": null,
        "updated_at": null
    }]);

    Mock::given(method("GET"))
        .and(path("/rest/v1/leaderboard"))
        .respond_with(ResponseTemplate::new(200).set_body_json(existing_record))
        .mount(&mock_server)
        .await;

    // Mock PATCH request to update record
    let updated_record = serde_json::json!([{
        "id": player_id,
        "player_name": "ExistingPlayer",
        "score": 60000,
        "submitted_at": null,
        "updated_at": null
    }]);

    Mock::given(method("PATCH"))
        .and(path("/rest/v1/leaderboard"))
        .respond_with(ResponseTemplate::new(200).set_body_json(updated_record))
        .mount(&mock_server)
        .await;

    let config = SupabaseConfig::new(mock_server.uri(), "test-key");
    let result = submit_score(&config, Some(player_id), "ExistingPlayer", 60000).await;

    assert!(result.is_ok());
    let response = result.unwrap();
    assert_eq!(response.score, 60000);
}

/// Test updating an existing score with a lower score (should return existing)
#[tokio::test]
async fn test_update_existing_score_lower() {
    let mock_server = MockServer::start().await;
    let player_id = "00000000-0000-0000-0000-000000000789";

    // Mock GET request to fetch existing record with higher score
    let existing_record = serde_json::json!([{
        "id": player_id,
        "player_name": "HighScorer",
        "score": 100000,
        "submitted_at": null,
        "updated_at": null
    }]);

    Mock::given(method("GET"))
        .and(path("/rest/v1/leaderboard"))
        .respond_with(ResponseTemplate::new(200).set_body_json(existing_record))
        .mount(&mock_server)
        .await;

    // PATCH should NOT be called since new score is lower

    let config = SupabaseConfig::new(mock_server.uri(), "test-key");
    let result = submit_score(&config, Some(player_id), "HighScorer", 80000).await;

    assert!(result.is_ok());
    let response = result.unwrap();
    // Should return the existing higher score, not the new lower score
    assert_eq!(response.score, 100000);
}

/// Test handling network errors gracefully
#[tokio::test]
async fn test_network_error_handling() {
    let mock_server = MockServer::start().await;

    // Mock server returns 500 error
    Mock::given(method("GET"))
        .and(path("/rest/v1/leaderboard"))
        .respond_with(ResponseTemplate::new(500).set_body_string("Internal Server Error"))
        .mount(&mock_server)
        .await;

    let config = SupabaseConfig::new(mock_server.uri(), "test-key");
    let result = fetch_leaderboard(&config, 10).await;

    assert!(result.is_err());
    match result {
        Err(e) => {
            assert!(e.to_string().contains("Server error 500"));
        }
        Ok(_) => panic!("Expected error but got success"),
    }
}

/// Test handling invalid JSON response
#[tokio::test]
async fn test_invalid_json_response() {
    let mock_server = MockServer::start().await;

    // Mock server returns invalid JSON
    Mock::given(method("GET"))
        .and(path("/rest/v1/leaderboard"))
        .respond_with(ResponseTemplate::new(200).set_body_string("not valid json"))
        .mount(&mock_server)
        .await;

    let config = SupabaseConfig::new(mock_server.uri(), "test-key");
    let result = fetch_leaderboard(&config, 10).await;

    assert!(result.is_err());
    match result {
        Err(e) => {
            assert!(e.to_string().contains("Invalid response"));
        }
        Ok(_) => panic!("Expected error but got success"),
    }
}

/// Test score submission with RLS error
#[tokio::test]
async fn test_submit_score_rls_error() {
    let mock_server = MockServer::start().await;

    // Mock RLS policy violation (the actual error from Supabase)
    let rls_error = serde_json::json!({
        "code": "42501",
        "details": null,
        "hint": null,
        "message": "new row violates row-level security policy for table \"leaderboard\""
    });

    Mock::given(method("POST"))
        .and(path("/rest/v1/leaderboard"))
        .respond_with(ResponseTemplate::new(403).set_body_json(rls_error))
        .mount(&mock_server)
        .await;

    let config = SupabaseConfig::new(mock_server.uri(), "test-key");
    let result = submit_score(&config, None, "TestPlayer", 12345).await;

    assert!(result.is_err());
    match result {
        Err(e) => {
            assert!(e.to_string().contains("Server error 403"));
        }
        Ok(_) => panic!("Expected RLS error but got success"),
    }
}

/// Test that submit_score includes correct headers
#[tokio::test]
async fn test_submit_score_headers() {
    let mock_server = MockServer::start().await;

    let mock_response = serde_json::json!([{
        "id": "test-id",
        "player_name": "Player",
        "score": 1000,
        "submitted_at": null,
        "updated_at": null
    }]);

    // Verify that the request includes the required headers
    Mock::given(method("POST"))
        .and(path("/rest/v1/leaderboard"))
        .and(header("apikey", "test-anon-key"))
        .and(header("Authorization", "Bearer test-anon-key"))
        .and(header("Content-Type", "application/json"))
        .and(header("Prefer", "return=representation"))
        .respond_with(ResponseTemplate::new(201).set_body_json(mock_response))
        .expect(1) // Expect exactly one request
        .mount(&mock_server)
        .await;

    let config = SupabaseConfig::new(mock_server.uri(), "test-anon-key");
    let result = submit_score(&config, None, "Player", 1000).await;

    assert!(result.is_ok());
}

/// Test fetching leaderboard respects limit parameter
#[tokio::test]
async fn test_fetch_leaderboard_with_limit() {
    let mock_server = MockServer::start().await;

    // Create 50 mock entries
    let entries: Vec<_> = (0..50)
        .map(|i| {
            serde_json::json!({
                "id": format!("id-{}", i),
                "player_name": format!("Player{}", i),
                "score": 100000 - (i * 1000),
                "submitted_at": null,
                "updated_at": null
            })
        })
        .collect();

    Mock::given(method("GET"))
        .and(path("/rest/v1/leaderboard"))
        .respond_with(ResponseTemplate::new(200).set_body_json(entries))
        .mount(&mock_server)
        .await;

    let config = SupabaseConfig::new(mock_server.uri(), "test-key");
    let result = fetch_leaderboard(&config, 100).await;

    assert!(result.is_ok());
    let fetched = result.unwrap();
    assert_eq!(fetched.len(), 50);
    // Verify scores are in order
    assert!(fetched[0].score > fetched[1].score);
}
