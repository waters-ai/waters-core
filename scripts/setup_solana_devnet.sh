#!/bin/bash
# setup_solana_devnet.sh — создание devnet-окружения для Kapelka
# Использование: sudo bash scripts/setup_solana_devnet.sh

set -euo pipefail
log() { echo "[$(date '+%H:%M:%S')] $*"; }

KAPELKA_WALLET="/tmp/kapelka-wallet.json"
ANCHOR_VERSION="1.0.2"

log "=== Установка Anchor $ANCHOR_VERSION ==="
if ! command -v avm &>/dev/null; then
    curl -fsSL https://bin.anchor-lang.com/install.sh | bash
    export PATH="$HOME/.avm/bin:$PATH"
fi

avm install "$ANCHOR_VERSION"
avm use "$ANCHOR_VERSION"

log "=== Создание кошелька Kapelka ==="
if [ -f "$KAPELKA_WALLET" ]; then
    log "Кошелёк уже существует: $KAPELKA_WALLET"
else
    anchor-keygen --outfile "$KAPELKA_WALLET" --force
fi
K_ADDR=$(solana-keygen pubkey "$KAPELKA_WALLET")
log "Адрес Kapelka: $K_ADDR"

log "=== Airdrop SOL на devnet ==="
solana airdrop 5 "$K_ADDR" --url devnet || log "Airdrop может не сработать — попробуй позже"
solana airdrop 5 "$K_ADDR" --url devnet || true

log "=== Создание файла окружения ==="
MISSIONS_DIR="/home/ubuntu/WATERS/repos/waters-core/missions/kapelka/clients/_sandbox"
mkdir -p "$MISSIONS_DIR"
cat > "$MISSIONS_DIR/.env" <<EOF
SOLANA_NETWORK=devnet
KAPELKA_WALLET=$K_ADDR
TOKEN_2022_PROGRAM=TokenzQdBNbLqP5VEhdkAS6EPFLC1PHnBqCXEpPxuEb
RPC_ENDPOINT=https://api.devnet.solana.com
ANCHOR_VERSION=$ANCHOR_VERSION
EOF

log "=== Готово ==="
log "Адрес кошелька Kapelka: $K_ADDR"
log "   https://explorer.solana.com/address/$K_ADDR?cluster=devnet"
log "Файл: $MISSIONS_DIR/.env"
