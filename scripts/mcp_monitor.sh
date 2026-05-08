#!/usr/bin/env bash
# mcp_monitor.sh — Мониторинг MCP-серверов и баз памяти WATERS
set -euo pipefail

REPO_DIR="$(cd "$(dirname "$0")/.." && pwd)"
DOCKER="${DOCKER:-sudo docker}"
TIMESTAMP="$(date -u +%Y-%m-%dT%H:%M:%SZ)"
STATUS="ok"
FAILURES=""

echo "=== MCP Healthcheck $TIMESTAMP ==="

check_ok() {
    local name="$1"
    local desc="$2"
    if eval "$desc" 2>/dev/null; then
        echo "  [OK] $name"
    else
        echo "  [FAIL] $name"
        STATUS="degraded"
        FAILURES="${FAILURES}${name}: FAILED"$'\n'
    fi
}

check_ok "ChromaDB"    "curl -sf http://localhost:8000/api/v2/heartbeat > /dev/null"
check_ok "Redis"       "$DOCKER exec waters-redis redis-cli ping 2>&1 | grep -q PONG"
check_ok "Kafka"       "$DOCKER exec waters-kafka /opt/kafka/bin/kafka-topics.sh --bootstrap-server localhost:9092 --list > /dev/null 2>&1"
check_ok "LightRAG"    "test -d /data/lightrag"

# Memory Bridge
BRIDGE_TOOLS=$(echo '{"jsonrpc":"2.0","id":1,"method":"tools/list","params":{}}' | timeout 5 python3 "$REPO_DIR/scripts/mcp_memory_bridge.py" 2>/dev/null | python3 -c "import sys,json; print(len(json.loads(sys.stdin.read())['result']['tools']))" 2>/dev/null || echo "0")
if [ "$BRIDGE_TOOLS" -gt 0 ] 2>/dev/null; then
    echo "  [OK] Memory Bridge ($BRIDGE_TOOLS tools)"
else
    echo "  [FAIL] Memory Bridge"
    STATUS="degraded"
    FAILURES="${FAILURES}Memory Bridge: FAILED"$'\n'
fi

# Docker контейнеры
for c in waters-kafka waters-redis waters-chroma; do
    if $DOCKER ps --format '{{.Names}}' | grep -q "$c" 2>/dev/null; then
        echo "  [OK] Container $c"
    else
        echo "  [FAIL] Container $c"
        STATUS="degraded"
        FAILURES="${FAILURES}Container ${c}: not running"$'\n'
    fi
done

echo ""
echo "=== Overall Status: $STATUS ==="

# Alert mode
if [ "${1:-}" = "alert" ] && [ "$STATUS" != "ok" ]; then
    ALERT=$(cat <<JSON
{"msg_id":"monitor-$(date +%s)","ts":"$TIMESTAMP","type":"alert","from":"system.monitor","to":"alerts.security.v1","payload":{"kind":"mcp_health","status":"$STATUS","failures":"$(echo "$FAILURES" | tr '\n' ';')","host":"$(hostname)"}}
JSON
)
    echo "$ALERT" | $DOCKER exec -i waters-kafka /opt/kafka/bin/kafka-console-producer.sh --bootstrap-server localhost:9092 --topic alerts.security.v1 > /dev/null 2>&1 && echo "Alert sent" || echo "Alert FAILED"
fi

[ "${1:-}" = "status" ] && [ "$STATUS" = "ok" ] && exit 0 || [ "${1:-}" = "status" ] && exit 1
