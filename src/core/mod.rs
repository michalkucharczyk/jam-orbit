//! Platform-agnostic core module - shared between WASM dashboard and CLI

pub mod data;
pub mod events;
pub mod parser;

pub use data::{BestBlockData, EventStore, TimeSeriesData, GuaranteeQueueData, SyncStatusData, ShardMetrics};
#[allow(unused_imports)]
pub use events::{Event, EVENT_CATEGORIES, event_color_rgb, event_name};
pub use parser::{parse_event, ParserContext};
