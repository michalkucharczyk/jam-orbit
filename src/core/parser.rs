//! Event parser for jamtart WebSocket messages
//!
//! Parses all JIP-3 events and stores them in EventStore.
//! Special handling for Status, BestBlockChanged, FinalizedBlockChanged.
//! Directed events populate the vring visualization buffers.

use super::{BestBlockData, Event, EventStore, TimeSeriesData, GuaranteeQueueData, SyncStatusData, ShardMetrics};
use super::events::EventType;
use crate::vring::{DirectedEventBuffer, DirectedParticleInstance, PulseEvent};
use serde_json::Value;
use tracing::{debug, info, trace, warn};

/// Parse a WebSocket message and update data structures
///
/// Returns Some(()) if an event was successfully parsed, None otherwise.
pub fn parse_event(
    msg: &str,
    time_series: &mut TimeSeriesData,
    blocks: &mut BestBlockData,
    events: &mut EventStore,
    directed_buffer: &mut DirectedEventBuffer,
    pulse_events: &mut Vec<PulseEvent>,
    guarantee_queue: &mut GuaranteeQueueData,
    sync_status: &mut SyncStatusData,
    shard_metrics: &mut ShardMetrics,
    now: f64,
) -> Option<()> {
    trace!(len = msg.len(), "Parsing message");

    let json: Value = serde_json::from_str(msg)
        .map_err(|e| {
            warn!(error = %e, "Failed to parse JSON");
        })
        .ok()?;

    // Only process "event" type messages
    let msg_type = json["type"].as_str()?;
    if msg_type != "event" {
        // Not an event (could be "connected", "subscribed", "stats")
        return None;
    }

    let node_id = json["data"]["node_id"].as_str()?;

    // Parse the full Event enum from the "event" field
    let event_json = &json["data"]["event"];
    let event: Event = serde_json::from_value(event_json.clone())
        .map_err(|e| {
            trace!(error = %e, "Failed to parse Event enum");
        })
        .ok()?;

    // Store full event for all visualizations
    events.push(node_id, event.clone(), now);

    // Emit collapsing-pulse for Authoring and WorkPackageSubmission
    match event.event_type() {
        EventType::Authoring | EventType::WorkPackageSubmission => {
            if let Some(node_index) = events.node_index(node_id) {
                info!(
                    event_type = ?event.event_type(),
                    node_id = &node_id[..8],
                    node_index,
                    "PULSE emitted"
                );
                pulse_events.push(PulseEvent {
                    node_index,
                    event_type: event.event_type() as u8,
                    birth_time: now as f32,
                });
            }
        }
        _ => {}
    }

    // Handle directed events for vring visualization
    if let Some(directed) = event.directed_peer() {
        // Resolve peer_id to node_id via hex encoding (jamtart uses hex::encode(peer_id) as node_id)
        let peer_node_id = hex::encode(directed.peer_id);
        if let Some(peer_index) = events.node_index(&peer_node_id) {
            if let Some(node_index) = events.node_index(node_id) {
                let (source, target) = if directed.is_outbound {
                    (node_index, peer_index)
                } else {
                    (peer_index, node_index)
                };

                let et = event.event_type() as u8;
                // Log WP-related directed events (90-113) for debugging
                if et >= 90 && et <= 113 {
                    info!(
                        event_type = et,
                        event_name = crate::core::events::event_name(et),
                        emitter = &node_id[..8],
                        emitter_idx = node_index,
                        peer = &peer_node_id[..8],
                        peer_idx = peer_index,
                        is_outbound = directed.is_outbound,
                        source,
                        target,
                        "WP directed particle"
                    );
                }

                let curve_seed = {
                    let bytes = directed.peer_id;
                    let combined =
                        (bytes[0] as i32) ^ (bytes[1] as i32) ^ (bytes[2] as i32) ^ (bytes[3] as i32);
                    (combined as f32 / 127.5) - 1.0 // Range [-1, 1]
                };

                let particle = DirectedParticleInstance::new(
                    source,
                    target,
                    now as f32,
                    event.travel_duration(),
                    event.event_type() as u8,
                    curve_seed,
                );
                directed_buffer.push(particle);
            }
        }
    } else {
        // Non-directed event: radial particle (source == target = radial sentinel)
        let dominated_by_pulse = matches!(
            event.event_type(),
            EventType::Authoring | EventType::WorkPackageSubmission
        );
        if !dominated_by_pulse {
            if let Some(node_index) = events.node_index(node_id) {
                let particle = DirectedParticleInstance::new(
                    node_index,
                    node_index, // same index = radial mode
                    now as f32,
                    1.0, // fixed 1.0s
                    event.event_type() as u8,
                    0.0, // no curve
                );
                directed_buffer.push(particle);
            }
        }
    }

    // Special handling for specific event types
    match &event {
        Event::Status { num_peers, num_guarantees, num_shards, shards_size, .. } => {
            debug!(node_id, num_peers, "Status event");
            time_series.push(node_id, *num_peers as f32);
            guarantee_queue.update(node_id, num_guarantees.clone());
            shard_metrics.shard_counts.push(node_id, *num_shards as f32);
            shard_metrics.shard_sizes.push(node_id, *shards_size as f32);
        }
        Event::BestBlockChanged { slot, .. } => {
            debug!(node_id, slot, "BestBlockChanged event");
            blocks.set_best(node_id, *slot as u64);
        }
        Event::FinalizedBlockChanged { slot, .. } => {
            debug!(node_id, slot, "FinalizedBlockChanged event");
            blocks.set_finalized(node_id, *slot as u64);
        }
        Event::SyncStatusChanged { synced, .. } => {
            debug!(node_id, synced, "SyncStatusChanged event");
            sync_status.set(node_id, *synced, now);
        }
        _ => {
            // Other events stored but not specially handled
            trace!(event_type = ?event.event_type(), "Event stored");
        }
    }

    Some(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_test_stores() -> (TimeSeriesData, BestBlockData, EventStore, DirectedEventBuffer, Vec<PulseEvent>, GuaranteeQueueData, SyncStatusData, ShardMetrics) {
        (
            TimeSeriesData::new(10, 100),
            BestBlockData::new(10),
            EventStore::new(100, 60.0),
            DirectedEventBuffer::default(),
            Vec::new(),
            GuaranteeQueueData::new(10),
            SyncStatusData::new(10),
            ShardMetrics::new(10, 100),
        )
    }

    #[test]
    fn test_parse_status_event() {
        let (mut ts, mut blocks, mut events, mut db, mut pe, mut gq, mut ss, mut sm) = make_test_stores();

        let msg = r#"{
            "type": "event",
            "data": {
                "event": {
                    "Status": {
                        "num_peers": 42,
                        "num_val_peers": 2,
                        "num_sync_peers": 1,
                        "num_guarantees": [],
                        "num_shards": 0,
                        "shards_size": 0,
                        "num_preimages": 0,
                        "preimages_size": 0,
                        "timestamp": 12345
                    }
                },
                "event_type": 10,
                "node_id": "abc123"
            }
        }"#;

        let result = parse_event(msg, &mut ts, &mut blocks, &mut events, &mut db, &mut pe, &mut gq, &mut ss, &mut sm, 0.0);
        assert!(result.is_some());
        assert_eq!(ts.validator_count(), 1);
        assert_eq!(events.node_count(), 1);
    }

    #[test]
    fn test_parse_best_block_event() {
        let (mut ts, mut blocks, mut events, mut db, mut pe, mut gq, mut ss, mut sm) = make_test_stores();

        let msg = r#"{
            "type": "event",
            "data": {
                "event": {
                    "BestBlockChanged": {
                        "slot": 5662737,
                        "hash": [1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16, 17, 18, 19, 20, 21, 22, 23, 24, 25, 26, 27, 28, 29, 30, 31, 32],
                        "timestamp": 12345
                    }
                },
                "event_type": 11,
                "node_id": "abc123"
            }
        }"#;

        let result = parse_event(msg, &mut ts, &mut blocks, &mut events, &mut db, &mut pe, &mut gq, &mut ss, &mut sm, 0.0);
        assert!(result.is_some());
        assert_eq!(blocks.highest_slot(), Some(5662737));
        assert_eq!(events.node_count(), 1);
    }

    #[test]
    fn test_ignore_non_event() {
        let (mut ts, mut blocks, mut events, mut db, mut pe, mut gq, mut ss, mut sm) = make_test_stores();

        let msg = r#"{"type": "connected", "data": {"message": "hello"}}"#;

        let result = parse_event(msg, &mut ts, &mut blocks, &mut events, &mut db, &mut pe, &mut gq, &mut ss, &mut sm, 0.0);
        assert!(result.is_none());
    }

    #[test]
    fn test_parse_directed_event() {
        let (mut ts, mut blocks, mut events, mut db, mut pe, mut gq, mut ss, mut sm) = make_test_stores();

        // The recipient peer_id [1,2,3,...,32] hex-encodes to this node_id.
        // We must pre-register this node in EventStore so the parser can resolve it.
        let recipient_node_id = hex::encode([1u8, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16, 17, 18, 19, 20, 21, 22, 23, 24, 25, 26, 27, 28, 29, 30, 31, 32]);
        // Seed EventStore with a dummy event so the recipient has a node_index
        let dummy_msg = format!(r#"{{
            "type": "event",
            "data": {{
                "event": {{
                    "Status": {{
                        "num_peers": 1, "num_val_peers": 0, "num_sync_peers": 0,
                        "num_guarantees": [], "num_shards": 0, "shards_size": 0,
                        "num_preimages": 0, "preimages_size": 0, "timestamp": 0
                    }}
                }},
                "event_type": 10,
                "node_id": "{}"
            }}
        }}"#, recipient_node_id);
        parse_event(&dummy_msg, &mut ts, &mut blocks, &mut events, &mut db, &mut pe, &mut gq, &mut ss, &mut sm, 0.0);

        // SendingGuarantee is a directed event (outbound to recipient peer)
        let msg = r#"{
            "type": "event",
            "data": {
                "event": {
                    "SendingGuarantee": {
                        "timestamp": 12345,
                        "built_id": 1,
                        "recipient": [1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16, 17, 18, 19, 20, 21, 22, 23, 24, 25, 26, 27, 28, 29, 30, 31, 32]
                    }
                },
                "event_type": 106,
                "node_id": "node_abc"
            }
        }"#;

        let result = parse_event(msg, &mut ts, &mut blocks, &mut events, &mut db, &mut pe, &mut gq, &mut ss, &mut sm, 1.5);
        assert!(result.is_some());

        // 2 particles: 1 radial from dummy Status + 1 directed from SendingGuarantee
        assert_eq!(db.len(), 2);

        // Verify directed particle properties (find by event_type 106)
        let particles = db.get_active_particles(2.0, 5.0);
        let p = particles.iter().find(|p| p.event_type == 106.0).expect("directed particle");
        assert_eq!(p.source_index, 1.0); // node_abc = second node registered (index 1)
        assert_eq!(p.target_index, 0.0); // recipient = first node registered (index 0)
        assert_eq!(p.birth_time, 1.5);

        // Verify radial particle from dummy Status (source == target = radial sentinel)
        let r = particles.iter().find(|p| p.event_type == 10.0).expect("radial particle");
        assert_eq!(r.source_index, r.target_index); // radial sentinel
    }
}
