//! Data structures for storing telemetry from jamtart
//!
//! These structures are platform-agnostic (no WASM deps) and shared
//! between the CLI and dashboard.

use std::collections::{HashMap, VecDeque};
use tracing::{debug, trace};

use super::events::{Event, GuaranteeDiscardReason};

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
            debug!(node_id, idx, "New node registered for events");
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

    /// Aggregate event rate across all nodes for given event types.
    /// Returns Vec<f64> of length num_buckets (newest last).
    pub fn compute_aggregate_rate(
        &self,
        event_types: &[u8],
        now: f64,
        bucket_duration: f64,
        num_buckets: usize,
    ) -> Vec<f64> {
        let aligned_now = (now / bucket_duration).floor() * bucket_duration;
        let oldest_time = aligned_now - (bucket_duration * num_buckets as f64);
        let mut buckets = vec![0.0_f64; num_buckets];

        for node in self.nodes.values() {
            for &et in event_types {
                if let Some(events) = node.by_type.get(&et) {
                    for stored in events {
                        if stored.timestamp < oldest_time || stored.timestamp >= aligned_now {
                            continue;
                        }
                        let age = aligned_now - stored.timestamp;
                        let bucket_idx = ((age / bucket_duration) as usize).min(num_buckets - 1);
                        let bucket_idx = num_buckets - 1 - bucket_idx;
                        buckets[bucket_idx] += 1.0;
                    }
                }
            }
        }

        buckets
    }

    /// Count events of specific types across all nodes in time window.
    pub fn count_events(&self, event_types: &[u8], now: f64, window: f64) -> u64 {
        let cutoff = now - window;
        let mut count = 0u64;

        for node in self.nodes.values() {
            for &et in event_types {
                if let Some(events) = node.by_type.get(&et) {
                    for stored in events {
                        if stored.timestamp >= cutoff {
                            count += 1;
                        }
                    }
                }
            }
        }

        count
    }

    /// Recent events of specific types with reason strings.
    /// Returns Vec<RecentError> sorted newest-first, up to `limit`.
    pub fn recent_errors(
        &self,
        event_types: &[u8],
        limit: usize,
        now: f64,
        max_age: f64,
    ) -> Vec<RecentError> {
        let cutoff = now - max_age;
        let mut errors: Vec<RecentError> = Vec::new();

        for node in self.nodes.values() {
            for &et in event_types {
                if let Some(events) = node.by_type.get(&et) {
                    for stored in events.iter().rev() {
                        if stored.timestamp < cutoff {
                            break;
                        }
                        let reason = stored.event.reason().unwrap_or("").to_string();
                        errors.push(RecentError {
                            timestamp: stored.timestamp,
                            node_index: node.index,
                            event_type: et,
                            reason,
                        });
                    }
                }
            }
        }

        // Sort newest-first
        errors.sort_by(|a, b| b.timestamp.partial_cmp(&a.timestamp).unwrap_or(std::cmp::Ordering::Equal));
        errors.truncate(limit);
        errors
    }

    /// Distribution of GuaranteeDiscarded reasons in time window.
    pub fn discard_reason_distribution(
        &self,
        now: f64,
        window: f64,
    ) -> Vec<(GuaranteeDiscardReason, u64)> {
        let cutoff = now - window;
        let mut counts: HashMap<u8, u64> = HashMap::new();

        for node in self.nodes.values() {
            if let Some(events) = node.by_type.get(&113) {
                for stored in events {
                    if stored.timestamp < cutoff {
                        continue;
                    }
                    if let Event::GuaranteeDiscarded { reason, .. } = &stored.event {
                        *counts.entry(*reason as u8).or_insert(0) += 1;
                    }
                }
            }
        }

        let all_reasons = [
            GuaranteeDiscardReason::PackageReportedOnChain,
            GuaranteeDiscardReason::ReplacedByBetter,
            GuaranteeDiscardReason::CannotReportOnChain,
            GuaranteeDiscardReason::TooManyGuarantees,
            GuaranteeDiscardReason::Other,
        ];

        all_reasons
            .iter()
            .filter_map(|&r| {
                let c = counts.get(&(r as u8)).copied().unwrap_or(0);
                if c > 0 { Some((r, c)) } else { None }
            })
            .collect()
    }
}

/// A recent error event for display
pub struct RecentError {
    pub timestamp: f64,
    pub node_index: u16,
    pub event_type: u8,
    pub reason: String,
}

// ============================================================================
// New data structures for Phase 3 panels
// ============================================================================

/// Guarantee queue depth per core, from Status(10).num_guarantees
pub struct GuaranteeQueueData {
    /// Latest num_guarantees per validator: [validator_idx] = Vec<u8>
    pub per_validator: Vec<Vec<u8>>,
    node_index: HashMap<String, usize>,
}

impl GuaranteeQueueData {
    pub fn new(max_validators: usize) -> Self {
        Self {
            per_validator: vec![Vec::new(); max_validators],
            node_index: HashMap::new(),
        }
    }

    pub fn update(&mut self, node_id: &str, num_guarantees: Vec<u8>) {
        let idx = self.get_or_create_index(node_id);
        self.per_validator[idx] = num_guarantees;
    }

    /// Sum guarantees per core across all validators -> Vec<u32>
    pub fn aggregate_per_core(&self) -> Vec<u32> {
        let max_cores = self.per_validator.iter().map(|v| v.len()).max().unwrap_or(0);
        if max_cores == 0 {
            return Vec::new();
        }
        let mut totals = vec![0u32; max_cores];
        for per_core in &self.per_validator {
            for (core_idx, &count) in per_core.iter().enumerate() {
                totals[core_idx] += count as u32;
            }
        }
        totals
    }

    fn get_or_create_index(&mut self, node_id: &str) -> usize {
        if let Some(&idx) = self.node_index.get(node_id) {
            return idx;
        }
        let idx = self.node_index.len().min(self.per_validator.len() - 1);
        self.node_index.insert(node_id.to_string(), idx);
        idx
    }
}

/// Per-validator sync status from SyncStatusChanged(13)
pub struct SyncStatusData {
    /// (synced, last_update_time) per validator
    pub status: Vec<(bool, f64)>,
    node_index: HashMap<String, usize>,
}

impl SyncStatusData {
    pub fn new(max_validators: usize) -> Self {
        Self {
            status: vec![(false, 0.0); max_validators],
            node_index: HashMap::new(),
        }
    }

    pub fn set(&mut self, node_id: &str, synced: bool, now: f64) {
        let idx = self.get_or_create_index(node_id);
        self.status[idx] = (synced, now);
    }

    pub fn synced_count(&self) -> usize {
        self.node_index.values().filter(|&&idx| self.status[idx].0).count()
    }

    pub fn total_count(&self) -> usize {
        self.node_index.len()
    }

    fn get_or_create_index(&mut self, node_id: &str) -> usize {
        if let Some(&idx) = self.node_index.get(node_id) {
            return idx;
        }
        let idx = self.node_index.len().min(self.status.len() - 1);
        self.node_index.insert(node_id.to_string(), idx);
        idx
    }
}

/// Shard metrics from Status(10): num_shards + shards_size time series
pub struct ShardMetrics {
    pub shard_counts: TimeSeriesData,
    pub shard_sizes: TimeSeriesData,
}

impl ShardMetrics {
    pub fn new(num_validators: usize, max_points: usize) -> Self {
        Self {
            shard_counts: TimeSeriesData::new(num_validators, max_points),
            shard_sizes: TimeSeriesData::new(num_validators, max_points),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::events::{Event, Reason, GuaranteeOutline, GuaranteeDiscardReason};

    #[test]
    fn test_time_series_push_and_eviction() {
        let mut ts = TimeSeriesData::new(2, 3);

        // Push values to first node
        ts.push("node1", 1.0);
        ts.push("node1", 2.0);
        ts.push("node1", 3.0);

        assert_eq!(ts.validator_count(), 1);
        assert_eq!(ts.max_series_len(), 3);

        // Push beyond max_points to trigger eviction
        ts.push("node1", 4.0);

        // Should still have 3 points (oldest evicted)
        assert_eq!(ts.max_series_len(), 3);

        // Push to second node
        ts.push("node2", 5.0);
        assert_eq!(ts.validator_count(), 2);

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
    fn test_event_store_push_and_count() {
        let mut store = EventStore::new(100, 60.0);

        let now = 50.0;

        // Push Status events to node1
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
        store.push("node1", status_event.clone(), now);
        store.push("node1", status_event.clone(), now - 1.0);

        // Push ConnectInFailed to node2
        let error_event = Event::ConnectInFailed {
            timestamp: 0,
            connecting_id: 0,
            reason: Reason("test error".into()),
        };
        store.push("node2", error_event, now - 2.0);

        assert_eq!(store.node_count(), 2);

        // Count Status events (type 10) within window
        let count = store.count_events(&[10], now, 10.0);
        assert_eq!(count, 2);

        // Count ConnectInFailed events (type 22)
        let count = store.count_events(&[22], now, 10.0);
        assert_eq!(count, 1);
    }

    #[test]
    fn test_compute_aggregate_rate() {
        let mut store = EventStore::new(100, 60.0);

        let now = 60.0;
        let bucket_duration = 1.0;
        let num_buckets = 60;

        // Push Status events at known timestamps
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

        store.push("node1", status_event.clone(), 59.0);
        store.push("node1", status_event.clone(), 59.5);
        store.push("node1", status_event.clone(), 58.0);

        let rates = store.compute_aggregate_rate(&[10], now, bucket_duration, num_buckets);

        // Verify we got the right number of buckets
        assert_eq!(rates.len(), num_buckets);

        // Sum should be 3 (all events within window)
        let total: f64 = rates.iter().sum();
        assert_eq!(total, 3.0);

        // Push event outside window (before oldest_time which is 60.0 - 60.0 = 0.0)
        // Need to push at negative time or we change now
        store.push("node1", status_event.clone(), -1.0);

        let rates = store.compute_aggregate_rate(&[10], now, bucket_duration, num_buckets);
        let total: f64 = rates.iter().sum();

        // Should still be 3 (old event excluded)
        assert_eq!(total, 3.0);
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

        // Count before prune
        let count_before = store.count_events(&[10], 100.0, 100.0);
        assert_eq!(count_before, 3);

        // Prune with current time = 100.0, retention = 30.0
        // Events older than 70.0 should be removed
        store.prune(100.0);

        // Count after prune (only event at 100.0 remains)
        let count_after = store.count_events(&[10], 100.0, 100.0);
        assert_eq!(count_after, 1);
    }

    #[test]
    fn test_recent_errors() {
        let mut store = EventStore::new(100, 60.0);

        let now = 100.0;

        // Push error events with different reasons and timestamps
        let error1 = Event::ConnectInFailed {
            timestamp: 0,
            connecting_id: 0,
            reason: Reason("error 1".into()),
        };
        let error2 = Event::ConnectInFailed {
            timestamp: 0,
            connecting_id: 0,
            reason: Reason("error 2".into()),
        };
        let error3 = Event::ConnectInFailed {
            timestamp: 0,
            connecting_id: 0,
            reason: Reason("error 3".into()),
        };

        store.push("node1", error1, now - 5.0);
        store.push("node2", error2, now - 3.0);
        store.push("node1", error3, now - 1.0);

        // Get recent errors, sorted newest-first
        let errors = store.recent_errors(&[22], 10, now, 60.0);

        assert_eq!(errors.len(), 3);

        // Verify newest first
        assert!(errors[0].reason.contains("error 3"));
        assert!(errors[1].reason.contains("error 2"));
        assert!(errors[2].reason.contains("error 1"));

        // Test limit
        let errors_limited = store.recent_errors(&[22], 2, now, 60.0);
        assert_eq!(errors_limited.len(), 2);

        // Test max_age filter
        let errors_recent = store.recent_errors(&[22], 10, now, 2.0);
        assert_eq!(errors_recent.len(), 1); // Only error 3 within 2.0s
    }

    #[test]
    fn test_discard_reason_distribution() {
        let mut store = EventStore::new(100, 60.0);

        let now = 100.0;

        // Create discard events with different reasons
        let discard1 = Event::GuaranteeDiscarded {
            timestamp: 0,
            outline: GuaranteeOutline {
                work_report_hash: [0u8; 32],
                slot: 0,
                guarantors: vec![],
            },
            reason: GuaranteeDiscardReason::PackageReportedOnChain,
        };
        let discard2 = Event::GuaranteeDiscarded {
            timestamp: 0,
            outline: GuaranteeOutline {
                work_report_hash: [1u8; 32],
                slot: 0,
                guarantors: vec![],
            },
            reason: GuaranteeDiscardReason::PackageReportedOnChain,
        };
        let discard3 = Event::GuaranteeDiscarded {
            timestamp: 0,
            outline: GuaranteeOutline {
                work_report_hash: [2u8; 32],
                slot: 0,
                guarantors: vec![],
            },
            reason: GuaranteeDiscardReason::ReplacedByBetter,
        };

        store.push("node1", discard1, now - 1.0);
        store.push("node1", discard2, now - 2.0);
        store.push("node2", discard3, now - 3.0);

        let distribution = store.discard_reason_distribution(now, 10.0);

        // Should have counts for both reasons
        assert!(distribution.len() >= 2);

        let on_chain_count = distribution
            .iter()
            .find(|(reason, _)| matches!(reason, GuaranteeDiscardReason::PackageReportedOnChain))
            .map(|(_, count)| *count)
            .unwrap_or(0);

        let replaced_count = distribution
            .iter()
            .find(|(reason, _)| matches!(reason, GuaranteeDiscardReason::ReplacedByBetter))
            .map(|(_, count)| *count)
            .unwrap_or(0);

        assert_eq!(on_chain_count, 2);
        assert_eq!(replaced_count, 1);
    }

    #[test]
    fn test_guarantee_queue_aggregate() {
        let mut queue = GuaranteeQueueData::new(10);

        // Update validators with per-core counts
        // Assuming each validator reports counts for multiple cores
        queue.update("node1", vec![1, 2, 3, 0, 0]);
        queue.update("node2", vec![2, 1, 0, 1, 0]);

        let aggregate = queue.aggregate_per_core();

        // Verify sums per core
        assert!(aggregate.len() >= 4);
        assert_eq!(aggregate[0], 3); // 1 + 2
        assert_eq!(aggregate[1], 3); // 2 + 1
        assert_eq!(aggregate[2], 3); // 3 + 0
        assert_eq!(aggregate[3], 1); // 0 + 1
    }

    #[test]
    fn test_sync_status() {
        let mut sync = SyncStatusData::new(10);

        let now = 100.0;

        // Set sync status for multiple nodes
        sync.set("node1", true, now);
        sync.set("node2", true, now);
        sync.set("node3", false, now);

        assert_eq!(sync.synced_count(), 2);
        assert_eq!(sync.total_count(), 3);

        // Change status
        sync.set("node1", false, now);

        assert_eq!(sync.synced_count(), 1);
        assert_eq!(sync.total_count(), 3);
    }
}
