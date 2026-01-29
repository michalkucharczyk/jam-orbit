# JAM Visualization PoC

Real-time telemetry dashboard for JAM validators. Connects to [jamtart](https://github.com/jamtart) via WebSocket and visualizes validator metrics.

## Quick Start

**CLI** (for testing/debugging):
```bash
cargo run --bin jam-cli
```

**Dashboard** (WASM):
```bash
./build.sh  # builds and serves at http://localhost:8888
```

## Data Flow

```
jamtart (ws://127.0.0.1:8080/api/ws)
    │
    │ WebSocket
    ▼
┌─────────────────────────────────────────┐
│              parse_event()              │
│  Event 10 (Status)        → num_peers   │
│  Event 11 (BestBlock)     → slot        │
│  Event 12 (FinalizedBlock)→ slot        │
└─────────────────────────────────────────┘
    │
    ▼
┌─────────────────────────────────────────┐
│            Data Structures              │
│  TimeSeriesData: num_peers over time    │
│  BestBlockData:  slot per validator     │
└─────────────────────────────────────────┘
    │
    ▼
┌─────────────────────────────────────────┐
│              Rendering                  │
│  CLI:  tracing logs to stdout           │
│  WASM: egui plots in browser            │
└─────────────────────────────────────────┘
```

## Architecture

```
src/
├── lib.rs           # WASM entry point (egui dashboard)
├── main.rs          # CLI entry point (native only)
├── core/
│   ├── data.rs      # TimeSeriesData, BestBlockData
│   └── parser.rs    # parse_event() for events 10, 11, 12
├── websocket_wasm.rs # Browser WebSocket client
└── theme.rs         # UI colors
```

**Target-specific dependencies** (no feature flags needed):
- `wasm32`: egui, wasm-bindgen, web-sys
- `native`: tokio, tokio-tungstenite

## Environment Variables

| Variable | Default | Description |
|----------|---------|-------------|
| `JAMTART_WS` | `ws://127.0.0.1:8080/api/ws` | WebSocket endpoint |
| `RUST_LOG` | `info,jam_vis_poc=debug` | Log level (CLI only) |
