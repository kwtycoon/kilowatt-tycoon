//! OCPP WebSocket connection manager.
//!
//! Manages per-charger WebSocket connections to a Central System (CSMS).
//!
//! - **Native**: Uses `tokio-tungstenite` in a background thread with its own
//!   Tokio runtime. Messages are sent via `std::sync::mpsc` channels.
//! - **WASM**: Uses the `web_sys::WebSocket` API with a polling Bevy system.

use std::collections::HashMap;

use bevy::prelude::*;

use super::queue::OcppMessageQueue;

// ─────────────────────────────────────────────────────
//  Connection status (shared across platforms)
// ─────────────────────────────────────────────────────

/// Connection status for a single charger's WebSocket.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WsStatus {
    /// Not connected (never started or intentionally closed).
    Disconnected,
    /// Attempting to connect.
    Connecting,
    /// Connected and ready to send.
    Connected,
    /// Connection failed (will retry).
    Error,
}

// ─────────────────────────────────────────────────────
//  OcppConnectionManager (Resource)
// ─────────────────────────────────────────────────────

/// Manages all active WebSocket connections.
///
/// Each charger gets its own connection to `{endpoint_url}/{charger_id}`.
#[derive(Resource)]
pub struct OcppConnectionManager {
    /// Per-charger connection handles.
    pub connections: HashMap<String, ChargerConnection>,

    /// Last endpoint URL we used (to detect config changes).
    pub last_endpoint_url: String,

    /// Real-time timer for batch flushing (seconds).
    pub flush_timer: f32,
}

impl Default for OcppConnectionManager {
    fn default() -> Self {
        Self {
            connections: HashMap::new(),
            last_endpoint_url: String::new(),
            flush_timer: 0.0,
        }
    }
}

/// Connection state for a single charger.
pub struct ChargerConnection {
    pub status: WsStatus,
    /// Messages waiting to be sent over this connection.
    pub pending: Vec<String>,
    /// Platform-specific handle.
    pub handle: ConnectionHandle,
}

// ─────────────────────────────────────────────────────
//  Native connection handle (tokio-tungstenite)
// ─────────────────────────────────────────────────────

#[cfg(not(target_arch = "wasm32"))]
pub struct ConnectionHandle {
    /// Channel to send messages to the background WebSocket thread.
    pub sender: Option<std::sync::mpsc::Sender<String>>,
    /// Channel to receive status updates from the background thread.
    /// Wrapped in `Mutex` so the overall struct is `Sync` (required for Bevy `Resource`).
    pub status_rx: Option<std::sync::Mutex<std::sync::mpsc::Receiver<WsStatus>>>,
}

#[cfg(not(target_arch = "wasm32"))]
impl Default for ConnectionHandle {
    #[allow(clippy::derivable_impls)]
    fn default() -> Self {
        Self {
            sender: None,
            status_rx: None,
        }
    }
}

#[cfg(not(target_arch = "wasm32"))]
impl ConnectionHandle {
    /// Spawn a background thread that connects to the given WebSocket URL
    /// and sends all messages received on the channel.
    pub fn connect(url: String) -> Self {
        let (msg_tx, msg_rx) = std::sync::mpsc::channel::<String>();
        let (status_tx, status_rx) = std::sync::mpsc::channel::<WsStatus>();

        std::thread::spawn(move || {
            let rt = match tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
            {
                Ok(rt) => rt,
                Err(e) => {
                    bevy::log::error!("OCPP: Failed to create Tokio runtime: {}", e);
                    let _ = status_tx.send(WsStatus::Error);
                    return;
                }
            };

            rt.block_on(async move {
                use tokio_tungstenite::tungstenite::client::IntoClientRequest;

                // Build request with OCPP subprotocol
                let mut request = match url.as_str().into_client_request() {
                    Ok(r) => r,
                    Err(e) => {
                        bevy::log::error!("OCPP: Invalid WebSocket URL '{}': {}", url, e);
                        let _ = status_tx.send(WsStatus::Error);
                        return;
                    }
                };
                request
                    .headers_mut()
                    .insert("Sec-WebSocket-Protocol", "ocpp1.6".parse().unwrap());

                let _ = status_tx.send(WsStatus::Connecting);

                let ws_stream = match tokio_tungstenite::connect_async(request).await {
                    Ok((stream, _response)) => {
                        bevy::log::info!("OCPP: Connected to {}", url);
                        let _ = status_tx.send(WsStatus::Connected);
                        stream
                    }
                    Err(e) => {
                        bevy::log::error!("OCPP: Connection failed to {}: {}", url, e);
                        let _ = status_tx.send(WsStatus::Error);
                        return;
                    }
                };

                use futures_util::{SinkExt, StreamExt};

                let (mut ws_sender, mut ws_receiver) = ws_stream.split();

                // Spawn a task to read (and discard) incoming messages / detect close
                let status_tx_read = status_tx.clone();
                tokio::spawn(async move {
                    while let Some(msg) = ws_receiver.next().await {
                        match msg {
                            Ok(_) => {} // Ignore CallResult/CallError for now
                            Err(e) => {
                                bevy::log::warn!("OCPP: WebSocket read error: {}", e);
                                let _ = status_tx_read.send(WsStatus::Error);
                                return;
                            }
                        }
                    }
                    let _ = status_tx_read.send(WsStatus::Disconnected);
                });

                // Main send loop: pull from mpsc channel and write to WebSocket
                loop {
                    match msg_rx.recv() {
                        Ok(json) => {
                            use tokio_tungstenite::tungstenite::Message;
                            if let Err(e) = ws_sender.send(Message::Text(json.into())).await {
                                bevy::log::warn!("OCPP: WebSocket send error: {}", e);
                                let _ = status_tx.send(WsStatus::Error);
                                return;
                            }
                        }
                        Err(_) => {
                            // Channel closed, connection manager dropped
                            bevy::log::info!(
                                "OCPP: Channel closed, shutting down connection to {}",
                                url
                            );
                            return;
                        }
                    }
                }
            });
        });

        Self {
            sender: Some(msg_tx),
            status_rx: Some(std::sync::Mutex::new(status_rx)),
        }
    }
}

// ─────────────────────────────────────────────────────
//  WASM connection handle (web_sys::WebSocket)
// ─────────────────────────────────────────────────────

#[cfg(target_arch = "wasm32")]
pub struct ConnectionHandle {
    pub ws: Option<web_sys::WebSocket>,
    pub status: WsStatus,
}

// SAFETY: WASM is single-threaded; web_sys::WebSocket is !Send+!Sync but
// Bevy Resource requires both. This is safe because there is only one thread.
#[cfg(target_arch = "wasm32")]
unsafe impl Send for ConnectionHandle {}
#[cfg(target_arch = "wasm32")]
unsafe impl Sync for ConnectionHandle {}

#[cfg(target_arch = "wasm32")]
impl Default for ConnectionHandle {
    fn default() -> Self {
        Self {
            ws: None,
            status: WsStatus::Disconnected,
        }
    }
}

#[cfg(target_arch = "wasm32")]
impl ConnectionHandle {
    pub fn connect(url: String) -> Self {
        use wasm_bindgen::prelude::*;

        let ws = match web_sys::WebSocket::new_with_str(&url, "ocpp1.6") {
            Ok(ws) => ws,
            Err(e) => {
                bevy::log::error!("OCPP: Failed to create WebSocket: {:?}", e);
                return Self {
                    ws: None,
                    status: WsStatus::Error,
                };
            }
        };

        // Set binary type
        ws.set_binary_type(web_sys::BinaryType::Arraybuffer);

        // Open callback
        let on_open = Closure::<dyn FnMut()>::new(move || {
            bevy::log::info!("OCPP: WebSocket connected (WASM)");
        });
        ws.set_onopen(Some(on_open.as_ref().unchecked_ref()));
        on_open.forget();

        // Error callback
        let on_error = Closure::<dyn FnMut()>::new(move || {
            bevy::log::warn!("OCPP: WebSocket error (WASM)");
        });
        ws.set_onerror(Some(on_error.as_ref().unchecked_ref()));
        on_error.forget();

        Self {
            ws: Some(ws),
            status: WsStatus::Connecting,
        }
    }
}

// ─────────────────────────────────────────────────────
//  Bevy system: ocpp_send_system
// ─────────────────────────────────────────────────────

/// Batch flush interval in real seconds.
const FLUSH_INTERVAL_SECS: f32 = 0.1;

/// Drain the [`OcppMessageQueue`] and send messages over their respective
/// WebSocket connections. Creates new connections on-demand.
pub fn ocpp_send_system(
    mut queue: ResMut<OcppMessageQueue>,
    mut conn_mgr: ResMut<OcppConnectionManager>,
    time: Res<Time>,
) {
    if !queue.enabled || queue.endpoint_url.is_empty() {
        return;
    }

    // Batch-flush on a real-time interval to avoid overwhelming the wire
    conn_mgr.flush_timer += time.delta_secs();
    if conn_mgr.flush_timer < FLUSH_INTERVAL_SECS {
        return;
    }
    conn_mgr.flush_timer = 0.0;

    // If the endpoint URL changed, close all connections
    if conn_mgr.last_endpoint_url != queue.endpoint_url {
        conn_mgr.connections.clear();
        conn_mgr.last_endpoint_url = queue.endpoint_url.clone();
    }

    // Drain messages from queue
    let messages = queue.drain_all();
    if messages.is_empty() {
        return;
    }

    // Group messages by charger_id
    let mut grouped: HashMap<String, Vec<String>> = HashMap::new();
    for (charger_id, json) in messages {
        grouped.entry(charger_id).or_default().push(json);
    }

    let base_url = queue.endpoint_url.trim_end_matches('/').to_string();

    for (charger_id, msgs) in grouped {
        // Ensure connection exists
        let conn = conn_mgr
            .connections
            .entry(charger_id.clone())
            .or_insert_with(|| {
                let url = format!("{}/{}", base_url, charger_id);
                info!("OCPP: Opening WebSocket to {}", url);
                ChargerConnection {
                    status: WsStatus::Connecting,
                    pending: Vec::new(),
                    handle: ConnectionHandle::connect(url),
                }
            });

        // Update status from background thread (native only)
        #[cfg(not(target_arch = "wasm32"))]
        {
            if let Some(rx_mutex) = &conn.handle.status_rx
                && let Ok(rx) = rx_mutex.lock()
            {
                // Drain all status updates, keep the latest
                while let Ok(status) = rx.try_recv() {
                    conn.status = status;
                }
            }
        }

        // On WASM, check WebSocket readyState
        #[cfg(target_arch = "wasm32")]
        {
            if let Some(ws) = &conn.handle.ws {
                conn.status = match ws.ready_state() {
                    0 => WsStatus::Connecting,
                    1 => WsStatus::Connected,
                    2 | 3 => WsStatus::Disconnected,
                    _ => WsStatus::Error,
                };
                conn.handle.status = conn.status;
            }
        }

        // Add new messages to pending
        conn.pending.extend(msgs);

        // Send if connected
        if conn.status == WsStatus::Connected {
            send_pending(conn);
        }
    }
}

/// Send all pending messages for a connection.
fn send_pending(conn: &mut ChargerConnection) {
    let pending = std::mem::take(&mut conn.pending);

    #[cfg(not(target_arch = "wasm32"))]
    {
        if let Some(sender) = &conn.handle.sender {
            for json in pending {
                if sender.send(json).is_err() {
                    conn.status = WsStatus::Error;
                    break;
                }
            }
        }
    }

    #[cfg(target_arch = "wasm32")]
    {
        if let Some(ws) = &conn.handle.ws {
            for json in pending {
                if let Err(e) = ws.send_with_str(&json) {
                    bevy::log::warn!("OCPP: WebSocket send error: {:?}", e);
                    conn.status = WsStatus::Error;
                    break;
                }
            }
        }
    }
}
