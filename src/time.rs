//! Platform-agnostic time utilities
//!
//! Provides a unified way to get elapsed time in seconds since app start.

#[cfg(target_arch = "wasm32")]
pub fn now_seconds() -> f64 {
    web_sys::window()
        .and_then(|w| w.performance())
        .map(|p| p.now() / 1000.0)
        .unwrap_or(0.0)
}

#[cfg(not(target_arch = "wasm32"))]
pub fn now_seconds() -> f64 {
    use std::sync::OnceLock;
    use std::time::Instant;

    static START: OnceLock<Instant> = OnceLock::new();
    START.get_or_init(Instant::now).elapsed().as_secs_f64()
}
