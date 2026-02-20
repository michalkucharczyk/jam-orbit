//! Native WebSocket client for connecting to jamtart
//!
//! Uses tokio-tungstenite in a background thread, with channel-based message passing.

use crate::ws_state::WsState;
use std::sync::mpsc::{self, Receiver, Sender};
use std::sync::Arc;
use parking_lot::Mutex;
use tracing::{error, info, warn};

/// Native WebSocket client that runs in a background thread
pub struct NativeWsClient {
    /// Receiver for incoming messages
    pub rx: Receiver<String>,
    /// Shared connection state
    pub state: Arc<Mutex<WsState>>,
}

impl NativeWsClient {
    /// Connect to a WebSocket endpoint
    ///
    /// Spawns a background thread with a tokio runtime to handle the connection.
    /// Messages are sent through the returned receiver.
    pub fn connect(url: &str) -> Self {
        let (tx, rx): (Sender<String>, Receiver<String>) = mpsc::channel();
        let state = Arc::new(Mutex::new(WsState::Connecting));

        let url = url.to_string();
        let state_clone = state.clone();

        std::thread::spawn(move || {
            let rt = match tokio::runtime::Runtime::new() {
                Ok(rt) => rt,
                Err(e) => {
                    error!(error = %e, "Failed to create tokio runtime");
                    *state_clone.lock() = WsState::Error(e.to_string());
                    return;
                }
            };
            rt.block_on(async move {
                Self::run_websocket(&url, tx, state_clone).await;
            });
        });

        Self { rx, state }
    }

    async fn run_websocket(url: &str, tx: Sender<String>, state: Arc<Mutex<WsState>>) {
        use futures_util::{SinkExt, StreamExt};
        use tokio_tungstenite::{connect_async, tungstenite::Message};

        info!(url, "Connecting to WebSocket");

        let ws_stream = match connect_async(url).await {
            Ok((stream, _)) => {
                info!("WebSocket connected");
                *state.lock() = WsState::Connected;
                stream
            }
            Err(e) => {
                error!(error = %e, "Failed to connect");
                *state.lock() = WsState::Error(e.to_string());
                return;
            }
        };

        let (mut write, mut read) = ws_stream.split();

        // Subscribe to all events
        let subscribe = r#"{"type":"Subscribe","filter":{"type":"All"}}"#;
        if let Err(e) = write.send(Message::Text(subscribe.into())).await {
            error!(error = %e, "Failed to send subscribe message");
            *state.lock() = WsState::Error(e.to_string());
            return;
        }

        // Read messages and send through channel
        while let Some(msg) = read.next().await {
            match msg {
                Ok(Message::Text(text)) => {
                    if tx.send(text.to_string()).is_err() {
                        // Receiver dropped, exit
                        break;
                    }
                }
                Ok(Message::Close(_)) => {
                    warn!("WebSocket closed by server");
                    *state.lock() = WsState::Disconnected;
                    break;
                }
                Err(e) => {
                    error!(error = %e, "WebSocket error");
                    *state.lock() = WsState::Error(e.to_string());
                    break;
                }
                _ => {}
            }
        }

        warn!("WebSocket stream ended");
        *state.lock() = WsState::Disconnected;
    }
}
