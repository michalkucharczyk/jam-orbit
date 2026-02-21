//! Validators Ring (vring) visualization module
//!
//! Renders directed events as particles traveling between validators
//! arranged on a circle.

mod data;
mod renderer;

pub use data::{DirectedEventBuffer, DirectedParticleInstance, PulseEvent};

pub use renderer::{ColorLut, ColorSchema, FilterBitfield, GpuParticle, RingCallback, RingRenderer, Uniforms};

#[allow(unused_imports)]
pub use renderer::CATEGORY_COLORS;
