//! Event parser for jamtart WebSocket messages
//!
//! Parses all JIP-3 events and stores them in EventStore.
//! Special handling for Status, BestBlockChanged, FinalizedBlockChanged.

use super::{BestBlockData, Event, EventStore, TimeSeriesData};
use serde_json::Value;
use tracing::{debug, trace, warn};

/// Parse a WebSocket message and update data structures
///
/// Returns Some(()) if an event was successfully parsed, None otherwise.
pub fn parse_event(
    msg: &str,
    time_series: &mut TimeSeriesData,
    blocks: &mut BestBlockData,
    events: &mut EventStore,
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

    // Special handling for specific event types
    match &event {
        Event::Status { num_peers, .. } => {
            debug!(node_id, num_peers, "Status event");
            time_series.push(node_id, *num_peers as f32);
        }
        Event::BestBlockChanged { slot, .. } => {
            debug!(node_id, slot, "BestBlockChanged event");
            blocks.set_best(node_id, *slot as u64);
        }
        Event::FinalizedBlockChanged { slot, .. } => {
            debug!(node_id, slot, "FinalizedBlockChanged event");
            blocks.set_finalized(node_id, *slot as u64);
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

    #[test]
    fn test_parse_status_event() {
        let mut ts = TimeSeriesData::new(10, 100);
        let mut blocks = BestBlockData::new(10);
        let mut events = EventStore::new(100, 60.0);

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

        let result = parse_event(msg, &mut ts, &mut blocks, &mut events, 0.0);
        assert!(result.is_some());
        assert_eq!(ts.validator_count(), 1);
        assert_eq!(events.node_count(), 1);
    }

    #[test]
    fn test_parse_best_block_event() {
        let mut ts = TimeSeriesData::new(10, 100);
        let mut blocks = BestBlockData::new(10);
        let mut events = EventStore::new(100, 60.0);

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

        let result = parse_event(msg, &mut ts, &mut blocks, &mut events, 0.0);
        assert!(result.is_some());
        assert_eq!(blocks.highest_slot(), Some(5662737));
        assert_eq!(events.node_count(), 1);
    }

    #[test]
    fn test_ignore_non_event() {
        let mut ts = TimeSeriesData::new(10, 100);
        let mut blocks = BestBlockData::new(10);
        let mut events = EventStore::new(100, 60.0);

        let msg = r#"{"type": "connected", "data": {"message": "hello"}}"#;

        let result = parse_event(msg, &mut ts, &mut blocks, &mut events, 0.0);
        assert!(result.is_none());
    }
}
