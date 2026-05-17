#!/bin/bash
set -euo pipefail

# First-run wizard — если нет .waters/config
if [ ! -f /root/waters-node/.waters/done ]; then
    echo "🌊 WATERS Node v0.5.0 — First Run Setup"
    echo ""
    mkdir -p /root/waters-node
    
    # Load env
    [ -f /etc/waters-node/env ] && source /etc/waters-node/env
    
    # Node name
    NODE_NAME="${NODE_NAME:-$(hostname)}"
    
    # Connect to peer
    PEER="${CONNECT:-}"
    
    echo "🚀 Starting waters-node..."
    echo "   Name: $NODE_NAME"
    echo "   Redis: redis://127.0.0.1:6379"
    [ -n "$PEER" ] && echo "   Connect: $PEER"
    
    mkdir -p /root/waters-node/.waters
    date > /root/waters-node/.waters/done
fi

# Start Redis
redis-server --daemonize yes

# Start node
exec waters-node --port 42069 ${CONNECT:+--connect $CONNECT}
