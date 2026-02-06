//! Data structures for validators ring visualization
//!
//! - DirectedParticleInstance: GPU-ready particle data (24 bytes)
//! - PeerRegistry: Maps PeerId → node index
//! - DirectedEventBuffer: CPU-side ring buffer for directed events

use std::collections::{HashMap, VecDeque};

use crate::core::events::PeerId;

// ============================================================================
// DirectedParticleInstance - GPU particle data
// ============================================================================

/// GPU-ready particle instance for directed event visualization.
/// 24 bytes, suitable for GPU instancing.
#[repr(C)]
#[derive(Clone, Copy, Debug, Default)]
pub struct DirectedParticleInstance {
    /// Source validator index [0..1023]
    pub source_index: f32,
    /// Target validator index [0..1023]
    pub target_index: f32,
    /// Birth time (app-relative seconds)
    pub birth_time: f32,
    /// Travel duration in seconds (for speed variation per event type)
    pub travel_duration: f32,
    /// Event type discriminant (for color lookup) [0..255]
    pub event_type: f32,
    /// Path deviation seed [-1..1] for curved trajectories
    pub curve_seed: f32,
}

// bytemuck traits will be implemented when GPU renderer is added
// unsafe impl bytemuck::Pod for DirectedParticleInstance {}
// unsafe impl bytemuck::Zeroable for DirectedParticleInstance {}

impl DirectedParticleInstance {
    /// Create a new directed particle
    pub fn new(
        source_index: u16,
        target_index: u16,
        birth_time: f32,
        travel_duration: f32,
        event_type: u8,
        curve_seed: f32,
    ) -> Self {
        Self {
            source_index: source_index as f32,
            target_index: target_index as f32,
            birth_time,
            travel_duration,
            event_type: event_type as f32,
            curve_seed,
        }
    }
}

// ============================================================================
// PeerRegistry - PeerId to node index mapping
// ============================================================================

/// Registry mapping PeerId ([u8; 32]) to node index (u16).
/// Used to translate event peer IDs to positions on the validator ring.
#[derive(Debug)]
pub struct PeerRegistry {
    /// PeerId → node index mapping
    peer_to_index: HashMap<PeerId, u16>,
    /// Next available index
    next_index: u16,
    /// Maximum number of validators to track
    max_validators: u16,
}

impl Default for PeerRegistry {
    fn default() -> Self {
        Self::new(1024)
    }
}

impl PeerRegistry {
    /// Create a new registry with specified maximum validators
    pub fn new(max_validators: u16) -> Self {
        Self {
            peer_to_index: HashMap::with_capacity(max_validators as usize),
            next_index: 0,
            max_validators,
        }
    }

    /// Get index for a PeerId, creating a new one if not exists.
    /// Returns None if at capacity.
    pub fn get_or_insert(&mut self, peer_id: &PeerId) -> Option<u16> {
        if let Some(&idx) = self.peer_to_index.get(peer_id) {
            return Some(idx);
        }

        if self.next_index >= self.max_validators {
            return None; // At capacity
        }

        let idx = self.next_index;
        self.next_index += 1;
        self.peer_to_index.insert(*peer_id, idx);
        Some(idx)
    }

    /// Lookup only, no insertion
    #[inline]
    #[allow(dead_code)]
    pub fn get(&self, peer_id: &PeerId) -> Option<u16> {
        self.peer_to_index.get(peer_id).copied()
    }

    /// Number of registered peers
    pub fn len(&self) -> usize {
        self.peer_to_index.len()
    }

    /// Check if registry is empty
    #[allow(dead_code)]
    pub fn is_empty(&self) -> bool {
        self.peer_to_index.is_empty()
    }

    /// Clear all registrations
    #[allow(dead_code)]
    pub fn clear(&mut self) {
        self.peer_to_index.clear();
        self.next_index = 0;
    }
}

// ============================================================================
// DirectedEventBuffer - CPU-side ring buffer
// ============================================================================

/// CPU-side ring buffer for directed particle events.
/// Stores particles before GPU upload, supports filtering by event type.
#[derive(Debug)]
pub struct DirectedEventBuffer {
    /// Ring buffer of particles
    particles: VecDeque<DirectedParticleInstance>,
    /// Maximum capacity
    capacity: usize,
    /// Enabled event types as 256-bit bitfield (4 x u64)
    enabled_types: [u64; 4],
    /// Monotonic counter: total particles ever pushed (for incremental GPU upload)
    total_pushed: u64,
}

impl Default for DirectedEventBuffer {
    fn default() -> Self {
        Self::new(100_000)
    }
}

impl DirectedEventBuffer {
    /// Create a new buffer with specified capacity
    pub fn new(capacity: usize) -> Self {
        Self {
            particles: VecDeque::with_capacity(capacity.min(100_000)),
            capacity,
            enabled_types: [u64::MAX; 4], // All enabled by default
            total_pushed: 0,
        }
    }

    /// Push a new particle, evicting oldest if at capacity
    #[inline]
    pub fn push(&mut self, particle: DirectedParticleInstance) {
        if self.particles.len() >= self.capacity {
            self.particles.pop_front();
        }
        self.particles.push_back(particle);
        self.total_pushed += 1;
    }

    /// Get particles added since `cursor` for incremental GPU upload.
    /// Returns (particles deque, new cursor, number of items to skip).
    /// Caller should iterate `particles.iter().skip(skip)` to get only new items.
    /// If cursor is stale (evicted), returns all buffered particles (skip=0).
    pub fn get_new_since(&self, cursor: u64) -> (&VecDeque<DirectedParticleInstance>, u64, usize) {
        let oldest = self.total_pushed.saturating_sub(self.particles.len() as u64);
        let skip = if cursor >= oldest {
            (cursor - oldest) as usize
        } else {
            0
        };
        (&self.particles, self.total_pushed, skip)
    }

    /// Enable or disable an event type
    #[allow(dead_code)]
    pub fn set_type_enabled(&mut self, event_type: u8, enabled: bool) {
        let idx = (event_type / 64) as usize;
        let bit = event_type % 64;
        if enabled {
            self.enabled_types[idx] |= 1 << bit;
        } else {
            self.enabled_types[idx] &= !(1 << bit);
        }
    }

    /// Check if an event type is enabled
    #[inline]
    fn is_type_enabled(&self, event_type: u8) -> bool {
        let idx = (event_type / 64) as usize;
        let bit = event_type % 64;
        (self.enabled_types[idx] & (1 << bit)) != 0
    }

    /// Get all enabled types bitfield
    #[allow(dead_code)]
    pub fn enabled_types(&self) -> &[u64; 4] {
        &self.enabled_types
    }

    /// Set enabled types from bitfield
    #[allow(dead_code)]
    pub fn set_enabled_types(&mut self, enabled: [u64; 4]) {
        self.enabled_types = enabled;
    }

    /// Get active particles within time window, filtered by enabled types.
    /// Returns particles suitable for GPU upload.
    pub fn get_active_particles(&self, now: f32, max_age: f32) -> Vec<DirectedParticleInstance> {
        let cutoff = now - max_age;
        self.particles
            .iter()
            .filter(|p| {
                p.birth_time >= cutoff && self.is_type_enabled(p.event_type as u8)
            })
            .copied()
            .collect()
    }

    /// Get all particles (unfiltered) for debugging
    #[allow(dead_code)]
    pub fn all_particles(&self) -> &VecDeque<DirectedParticleInstance> {
        &self.particles
    }

    /// Number of particles in buffer
    pub fn len(&self) -> usize {
        self.particles.len()
    }

    /// Check if buffer is empty
    #[allow(dead_code)]
    pub fn is_empty(&self) -> bool {
        self.particles.is_empty()
    }

    /// Clear all particles
    #[allow(dead_code)]
    pub fn clear(&mut self) {
        self.particles.clear();
    }

    /// Retain only particles matching predicate
    #[allow(dead_code)]
    pub fn retain<F>(&mut self, f: F)
    where
        F: FnMut(&DirectedParticleInstance) -> bool,
    {
        self.particles.retain(f);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_peer_registry() {
        let mut registry = PeerRegistry::new(10);

        let peer1 = [1u8; 32];
        let peer2 = [2u8; 32];

        // First insert
        assert_eq!(registry.get_or_insert(&peer1), Some(0));
        assert_eq!(registry.get_or_insert(&peer2), Some(1));

        // Repeated lookup
        assert_eq!(registry.get_or_insert(&peer1), Some(0));
        assert_eq!(registry.get(&peer1), Some(0));
        assert_eq!(registry.get(&peer2), Some(1));

        // Unknown peer
        let peer3 = [3u8; 32];
        assert_eq!(registry.get(&peer3), None);
    }

    #[test]
    fn test_directed_event_buffer() {
        let mut buffer = DirectedEventBuffer::new(3);

        let p1 = DirectedParticleInstance::new(0, 1, 0.0, 1.0, 106, 0.5);
        let p2 = DirectedParticleInstance::new(1, 2, 1.0, 1.0, 128, -0.5);
        let p3 = DirectedParticleInstance::new(2, 3, 2.0, 1.0, 131, 0.0);
        let p4 = DirectedParticleInstance::new(3, 4, 3.0, 1.0, 106, 0.3);

        buffer.push(p1);
        buffer.push(p2);
        buffer.push(p3);
        assert_eq!(buffer.len(), 3);

        // Push past capacity evicts oldest
        buffer.push(p4);
        assert_eq!(buffer.len(), 3);

        // Filter by time
        let active = buffer.get_active_particles(3.5, 2.0);
        assert_eq!(active.len(), 2); // p3 and p4 (birth_time >= 1.5)

        // Disable event type 106
        buffer.set_type_enabled(106, false);
        let active = buffer.get_active_particles(3.5, 2.0);
        assert_eq!(active.len(), 1); // Only p3 (type 131)
    }

    #[test]
    fn test_type_filter_bitfield() {
        let mut buffer = DirectedEventBuffer::new(10);

        // All enabled by default
        assert!(buffer.is_type_enabled(0));
        assert!(buffer.is_type_enabled(63));
        assert!(buffer.is_type_enabled(64));
        assert!(buffer.is_type_enabled(255));

        // Disable some
        buffer.set_type_enabled(0, false);
        buffer.set_type_enabled(255, false);

        assert!(!buffer.is_type_enabled(0));
        assert!(buffer.is_type_enabled(1));
        assert!(!buffer.is_type_enabled(255));
        assert!(buffer.is_type_enabled(254));
    }
}
