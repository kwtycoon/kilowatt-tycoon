//! Leaderboard UI systems

use bevy::prelude::*;
use bevy::tasks::Task;

use crate::api::{SupabaseConfig, fetch_leaderboard};
use crate::resources::{LeaderboardData, LeaderboardEntry, PlayerProfile};
use crate::ui::LeaderboardModalState;

/// Marker component for leaderboard fetch tasks
#[derive(Component)]
pub struct LeaderboardFetchTask(Task<Result<Vec<LeaderboardEntry>, String>>);

/// Marker component for score submission tasks
#[derive(Component)]
pub struct ScoreSubmitTask(pub Task<Option<String>>);

/// Spawn an async task on the appropriate runtime.
///
/// On **native**, this creates a real Tokio runtime in a background thread
/// (required by reqwest/hyper for DNS resolution). The result is bridged
/// back into a Bevy `Task` so the existing poll systems work unchanged.
///
/// On **WASM**, this uses Bevy's `AsyncComputeTaskPool` directly.
/// WASM futures are `!Send` (they use `Rc`/`JsFuture`), so the WASM
/// variant drops the `Send` bound.
#[cfg(not(target_arch = "wasm32"))]
pub fn spawn_network_task<F, T>(future: F) -> Task<T>
where
    F: std::future::Future<Output = T> + Send + 'static,
    T: Send + 'static,
{
    // Channel to receive the result from the Tokio thread
    let (tx, rx) = std::sync::mpsc::channel();

    std::thread::spawn(move || {
        let rt = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .expect("Failed to create Tokio runtime for network task");
        let result = rt.block_on(future);
        let _ = tx.send(result);
    });

    // Wrap in a Bevy Task that yields until the result is ready
    bevy::tasks::AsyncComputeTaskPool::get().spawn(async move {
        std::future::poll_fn(|cx| {
            match rx.try_recv() {
                Ok(result) => std::task::Poll::Ready(result),
                Err(std::sync::mpsc::TryRecvError::Empty) => {
                    // Schedule a wake-up so we get polled again
                    cx.waker().wake_by_ref();
                    std::task::Poll::Pending
                }
                Err(std::sync::mpsc::TryRecvError::Disconnected) => {
                    panic!("Network task thread panicked or disconnected");
                }
            }
        })
        .await
    })
}

/// WASM variant — no `Send` bound required (WASM is single-threaded).
#[cfg(target_arch = "wasm32")]
pub fn spawn_network_task<F, T>(future: F) -> Task<T>
where
    F: std::future::Future<Output = T> + 'static,
    T: 'static,
{
    bevy::tasks::AsyncComputeTaskPool::get().spawn(future)
}

/// System to trigger leaderboard fetch when opening leaderboard modal
pub fn fetch_leaderboard_on_modal_open(
    mut commands: Commands,
    modal_state: Res<LeaderboardModalState>,
    mut leaderboard_data: ResMut<LeaderboardData>,
    supabase_config: Option<Res<SupabaseConfig>>,
    existing_tasks: Query<Entity, With<LeaderboardFetchTask>>,
    time: Res<Time>,
) {
    // Only fetch when leaderboard modal is open
    if !modal_state.is_open {
        return;
    }

    // Don't fetch if already loading or a task is running
    if leaderboard_data.is_loading || !existing_tasks.is_empty() {
        return;
    }

    let now = time.elapsed_secs_f64();

    // Check if we have Supabase configured
    let Some(config) = supabase_config else {
        if leaderboard_data.error.is_none() {
            leaderboard_data.set_error("Leaderboard service not configured", now);
        }
        return;
    };

    // Check if we can retry (respects exponential backoff after failures)
    if !leaderboard_data.can_retry(now) {
        return;
    }

    // Check if we should refresh (never fetched or stale after 30s)
    if !leaderboard_data.should_fetch(now, 30.0) {
        return;
    }

    // Mark as loading
    leaderboard_data.start_loading();

    // Spawn async task to fetch leaderboard (on a real Tokio runtime for native)
    let config_clone = config.clone();
    let task = spawn_network_task(async move {
        fetch_leaderboard(&config_clone, 10)
            .await
            .map_err(|e| e.to_string())
    });

    commands.spawn(LeaderboardFetchTask(task));
}

/// System to poll leaderboard fetch tasks and update data when complete
pub fn poll_leaderboard_fetch_tasks(
    mut commands: Commands,
    mut leaderboard_data: ResMut<LeaderboardData>,
    mut tasks: Query<(Entity, &mut LeaderboardFetchTask)>,
    time: Res<Time>,
) {
    let now = time.elapsed_secs_f64();

    for (entity, mut task) in &mut tasks {
        if let Some(result) = poll_task(&mut task.0) {
            match result {
                Ok(entries) => {
                    info!("Leaderboard fetched: {} entries", entries.len());
                    leaderboard_data.update_entries(entries, now);
                }
                Err(e) => {
                    error!("Failed to fetch leaderboard: {}", e);
                    leaderboard_data.set_error(format!("Failed to load: {}", e), now);
                }
            }
            commands.entity(entity).despawn();
        }
    }
}

/// System to poll score submission tasks and update player profile
pub fn poll_score_submit_tasks(
    mut commands: Commands,
    mut player_profile: ResMut<PlayerProfile>,
    mut leaderboard_data: ResMut<LeaderboardData>,
    mut tasks: Query<(Entity, &mut ScoreSubmitTask)>,
) {
    for (entity, mut task) in &mut tasks {
        if let Some(player_id) = poll_task(&mut task.0) {
            if let Some(id) = player_id {
                player_profile.player_id = Some(id);
                info!("Score submitted, player ID: {:?}", player_profile.player_id);

                // Trigger a refresh of the leaderboard
                leaderboard_data.last_fetched_at_secs = None;
            }
            commands.entity(entity).despawn();
        }
    }
}

/// Poll a Bevy task without blocking. Returns `Some(value)` when ready, `None` while pending.
fn poll_task<T>(task: &mut Task<T>) -> Option<T> {
    use std::future::Future;
    use std::pin::Pin;
    use std::task::{Context, Poll, Waker};

    struct NoopWaker;

    impl std::task::Wake for NoopWaker {
        fn wake(self: std::sync::Arc<Self>) {}
    }

    let waker = Waker::from(std::sync::Arc::new(NoopWaker));
    let mut context = Context::from_waker(&waker);

    match Pin::new(task).poll(&mut context) {
        Poll::Ready(result) => Some(result),
        Poll::Pending => None,
    }
}
