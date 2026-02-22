# JAM Orbit

Real-time telemetry dashboard for [JAM](https://graypaper.com/) validators. Connects to a jamtart node via WebSocket and visualizes network activity — validator ring, event particles, block progression, and peer metrics.

Supports filtering by event type. Built with [egui](https://github.com/emilk/egui) + wgpu. Runs natively and in the browser (WASM).

## Quick Start

**Native:**
```bash
cargo run --release --bin jam-orbit
```

**WASM:**
```bash
./build.sh  # builds and serves at http://localhost:8888
```

## Environment Variables

| Variable | Default | Description |
|----------|---------|-------------|
| `JAMTART_WS` | `ws://127.0.0.1:8080/api/ws` | WebSocket endpoint |
| `RUST_LOG` | `info,jam_orbit=debug` | Log level (native only) |

## License

Licensed under the MIT License — see [LICENSE](LICENSE) for details.
