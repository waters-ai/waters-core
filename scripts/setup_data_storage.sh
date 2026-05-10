#!/bin/bash
set -euo pipefail

# ============================================================
# setup_data_storage.sh — создать структуру папок на 167 и 238
# ============================================================

SERVERS=(
  "167:ubuntu@171.22.180.167"
  "238:ubuntu@171.22.180.238"
)

RAW_BASE="/home/waters-data/raw"
AGENTS_DIR="/home/ubuntu/WATERS/repos/waters-core/agents"
SUBDIRS=("architect" "constructor" "integrator" "director" "keeper" "lawkeeper" "scout")

log() { echo "[$(date '+%H:%M:%S')] $*"; }

setup_167() {
  log "=== Настройка 167 ==="
  for agent in "${SUBDIRS[@]}"; do
    mkdir -p "${RAW_BASE}/${agent}"
    log "  ${RAW_BASE}/${agent}"
  done
  mkdir -p /home/waters-data/logs
  mkdir -p /home/waters-data/secrets
  log "  /home/waters-data/logs"
  log "  /home/waters-data/secrets"
}

setup_238() {
  log "=== Настройка 238 ==="
  for agent in "${SUBDIRS[@]}"; do
    mkdir -p "${AGENTS_DIR}/${agent}/data"
    log "  ${AGENTS_DIR}/${agent}/data"
  done
  mkdir -p /home/ubuntu/WATERS/repos/waters-core/logs
}

case "${1:-all}" in
  167)
    setup_167
    ;;
  238)
    setup_238
    ;;
  all)
    setup_167
    setup_238
    ;;
  *)
    echo "Использование: $0 [167|238|all]"
    exit 1
    ;;
esac

log "Готово."
