#!/bin/bash
set -euo pipefail

HOST="${1:-171.22.180.177}"
USER="${2:-ubuntu}"

echo "🌊 WATERS Node v0.5.0-alpha — deploy to $USER@$HOST"
ssh -o StrictHostKeyChecking=no "$USER@$HOST" \
  -R 11434:localhost:11434 \
  bash -s << 'REMOTE'
set -euo pipefail
cd ~
mkdir -p waters-node
cd waters-node

echo "📥 Downloading v0.5.0-alpha..."
wget -q https://github.com/waters-ai/waters-core/releases/download/v0.5.0-alpha/waters-node -O waters-node
chmod +x waters-node

echo "⚙️ Setting up systemd..."
cat > /tmp/waters-node.service << 'SERVICE'
[Unit]
Description=WATERS Node v0.5.0-alpha
After=network.target redis.service
Wants=redis.service

[Service]
Type=simple
User=ubuntu
WorkingDirectory=/home/ubuntu/waters-node
EnvironmentFile=-/home/ubuntu/waters-node/.env
ExecStart=/home/ubuntu/waters-node/waters-node --port 42069
Restart=always
RestartSec=10
LimitNOFILE=65536

[Install]
WantedBy=multi-user.target
SERVICE

sudo mv /tmp/waters-node.service /etc/systemd/system/waters-node.service
sudo systemctl daemon-reload
echo "✅ Done. Start: sudo systemctl start waters-node"
echo "   Logs: journalctl -u waters-node -f"
REMOTE
