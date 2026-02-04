//! Standalone CLI for testing the jamtart data pipeline
//!
//! Run with: cargo run --bin jam-cli

#[cfg(not(target_arch = "wasm32"))]
mod core;

#[cfg(not(target_arch = "wasm32"))]
#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    use core::{parse_event, BestBlockData, EventStore, TimeSeriesData};
    use futures_util::{SinkExt, StreamExt};
    use std::time::Instant;
    use tokio_tungstenite::{connect_async, tungstenite::Message};
    use tracing::{error, info, warn};
    use tracing_subscriber::{fmt, EnvFilter};

    let filter = EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| EnvFilter::new("info,jam_vis_poc=debug"));
    fmt().with_env_filter(filter).with_target(true).init();

    let url =
        std::env::var("JAMTART_WS").unwrap_or_else(|_| "ws://127.0.0.1:8080/api/ws".to_string());

    info!(url = %url, "Connecting to jamtart");
    let (ws_stream, _) = connect_async(&url).await?;
    let (mut write, mut read) = ws_stream.split();

    info!("WebSocket connected, subscribing...");
    write.send(Message::Text(r#"{"type":"Subscribe","filter":{"type":"All"}}"#.into())).await?;

    let start_time = Instant::now();
    let mut time_series = TimeSeriesData::new(1024, 200);
    let mut blocks = BestBlockData::new(1024);
    let mut events = EventStore::new(50000, 60.0);
    let mut event_count = 0u64;
    let mut events_last_interval = 0u64;
    let mut stats_interval = tokio::time::interval(std::time::Duration::from_secs(5));

    info!("Subscribed, waiting for events...");

    loop {
        tokio::select! {
            msg = read.next() => {
                match msg {
                    Some(Ok(Message::Text(text))) => {
                        let now = start_time.elapsed().as_secs_f64();
                        if parse_event(&text, &mut time_series, &mut blocks, &mut events, now).is_some() {
                            event_count += 1;
                            events_last_interval += 1;
                        }
                    }
                    Some(Ok(Message::Close(_))) | None => {
                        warn!("WebSocket closed");
                        break;
                    }
                    Some(Err(e)) => error!(error = %e, "WebSocket error"),
                    _ => {}
                }
            }
            _ = stats_interval.tick() => {
                info!(
                    validators = time_series.validator_count(),
                    nodes = events.node_count(),
                    events = event_count,
                    "/sec" = format!("{:.1}", events_last_interval as f64 / 5.0),
                    slot = ?blocks.highest_slot(),
                    "stats"
                );
                events_last_interval = 0;
            }
        }
    }
    Ok(())
}

#[cfg(target_arch = "wasm32")]
fn main() {}
