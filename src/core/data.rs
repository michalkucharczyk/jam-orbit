//! Data structures for storing telemetry from jamtart
//!
//! These structures are platform-agnostic (no WASM deps) and shared
//! between the CLI and dashboard.

use std::collections::{HashMap, VecDeque};
use tracing::{debug, trace};

use super::events::Event;

/// Time series data - stores num_peers over time per validator
pub struct TimeSeriesData {
    /// [validator_idx][time_idx] = value
    pub series: Vec<Vec<f32>>,
    /// Maximum points to keep per series (ring buffer)
    pub max_points: usize,
    /// Maps node_id (hex string) to array index
    node_index: HashMap<String, usize>,
}

impl TimeSeriesData {
    pub fn new(num_series: usize, max_points: usize) -> Self {
        Self {
            series: vec![Vec::with_capacity(max_points); num_series],
            max_points,
            node_index: HashMap::new(),
        }
    }

    /// Push a new data point for a validator
    pub fn push(&mut self, node_id: &str, value: f32) {
        let (idx, is_new) = self.get_or_create_index(node_id);

        if is_new {
            debug!(node_id, idx, "New validator registered for time series");
        }

        let series = &mut self.series[idx];
        if series.len() >= self.max_points {
            series.remove(0);
        }
        series.push(value);

        trace!(node_id, idx, value, series_len = series.len(), "Time series data point");
    }

    /// Get or create an index for a node_id
    fn get_or_create_index(&mut self, node_id: &str) -> (usize, bool) {
        if let Some(&idx) = self.node_index.get(node_id) {
            return (idx, false);
        }

        // Assign next available index, capped at series capacity
        let idx = self.node_index.len().min(self.series.len() - 1);
        self.node_index.insert(node_id.to_string(), idx);
        (idx, true)
    }

    /// Number of unique validators seen
    pub fn validator_count(&self) -> usize {
        self.node_index.len()
    }

    /// Length of the longest series
    #[allow(dead_code)]
    pub fn max_series_len(&self) -> usize {
        self.series.iter().map(|s| s.len()).max().unwrap_or(0)
    }

    /// Number of data points in the first non-empty series
    #[allow(dead_code)]
    pub fn point_count(&self) -> usize {
        self.series.first().map_or(0, |s| s.len())
    }
}

/// Best block and finalized block data per validator
pub struct BestBlockData {
    /// [validator_idx] = best block slot
    pub best_blocks: Vec<u64>,
    /// [validator_idx] = finalized block slot
    pub finalized_blocks: Vec<u64>,
    /// Maps node_id to array index
    node_index: HashMap<String, usize>,
}

impl BestBlockData {
    pub fn new(num_validators: usize) -> Self {
        Self {
            best_blocks: vec![0; num_validators],
            finalized_blocks: vec![0; num_validators],
            node_index: HashMap::new(),
        }
    }

    /// Update best block for a validator
    pub fn set_best(&mut self, node_id: &str, slot: u64) {
        let (idx, is_new) = self.get_or_create_index(node_id);

        if is_new {
            debug!(node_id, idx, "New validator registered for blocks");
        }

        let prev = self.best_blocks[idx];
        self.best_blocks[idx] = slot;

        trace!(node_id, prev_slot = prev, new_slot = slot, "Best block updated");
    }

    /// Update finalized block for a validator
    pub fn set_finalized(&mut self, node_id: &str, slot: u64) {
        let (idx, _) = self.get_or_create_index(node_id);
        let prev = self.finalized_blocks[idx];
        self.finalized_blocks[idx] = slot;

        trace!(node_id, prev_slot = prev, new_slot = slot, "Finalized block updated");
    }

    fn get_or_create_index(&mut self, node_id: &str) -> (usize, bool) {
        if let Some(&idx) = self.node_index.get(node_id) {
            return (idx, false);
        }

        let idx = self.node_index.len().min(self.best_blocks.len() - 1);
        self.node_index.insert(node_id.to_string(), idx);
        (idx, true)
    }

    /// Number of unique validators seen
    #[allow(dead_code)]
    pub fn validator_count(&self) -> usize {
        self.node_index.len()
    }

    /// Highest best block slot across all validators
    pub fn highest_slot(&self) -> Option<u64> {
        self.best_blocks.iter().copied().filter(|&s| s > 0).max()
    }

    /// Highest finalized slot across all validators
    #[allow(dead_code)]
    pub fn highest_finalized(&self) -> Option<u64> {
        self.finalized_blocks.iter().copied().filter(|&s| s > 0).max()
    }
}

// ============================================================================
// Event Storage (full events, indexed per-node)
// ============================================================================

/// A stored event with app-relative timestamp
#[allow(dead_code)]
pub struct StoredEvent {
    /// When event occurred (app-relative seconds)
    pub timestamp: f64,
    /// Full parsed event with all variant data
    pub event: Event,
}

impl StoredEvent {
    /// Get the event type discriminant
    #[allow(dead_code)]
    pub fn event_type(&self) -> u8 {
        self.event.event_type() as u8
    }
}

/// Events for a single node
pub struct NodeEvents {
    /// Ring buffer of events for this node
    pub events: VecDeque<StoredEvent>,
    /// Node index (for visualization X coordinate)
    pub index: u16,
}

impl NodeEvents {
    fn new(index: u16, capacity: usize) -> Self {
        Self {
            events: VecDeque::with_capacity(capacity),
            index,
        }
    }
}

/// Per-node event storage - source of truth for all event visualizations
pub struct EventStore {
    /// Events grouped by node: node_id → NodeEvents
    nodes: HashMap<String, NodeEvents>,
    /// Max events per node (ring buffer per node)
    max_events_per_node: usize,
    /// How long to keep events (seconds)
    #[allow(dead_code)]
    pub retention: f64,
    /// Counter for assigning node indices
    next_node_index: u16,
}

impl EventStore {
    pub fn new(max_events_per_node: usize, retention: f64) -> Self {
        Self {
            nodes: HashMap::new(),
            max_events_per_node,
            retention,
            next_node_index: 0,
        }
    }

    /// Store a new event for a node
    pub fn push(&mut self, node_id: &str, event: Event, timestamp: f64) {
        let max_events = self.max_events_per_node;
        let next_idx = &mut self.next_node_index;

        let node = self.nodes.entry(node_id.to_string()).or_insert_with(|| {
            let idx = *next_idx;
            *next_idx = next_idx.saturating_add(1);
            debug!(node_id, idx, "New node registered for events");
            NodeEvents::new(idx, max_events)
        });

        // Ring buffer: remove oldest if at capacity
        if node.events.len() >= max_events {
            node.events.pop_front();
        }

        node.events.push_back(StoredEvent { timestamp, event });

        trace!(
            node_id,
            events_count = node.events.len(),
            "Event stored"
        );
    }

    /// Get node index (for X position in visualizations)
    #[allow(dead_code)]
    pub fn node_index(&self, node_id: &str) -> Option<u16> {
        self.nodes.get(node_id).map(|n| n.index)
    }

    /// Iterate all nodes
    #[allow(dead_code)]
    pub fn nodes(&self) -> impl Iterator<Item = (&str, &NodeEvents)> {
        self.nodes.iter().map(|(k, v)| (k.as_str(), v))
    }

    /// Iterate events for a specific node
    #[allow(dead_code)]
    pub fn node_events(&self, node_id: &str) -> Option<&VecDeque<StoredEvent>> {
        self.nodes.get(node_id).map(|n| &n.events)
    }

    /// Total node count
    pub fn node_count(&self) -> usize {
        self.nodes.len()
    }

    /// Compute event rates per node for given event types
    ///
    /// Returns Vec<(node_idx, Vec<count_per_bucket>)>
    #[allow(dead_code)]
    pub fn compute_rates_per_node(
        &self,
        now: f64,
        bucket_duration: f64,
        num_buckets: usize,
        event_filter: &[bool],
    ) -> Vec<(u16, Vec<u32>)> {
        let oldest_time = now - (bucket_duration * num_buckets as f64);

        self.nodes
            .values()
            .map(|node| {
                let mut buckets = vec![0u32; num_buckets];

                for stored in &node.events {
                    // Skip events outside time window
                    if stored.timestamp < oldest_time {
                        continue;
                    }

                    // Skip events not in filter
                    let et = stored.event_type() as usize;
                    if et >= event_filter.len() || !event_filter[et] {
                        continue;
                    }

                    // Compute bucket index
                    let age = now - stored.timestamp;
                    let bucket_idx = ((age / bucket_duration) as usize).min(num_buckets - 1);
                    // Invert so newest is at the end
                    let bucket_idx = num_buckets - 1 - bucket_idx;
                    buckets[bucket_idx] += 1;
                }

                (node.index, buckets)
            })
            .collect()
    }

    /// Prune old events beyond retention period
    #[allow(dead_code)]
    pub fn prune(&mut self, now: f64) {
        let cutoff = now - self.retention;
        for node in self.nodes.values_mut() {
            while let Some(front) = node.events.front() {
                if front.timestamp < cutoff {
                    node.events.pop_front();
                } else {
                    break;
                }
            }
        }
    }
}
