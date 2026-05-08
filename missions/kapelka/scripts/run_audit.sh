#!/bin/bash
# run_audit.sh — быстрый запуск полного цикла аудита
# Использование:
#   ./run_audit.sh <target_url> [quick|full]
#   ./run_audit.sh https://example.com quick
#   ./run_audit.sh https://example.com full

set -euo pipefail

TARGET="${1:?Usage: run_audit.sh <target_url> [quick|full]}"
DEPTH="${2:-quick}"
REPORT_DIR="./reports/$(echo "$TARGET" | sed 's|https\?://||' | tr '/' '_')/$(date +%Y%m%d_%H%M%S)"

mkdir -p "${REPORT_DIR}"
log() { echo "[audit] $*"; }

log "=== Аудит: ${TARGET} (${DEPTH}) ==="
log "Отчёты: ${REPORT_DIR}"

# === 1. Проверка доступности ===
log "[1/8] Проверка доступности..."
if curl -sf -o /dev/null --max-time 10 "${TARGET}"; then
    log "✅ Сайт доступен"
else
    log "❌ Сайт не доступен"
    exit 1
fi

# === 2. Security Headers ===
log "[2/8] Security Headers..."
{
    echo "=== Security Headers: ${TARGET} ==="
    curl -sI "${TARGET}" | grep -i -E "^(strict-transport-security|content-security-policy|x-frame-options|x-content-type-options|referrer-policy|permissions-policy|cors|cross-origin)" || echo "Нет security headers"
} > "${REPORT_DIR}/security_headers.txt"
log "Готово"

# === 3. SSL/TLS ===
log "[3/8] SSL/TLS..."
if command -v ~/testssl.sh/testssl.sh &>/dev/null; then
    ~/testssl.sh/testssl.sh --quiet --htmlfile "${REPORT_DIR}/ssl_report.html" "${TARGET}" 2>/dev/null || true
else
    echo "testssl.sh не установлен. Пропускаем." > "${REPORT_DIR}/ssl_report.txt"
fi
log "Готово"

# === 4. npm audit (если есть package.json) ===
log "[4/8] npm audit..."
if [ -f package.json ]; then
    npm audit --json > "${REPORT_DIR}/npm_audit.json" 2>/dev/null || true
    log "Готово"
else
    log "package.json не найден — пропускаем"
fi

# === 5. Lighthouse ===
log "[5/8] Lighthouse CI..."
if command -v lighthouse &>/dev/null; then
    lighthouse "${TARGET}" --quiet --chrome-flags="--headless --no-sandbox" --output=html --output=json --output-path="${REPORT_DIR}/lighthouse" 2>/dev/null || true
    log "Готово"
else
    log "Lighthouse не установлен — пропускаем"
fi

# === 6. Secret Scanner (если есть git-репозиторий) ===
log "[6/8] Secret scan..."
if command -v gitleaks &>/dev/null && [ -d .git ]; then
    gitleaks detect --report-format json --report-path "${REPORT_DIR}/secrets.json" -v 2>/dev/null || true
    log "Готово"
elif command -v trufflehog &>/dev/null && [ -d .git ]; then
    trufflehog --json . > "${REPORT_DIR}/secrets.json" 2>/dev/null || true
    log "Готово"
else
    log "gitleaks/trufflehog не установлены или нет .git — пропускаем"
fi

# === 7. OWASP ZAP (только full) ===
if [ "${DEPTH}" = "full" ]; then
    log "[7/8] OWASP ZAP (полный)..."
    if command -v docker &>/dev/null; then
        sudo docker run --rm -v "${REPORT_DIR}:/zap/wrk/:rw" \
            ghcr.io/zaproxy/zaproxy:stable \
            zap-full-scan.py -t "${TARGET}" -r zap_report.html \
            2>/dev/null || true
        log "Готово"
    else
        log "Docker не доступен — пропускаем ZAP"
    fi
else
    log "[7/8] OWASP ZAP — пропущен (только для full)"
fi

# === 8. Сводка ===
log "[8/8] Формирование сводки..."
{
    echo "# Результаты аудита: ${TARGET}"
    echo "Дата: $(date)"
    echo "Глубина: ${DEPTH}"
    echo ""
    echo "## Файлы отчёта:"
    ls -la "${REPORT_DIR}/"
} > "${REPORT_DIR}/summary.md"

log ""
log "=== Аудит завершён ==="
log "Отчёты в: ${REPORT_DIR}"
log "Сводка: ${REPORT_DIR}/summary.md"
