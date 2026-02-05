//! Shared WebSocket connection state
//!
//! Used by both WASM and native WebSocket clients.

/// WebSocket connection state
#[derive(Clone, Debug)]
#[allow(dead_code)]
pub enum WsState {
    Connecting,
    Connected,
    Disconnected,
    Error(String),
}

impl WsState {
    #[allow(dead_code)]
    pub fn is_connected(&self) -> bool {
        matches!(self, WsState::Connected)
    }
}
