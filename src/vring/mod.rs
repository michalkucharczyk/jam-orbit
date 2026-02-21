//! Validators Ring (vring) visualization module
//!
//! Renders directed events as particles traveling between validators
//! arranged on a circle.

mod data;
mod renderer;

pub use data::{DirectedEventBuffer, DirectedParticleInstance, PulseEvent};

pub use renderer::{ColorLut, FilterBitfield, GpuParticle, RingCallback, RingRenderer, Uniforms, CATEGORY_COLORS};
