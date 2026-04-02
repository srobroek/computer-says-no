#!/usr/bin/env bash
# Install computer-says-no (csn) and download default reference sets.
# Usage: curl -fsSL https://raw.githubusercontent.com/srobroek/computer-says-no/main/install.sh | bash

set -euo pipefail

REPO="srobroek/computer-says-no"
REPO_URL="https://raw.githubusercontent.com/${REPO}/main"

echo "==> Installing computer-says-no (csn)..."

# Check for cargo
if ! command -v cargo &>/dev/null; then
    echo "Error: cargo not found. Install Rust first: https://rustup.rs"
    exit 1
fi

# Install from crates.io
cargo install computer-says-no

# Determine config directory (matches the directories crate behavior)
case "$(uname -s)" in
    Darwin)
        CONFIG_DIR="${HOME}/Library/Application Support/computer-says-no"
        ;;
    *)
        CONFIG_DIR="${XDG_CONFIG_HOME:-${HOME}/.config}/computer-says-no"
        ;;
esac

SETS_DIR="${CONFIG_DIR}/reference-sets"

# Download default reference sets
echo "==> Downloading default reference sets to ${SETS_DIR}..."
mkdir -p "${SETS_DIR}"

curl -fsSL -o "${SETS_DIR}/corrections.toml" \
    "${REPO_URL}/reference-sets/corrections.toml"

echo "==> Downloaded: corrections.toml (correction/frustration/neutral, 1600+ phrases)"

# Verify
if command -v csn &>/dev/null; then
    echo ""
    echo "==> Installation complete!"
    echo ""
    echo "  csn sets list              # list reference sets"
    echo "  csn classify \"text\" --set corrections --json"
    echo ""
    echo "First classify call downloads the ONNX model (~500MB) and trains the MLP (~10s)."
    echo "Subsequent calls: ~5ms via background daemon."
elif [ -x "${HOME}/.cargo/bin/csn" ]; then
    echo ""
    echo "==> Installed to ~/.cargo/bin/csn (not on PATH)"
    echo "  Add ~/.cargo/bin to your PATH, or run directly:"
    echo "  ~/.cargo/bin/csn classify \"text\" --set corrections --json"
else
    echo ""
    echo "==> cargo install succeeded but csn binary not found. Check your PATH."
fi
