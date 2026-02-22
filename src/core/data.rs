//! Data structures for storing telemetry from jamtart
//!
//! These structures are platform-agnostic (no WASM deps) and shared
//! between the CLI and dashboard.

use std::collections::{HashMap, VecDeque};
use tracing::trace;

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
            trace!(node_id, idx, "New validator registered for time series");
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

    /// Get the latest value (peer count) for a node_id
    pub fn latest_value(&self, node_id: &str) -> Option<f32> {
        self.node_index.get(node_id)
            .and_then(|&idx| self.series.get(idx))
            .and_then(|s| s.last().copied())
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
            trace!(node_id, idx, "New validator registered for blocks");
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

/// Events for a single node, organized by event type for O(1) filtered access
pub struct NodeEvents {
    /// Events grouped by event_type: event_type → ring buffer
    pub by_type: HashMap<u8, VecDeque<StoredEvent>>,
    /// Node index (for visualization X coordinate)
    pub index: u16,
    /// Max events per type (ring buffer capacity)
    max_per_type: usize,
}

impl NodeEvents {
    fn new(index: u16, max_per_type: usize) -> Self {
        Self {
            by_type: HashMap::new(),
            index,
            max_per_type,
        }
    }

    /// Push an event into the appropriate type bucket
    fn push(&mut self, event: Event, timestamp: f64) {
        let event_type = event.event_type() as u8;
        let max = self.max_per_type;

        let bucket = self.by_type.entry(event_type).or_insert_with(|| {
            VecDeque::with_capacity(max.min(256)) // reasonable initial capacity
        });

        if bucket.len() >= max {
            bucket.pop_front();
        }
        bucket.push_back(StoredEvent { timestamp, event });
    }

    /// Total event count across all types
    pub fn total_events(&self) -> usize {
        self.by_type.values().map(|v| v.len()).sum()
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
            trace!(node_id, idx, "New node registered for events");
            NodeEvents::new(idx, max_events)
        });

        node.push(event, timestamp);

        trace!(
            node_id,
            events_count = node.total_events(),
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

    /// Iterate events for a specific node and event type
    #[allow(dead_code)]
    pub fn node_events(&self, node_id: &str, event_type: u8) -> Option<&VecDeque<StoredEvent>> {
        self.nodes
            .get(node_id)
            .and_then(|n| n.by_type.get(&event_type))
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
        // Align bucket boundaries to fixed time intervals to prevent oscillation
        // at bucket edges due to floating-point precision issues.
        // Floor now to the bucket duration so that bucket boundaries are stable.
        let aligned_now = (now / bucket_duration).floor() * bucket_duration;
        let oldest_time = aligned_now - (bucket_duration * num_buckets as f64);

        self.nodes
            .values()
            .map(|node| {
                let mut buckets = vec![0u32; num_buckets];

                // Only iterate over event types that are selected in the filter
                for (&event_type, events) in &node.by_type {
                    if (event_type as usize) >= event_filter.len()
                        || !event_filter[event_type as usize]
                    {
                        continue; // Skip entire event type bucket
                    }

                    for stored in events {
                        // Skip events outside time window
                        if stored.timestamp < oldest_time || stored.timestamp >= aligned_now {
                            continue;
                        }

                        // Compute bucket index using aligned time
                        let age = aligned_now - stored.timestamp;
                        let bucket_idx = ((age / bucket_duration) as usize).min(num_buckets - 1);
                        // Invert so newest is at the end
                        let bucket_idx = num_buckets - 1 - bucket_idx;
                        buckets[bucket_idx] += 1;
                    }
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
            for events in node.by_type.values_mut() {
                while let Some(front) = events.front() {
                    if front.timestamp < cutoff {
                        events.pop_front();
                    } else {
                        break;
                    }
                }
            }
        }
    }

}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::events::Event;

    #[test]
    fn test_time_series_push_and_eviction() {
        let mut ts = TimeSeriesData::new(2, 3);

        // Push values to first node
        ts.push("node1", 1.0);
        ts.push("node1", 2.0);
        ts.push("node1", 3.0);

        assert!(ts.latest_value("node1").is_some());
        assert!(ts.latest_value("node2").is_none());
        assert_eq!(ts.max_series_len(), 3);

        // Push beyond max_points to trigger eviction
        ts.push("node1", 4.0);

        // Should still have 3 points (oldest evicted)
        assert_eq!(ts.max_series_len(), 3);

        // Push to second node
        ts.push("node2", 5.0);
        assert!(ts.latest_value("node2").is_some());

        // Verify point_count - returns length of first series
        assert_eq!(ts.point_count(), 3); // node1 has 3 points
    }

    #[test]
    fn test_best_block_data() {
        let mut bbd = BestBlockData::new(10);

        // Set best blocks for two nodes
        bbd.set_best("node1", 100);
        bbd.set_best("node2", 150);

        // Set finalized for one node
        bbd.set_finalized("node1", 90);

        // Verify highest_slot filters zeros and returns max
        assert_eq!(bbd.highest_slot(), Some(150));

        // Verify highest_finalized filters zeros
        assert_eq!(bbd.highest_finalized(), Some(90));

        // Set a higher finalized
        bbd.set_finalized("node2", 95);
        assert_eq!(bbd.highest_finalized(), Some(95));
    }

    #[test]
    fn test_compute_rates_per_node() {
        let mut store = EventStore::new(100, 60.0);

        let now = 60.0;
        let bucket_duration = 1.0;
        let num_buckets = 10;

        let status_event = Event::Status {
            timestamp: 0,
            num_peers: 1,
            num_val_peers: 0,
            num_sync_peers: 0,
            num_guarantees: vec![],
            num_shards: 0,
            shards_size: 0,
            num_preimages: 0,
            preimages_size: 0,
        };

        // Push to node1
        store.push("node1", status_event.clone(), now - 1.0);
        store.push("node1", status_event.clone(), now - 2.0);

        // Push to node2
        store.push("node2", status_event.clone(), now - 1.0);

        let rates = store.compute_rates_per_node(now, bucket_duration, num_buckets, &[true; 256]);

        // Should have 2 nodes
        assert_eq!(rates.len(), 2);

        // Each entry has buckets
        for (node_idx, buckets) in &rates {
            assert!(node_idx < &2);
            assert_eq!(buckets.len(), num_buckets);

            let total: u32 = buckets.iter().sum();
            if store.node_index("node1").unwrap() == *node_idx {
                assert_eq!(total, 2);
            } else {
                assert_eq!(total, 1);
            }
        }
    }

    #[test]
    fn test_prune() {
        let mut store = EventStore::new(100, 30.0);

        let status_event = Event::Status {
            timestamp: 0,
            num_peers: 1,
            num_val_peers: 0,
            num_sync_peers: 0,
            num_guarantees: vec![],
            num_shards: 0,
            shards_size: 0,
            num_preimages: 0,
            preimages_size: 0,
        };

        // Push events at different timestamps (in chronological order)
        store.push("node1", status_event.clone(), 10.0);
        store.push("node1", status_event.clone(), 50.0);
        store.push("node1", status_event.clone(), 100.0);

        // Prune with current time = 100.0, retention = 30.0
        // Events older than 70.0 should be removed
        store.prune(100.0);

        // cutoff = 100 - 30 = 70, so events at 10.0 and 50.0 are pruned, only 100.0 remains
        assert_eq!(store.node_count(), 1);
        // Verify only 1 Status event remains for node1
        let remaining = store.node_events("node1", 10).unwrap();
        assert_eq!(remaining.len(), 1);
    }

}
