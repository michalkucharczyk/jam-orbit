#!/bin/bash
set -e

echo "Building WASM..."
wasm-pack build --target web --release

echo ""
echo "Done! Serving..."
python3 -m http.server 8888
