//! GPU scatter plot renderer for Event Particles visualization
//!
//! Renders events as colored dots on a scatter plot (X=node, Y=age)
//! using GPU instancing with an off-screen texture.

mod renderer;

pub use renderer::{ScatterCallback, ScatterParticle, ScatterRenderer, ScatterUniforms};
