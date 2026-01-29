//! Event parser for jamtart WebSocket messages
//!
//! Parses events 10 (Status), 11 (BestBlockChanged), 12 (FinalizedBlockChanged)

use super::{BestBlockData, TimeSeriesData};
use serde_json::Value;
use tracing::{debug, trace, warn};

/// Parse a WebSocket message and update data structures
///
/// Returns Some(()) if an event was successfully parsed, None otherwise.
pub fn parse_event(
    msg: &str,
    time_series: &mut TimeSeriesData,
    blocks: &mut BestBlockData,
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

    let event_type = json["data"]["event_type"].as_u64()?;
    let node_id = json["data"]["node_id"].as_str()?;

    match event_type {
        10 => {
            // Status event - extract num_peers
            let num_peers = json["data"]["event"]["Status"]["num_peers"].as_u64()?;
            debug!(node_id, num_peers, "Status event");
            time_series.push(node_id, num_peers as f32);
            Some(())
        }
        11 => {
            // BestBlockChanged - extract slot
            let slot = json["data"]["event"]["BestBlockChanged"]["slot"].as_u64()?;
            debug!(node_id, slot, "BestBlockChanged event");
            blocks.set_best(node_id, slot);
            Some(())
        }
        12 => {
            // FinalizedBlockChanged - extract slot
            let slot = json["data"]["event"]["FinalizedBlockChanged"]["slot"].as_u64()?;
            debug!(node_id, slot, "FinalizedBlockChanged event");
            blocks.set_finalized(node_id, slot);
            Some(())
        }
        other => {
            // Unknown event type - ignore (there are ~130 event types, we only care about 3)
            trace!(event_type = other, "Ignoring event type");
            None
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_status_event() {
        let mut ts = TimeSeriesData::new(10, 100);
        let mut blocks = BestBlockData::new(10);

        let msg = r#"{
            "type": "event",
            "data": {
                "event": {
                    "Status": {
                        "num_peers": 42,
                        "num_val_peers": 2,
                        "timestamp": 12345
                    }
                },
                "event_type": 10,
                "node_id": "abc123"
            }
        }"#;

        let result = parse_event(msg, &mut ts, &mut blocks);
        assert!(result.is_some());
        assert_eq!(ts.validator_count(), 1);
    }

    #[test]
    fn test_parse_best_block_event() {
        let mut ts = TimeSeriesData::new(10, 100);
        let mut blocks = BestBlockData::new(10);

        let msg = r#"{
            "type": "event",
            "data": {
                "event": {
                    "BestBlockChanged": {
                        "slot": 5662737,
                        "hash": [1, 2, 3],
                        "timestamp": 12345
                    }
                },
                "event_type": 11,
                "node_id": "abc123"
            }
        }"#;

        let result = parse_event(msg, &mut ts, &mut blocks);
        assert!(result.is_some());
        assert_eq!(blocks.highest_slot(), Some(5662737));
    }

    #[test]
    fn test_ignore_non_event() {
        let mut ts = TimeSeriesData::new(10, 100);
        let mut blocks = BestBlockData::new(10);

        let msg = r#"{"type": "connected", "data": {"message": "hello"}}"#;

        let result = parse_event(msg, &mut ts, &mut blocks);
        assert!(result.is_none());
    }
}
