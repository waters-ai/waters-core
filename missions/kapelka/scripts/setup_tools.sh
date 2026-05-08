#!/bin/bash
# setup_tools.sh — установка инструментов аудита Kapelka
# Запускать на сервере, где будут работать агенты аудита

set -euo pipefail

log() { echo "[setup] $*"; }

log "Установка инструментов аудита Kapelka..."

# === Базовое ПО ===
log "Установка системных пакетов..."
sudo apt update && sudo apt install -y \
    curl wget git jq \
    openssl ca-certificates \
    python3 python3-pip

# === Node.js ===
if ! command -v node &>/dev/null; then
    log "Установка Node.js 20 LTS..."
    curl -fsSL https://deb.nodesource.com/setup_20.x | sudo bash -
    sudo apt install -y nodejs
fi
log "Node.js: $(node --version)"
log "npm: $(npm --version)"

# === Lighthouse CI ===
if ! command -v lighthouse &>/dev/null; then
    log "Установка Lighthouse..."
    sudo npm install -g @lhci/cli
fi

# === npm audit ===
# уже встроен в npm, дополнительно Snyk
if ! command -v snyk &>/dev/null; then
    log "Установка Snyk..."
    sudo npm install -g snyk
fi

# === testssl.sh ===
if [ ! -f ~/testssl.sh/testssl.sh ]; then
    log "Установка testssl.sh..."
    git clone --depth 1 https://github.com/drwetter/testssl.sh.git ~/testssl.sh
fi

# === OWASP ZAP (Docker) ===
log "Проверка Docker..."
if ! command -v docker &>/dev/null; then
    log "Docker не найден. Для OWASP ZAP требуется Docker."
    log "Установка Docker..."
    curl -fsSL https://get.docker.com | sudo bash
    sudo usermod -aG docker "${USER}"
fi
log "Docker: $(docker --version 2>/dev/null || echo 'требуется перезапуск сессии')"

# === truffleHog ===
if ! command -v trufflehog &>/dev/null; then
    log "Установка truffleHog..."
    pip3 install truffleHog
fi

# === gitleaks ===
if ! command -v gitleaks &>/dev/null; then
    log "Установка gitleaks..."
    wget -O /tmp/gitleaks.tar.gz https://github.com/gitleaks/gitleaks/releases/latest/download/gitleaks-linux-amd64.tar.gz
    sudo tar -xzf /tmp/gitleaks.tar.gz -C /usr/local/bin gitleaks
fi

# === SSLyze ===
if ! command -v sslyze &>/dev/null; then
    log "Установка SSLyze..."
    pip3 install sslyze
fi

# === Trivy ===
if ! command -v trivy &>/dev/null; then
    log "Установка Trivy..."
    wget -O /tmp/trivy.deb https://github.com/aquasecurity/trivy/releases/latest/download/trivy_0.49.1_Linux-64bit.deb
    sudo dpkg -i /tmp/trivy.deb || sudo apt install -f -y
fi

# === Hadolint ===
if ! command -v hadolint &>/dev/null; then
    log "Установка Hadolint..."
    wget -O /usr/local/bin/hadolint https://github.com/hadolint/hadolint/releases/latest/download/hadolint-Linux-x86_64
    sudo chmod +x /usr/local/bin/hadolint
fi

log ""
log "=== Установка завершена ==="
log "Проверь: node, npm, lighthouse, snyk, trufflehog, gitleaks, sslyze, trivy, hadolint"
log "Для OWASP ZAP: docker pull ghcr.io/zaproxy/zaproxy:stable"
