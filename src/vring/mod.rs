//! Validators Ring (vring) visualization module
//!
//! Renders directed events as particles traveling between validators
//! arranged on a circle.

mod data;
#[cfg(not(target_arch = "wasm32"))]
mod renderer;

pub use data::{DirectedEventBuffer, DirectedParticleInstance, PeerRegistry};

#[cfg(not(target_arch = "wasm32"))]
pub use renderer::{FilterBitfield, GpuParticle, RingCallback, RingRenderer, Uniforms};
