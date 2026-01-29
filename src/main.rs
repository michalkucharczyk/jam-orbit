//! Standalone CLI for testing the jamtart data pipeline
//!
//! Run with: cargo run --features cli --bin jam-cli

use futures_util::{SinkExt, StreamExt};
use tokio_tungstenite::{connect_async, tungstenite::Message};
use tracing::{error, info, warn};
use tracing_subscriber::{fmt, EnvFilter};

mod core;
use core::{parse_event, BestBlockData, TimeSeriesData};

fn init_logging() {
    let filter = EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| EnvFilter::new("info,jam_vis_poc=debug"));

    fmt()
        .with_env_filter(filter)
        .with_target(true)
        .with_thread_ids(false)
        .with_file(false)
        .with_line_number(false)
        .init();
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    init_logging();

    let url =
        std::env::var("JAMTART_WS").unwrap_or_else(|_| "ws://127.0.0.1:8080/api/ws".to_string());

    info!(url = %url, "Connecting to jamtart");

    let (ws_stream, _) = connect_async(&url).await.map_err(|e| {
        error!(error = %e, "Failed to connect to WebSocket");
        e
    })?;

    let (mut write, mut read) = ws_stream.split();

    info!("WebSocket connected, subscribing...");
    let subscribe = r#"{"type":"Subscribe","filter":{"type":"All"}}"#;
    write.send(Message::Text(subscribe.into())).await?;

    let mut time_series = TimeSeriesData::new(1024, 200);
    let mut blocks = BestBlockData::new(1024);
    let mut event_count = 0u64;
    let mut events_last_interval = 0u64;
    let mut stats_interval = tokio::time::interval(std::time::Duration::from_secs(5));

    info!("Subscribed, waiting for events...");

    loop {
        tokio::select! {
            msg = read.next() => {
                match msg {
                    Some(Ok(Message::Text(text))) => {
                        if parse_event(&text, &mut time_series, &mut blocks).is_some() {
                            event_count += 1;
                            events_last_interval += 1;
                        }
                    }
                    Some(Ok(Message::Close(_))) => {
                        warn!("WebSocket connection closed by server");
                        break;
                    }
                    Some(Err(e)) => {
                        error!(error = %e, "WebSocket error");
                    }
                    None => {
                        warn!("WebSocket connection closed");
                        break;
                    }
                    _ => {}
                }
            }
            _ = stats_interval.tick() => {
                info!(
                    validators = time_series.validator_count(),
                    events_total = event_count,
                    events_per_sec = format!("{:.1}", events_last_interval as f64 / 5.0),
                    highest_slot = ?blocks.highest_slot(),
                    time_series_points = time_series.max_series_len(),
                    "Pipeline stats"
                );
                events_last_interval = 0;
            }
        }
    }

    Ok(())
}
