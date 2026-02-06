//! Validators Ring (vring) visualization module
//!
//! Renders directed events as particles traveling between validators
//! arranged on a circle.

mod data;
#[cfg(not(target_arch = "wasm32"))]
mod renderer;

pub use data::{DirectedEventBuffer, DirectedParticleInstance, PulseEvent};

#[cfg(not(target_arch = "wasm32"))]
pub use renderer::{ColorLut, FilterBitfield, GpuParticle, RingCallback, RingRenderer, Uniforms};
