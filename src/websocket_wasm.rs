//! WASM WebSocket client for connecting to jamtart
//!
//! This module is only available when the `wasm` feature is enabled.

use std::cell::RefCell;
use std::rc::Rc;
use tracing::{debug, error, info, trace, warn};
use wasm_bindgen::prelude::*;
use wasm_bindgen::JsCast;
use web_sys::{CloseEvent, ErrorEvent, MessageEvent, WebSocket};

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

    #[allow(dead_code)]
    pub fn display(&self) -> &'static str {
        match self {
            WsState::Connecting => "Connecting...",
            WsState::Connected => "Connected",
            WsState::Disconnected => "Disconnected",
            WsState::Error(_) => "Error",
        }
    }
}

/// WASM WebSocket client
pub struct WsClient {
    #[allow(dead_code)]
    ws: WebSocket,
    #[allow(dead_code)]
    state: Rc<RefCell<WsState>>,
}

impl WsClient {
    /// Connect to a WebSocket endpoint
    ///
    /// # Arguments
    /// * `url` - WebSocket URL (e.g., "ws://127.0.0.1:8080/api/ws")
    /// * `on_message` - Callback invoked for each message received
    /// * `state` - Shared state that will be updated on connection events
    pub fn connect(
        url: &str,
        on_message: impl Fn(String) + 'static,
        state: Rc<RefCell<WsState>>,
    ) -> Result<Self, JsValue> {
        info!(url, "Connecting to WebSocket");

        let ws = WebSocket::new(url)?;

        // On open - update state and send subscribe message
        let ws_clone = ws.clone();
        let state_clone = state.clone();
        let on_open = Closure::wrap(Box::new(move |_| {
            info!("WebSocket connected");
            *state_clone.borrow_mut() = WsState::Connected;

            // Subscribe to all events
            let subscribe = r#"{"type":"Subscribe","filter":{"type":"All"}}"#;
            debug!(subscribe, "Sending subscribe message");
            if let Err(e) = ws_clone.send_with_str(subscribe) {
                error!(?e, "Failed to send subscribe message");
            }
        }) as Box<dyn Fn(JsValue)>);
        ws.set_onopen(Some(on_open.as_ref().unchecked_ref()));
        on_open.forget();

        // On message - invoke callback
        let on_msg = Closure::wrap(Box::new(move |e: MessageEvent| {
            if let Ok(txt) = e.data().dyn_into::<js_sys::JsString>() {
                let msg: String = txt.into();
                trace!(len = msg.len(), "WebSocket message received");
                on_message(msg);
            }
        }) as Box<dyn Fn(MessageEvent)>);
        ws.set_onmessage(Some(on_msg.as_ref().unchecked_ref()));
        on_msg.forget();

        // On error
        let state_clone = state.clone();
        let on_err = Closure::wrap(Box::new(move |e: ErrorEvent| {
            let msg = e.message();
            error!(error = %msg, "WebSocket error");
            *state_clone.borrow_mut() = WsState::Error(msg);
        }) as Box<dyn Fn(ErrorEvent)>);
        ws.set_onerror(Some(on_err.as_ref().unchecked_ref()));
        on_err.forget();

        // On close
        let state_clone = state.clone();
        let on_close = Closure::wrap(Box::new(move |e: CloseEvent| {
            let code = e.code();
            let reason = e.reason();
            warn!(code, reason = %reason, "WebSocket closed");
            *state_clone.borrow_mut() = WsState::Disconnected;
        }) as Box<dyn Fn(CloseEvent)>);
        ws.set_onclose(Some(on_close.as_ref().unchecked_ref()));
        on_close.forget();

        Ok(Self { ws, state })
    }

    /// Get the current connection state
    #[allow(dead_code)]
    pub fn state(&self) -> WsState {
        self.state.borrow().clone()
    }
}
