//! WASM WebSocket client for connecting to jamtart

use crate::ws_state::WsState;
use std::cell::RefCell;
use std::collections::VecDeque;
use std::rc::Rc;
use tracing::{debug, error, info, warn};
use wasm_bindgen::prelude::*;
use wasm_bindgen::JsCast;
use web_sys::{CloseEvent, ErrorEvent, MessageEvent, WebSocket};

/// Shared message buffer — WS callback pushes, app drains in update()
pub type MessageBuffer = Rc<RefCell<VecDeque<String>>>;

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
    /// Messages are buffered into `msg_buffer` for the app to drain with a time budget.
    pub fn connect(
        url: &str,
        msg_buffer: MessageBuffer,
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

        // On message - push to buffer (processed in app update())
        let on_msg = Closure::wrap(Box::new(move |e: MessageEvent| {
            if let Ok(txt) = e.data().dyn_into::<js_sys::JsString>() {
                let msg: String = txt.into();
                msg_buffer.borrow_mut().push_back(msg);
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
