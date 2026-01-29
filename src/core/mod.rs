//! Platform-agnostic core module - shared between WASM dashboard and CLI

pub mod data;
pub mod parser;

pub use data::{BestBlockData, TimeSeriesData};
pub use parser::parse_event;
