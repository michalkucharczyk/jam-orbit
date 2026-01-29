//! Data structures for storing telemetry from jamtart
//!
//! These structures are platform-agnostic (no WASM deps) and shared
//! between the CLI and dashboard.

use std::collections::HashMap;
use tracing::{debug, trace};

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
    pub fn max_series_len(&self) -> usize {
        self.series.iter().map(|s| s.len()).max().unwrap_or(0)
    }

    /// Number of data points in the first non-empty series
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
    pub fn validator_count(&self) -> usize {
        self.node_index.len()
    }

    /// Highest best block slot across all validators
    pub fn highest_slot(&self) -> Option<u64> {
        self.best_blocks.iter().copied().filter(|&s| s > 0).max()
    }

    /// Highest finalized slot across all validators
    pub fn highest_finalized(&self) -> Option<u64> {
        self.finalized_blocks.iter().copied().filter(|&s| s > 0).max()
    }
}
