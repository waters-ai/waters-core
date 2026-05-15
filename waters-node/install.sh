#!/bin/bash
set -euo pipefail

# ═══════════════════════════════════════════
# waters-node — one command install
# ═══════════════════════════════════════════

BASE_URL="https://kapelka.h2o-mining.space/download"
VERSION="v0.2.0"
ARCH="linux-x64"
FILENAME="waters-node-${VERSION}-${ARCH}.tar.gz"

echo "🌊 Installing waters-node ${VERSION}..."
echo ""

# Detect architecture
case "$(uname -m)" in
    x86_64) ARCH="linux-x64" ;;
    aarch64|arm64) ARCH="linux-arm64" ;;
    *)
        echo "Unsupported architecture: $(uname -m)"
        exit 1
        ;;
esac

FILENAME="waters-node-${VERSION}-${ARCH}.tar.gz"
URL="${BASE_URL}/${FILENAME}"

# Download
echo "   Downloading ${URL}..."
if command -v curl &>/dev/null; then
    curl -sL "${URL}" -o "/tmp/${FILENAME}"
elif command -v wget &>/dev/null; then
    wget -q "${URL}" -O "/tmp/${FILENAME}"
else
    echo "   Need curl or wget"
    exit 1
fi

# Extract
echo "   Extracting..."
tar xzf "/tmp/${FILENAME}" -C /tmp/

# Install
mkdir -p ~/.local/bin
cp /tmp/waters-node ~/.local/bin/
chmod +x ~/.local/bin/waters-node

# Cleanup
rm -f "/tmp/${FILENAME}" /tmp/waters-node

echo ""
echo "✅ waters-node ${VERSION} installed to ~/.local/bin/waters-node"
echo ""

# Setup DEEPSEEK_API_KEY if not set
if [ -z "${DEEPSEEK_API_KEY:-}" ]; then
    if [ -f ~/.deepseek/config.toml ]; then
        echo "   Using DeepSeek key from ~/.deepseek/config.toml"
    else
        echo "   ⚡ DEEPSEEK_API_KEY not set."
        echo "   Set it to use DeepSeek Chat:"
        echo "     export DEEPSEEK_API_KEY=\"your-key\""
        echo ""
        echo "   Or get a key at: https://platform.deepseek.com"
    fi
fi

echo ""
echo "🚀 Run it:"
echo "     waters-node                  # interactive mode"
echo "     waters-node --demo           # demo (5 devs + 5 chefs)"
echo "     waters-node --connect <ip>   # join network"
echo ""
echo "📖 More:"
echo "     https://kapelka.h2o-mining.space"
