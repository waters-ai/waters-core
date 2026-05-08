#!/bin/bash
# deploy.sh — универсальный скрипт деплоя SPA-сайта на Reg.ru VPS
# Использование:
#   ./deploy.sh                          # Быстрый деплой (ssh + rsync)
#   ./deploy.sh --full                   # Полный деплой (установка ПО + настройка)
#   ./deploy.sh --env                    # Загрузить .env.production на сервер
#   ./deploy.sh --rollback               # Откат к предыдущей версии
#
# Зависимости: ssh, rsync, curl
# Переменные окружения:
#   DEPLOY_HOST      — IP или домен сервера
#   DEPLOY_USER      — SSH-пользователь
#   DEPLOY_PATH      — путь на сервере (/var/www/<project>)
#   DEPLOY_SSH_KEY   — путь к SSH-ключу

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_DIR="$(dirname "$SCRIPT_DIR")"

# Defaults
: "${DEPLOY_HOST:?DEPLOY_HOST not set}"
: "${DEPLOY_USER:?DEPLOY_USER not set}"
: "${DEPLOY_PATH:=/var/www/project}"
: "${DEPLOY_SSH_KEY:=${HOME}/.ssh/id_rsa}"
: "${BUILD_DIR:=dist}"
: "${RELEASES_DIR:=releases}"
: "${KEEP_RELEASES:=5}"

SSH_CMD="ssh -i ${DEPLOY_SSH_KEY} -o StrictHostKeyChecking=no"
RSYNC_CMD="rsync -avz --delete -e 'ssh -i ${DEPLOY_SSH_KEY} -o StrictHostKeyChecking=no'"

log() { echo "[deploy] $(date '+%H:%M:%S') $*"; }

full_deploy() {
    log "Полный деплой: установка ПО и настройка сервера..."

    # Установка базового ПО
    ${SSH_CMD} "${DEPLOY_USER}@${DEPLOY_HOST}" << 'REMOTE'
        set -e
        echo "[server] Обновление системы..."
        sudo apt update && sudo apt upgrade -y

        echo "[server] Установка Nginx, Node.js, certbot..."
        sudo apt install -y nginx certbot python3-certbot-nginx curl git ufw

        echo "[server] Установка Node.js 20 LTS..."
        curl -fsSL https://deb.nodesource.com/setup_20.x | sudo bash -
        sudo apt install -y nodejs

        echo "[server] Firewall..."
        sudo ufw allow 22/tcp
        sudo ufw allow 80/tcp
        sudo ufw allow 443/tcp
        sudo ufw --force enable

        echo "[server] Создание директории проекта..."
        sudo mkdir -p ${DEPLOY_PATH}/{dist,${RELEASES_DIR},logs}
        sudo chown -R "${USER}:${USER}" "${DEPLOY_PATH}"

        echo "[server] Базовая настройка Nginx..."
        sudo ln -sf "${DEPLOY_PATH}/nginx.conf" "/etc/nginx/sites-available/${PROJECT_NAME:-project}"
        sudo ln -sf "/etc/nginx/sites-available/${PROJECT_NAME:-project}" "/etc/nginx/sites-enabled/"
        sudo nginx -t && sudo systemctl reload nginx

        echo "[server] Готово!"
REMOTE
    log "Полный деплой завершён"
}

quick_deploy() {
    local release_dir="${DEPLOY_PATH}/${RELEASES_DIR}/$(date +%Y%m%d_%H%M%S)"

    log "Быстрый деплой: сборка и отправка на сервер..."

    # 1. Сборка локально
    if [ -f package.json ]; then
        log "Установка зависимостей..."
        npm ci --omit=dev
        log "Сборка проекта..."
        npm run build
    else
        log "package.json не найден — пропускаем сборку"
    fi

    # 2. Создание директории релиза на сервере
    ${SSH_CMD} "${DEPLOY_USER}@${DEPLOY_HOST}" "mkdir -p ${release_dir}"

    # 3. Копирование собранных файлов
    log "Копирование файлов на сервер..."
    eval "${RSYNC_CMD}" "${BUILD_DIR}/" "${DEPLOY_USER}@${DEPLOY_HOST}:${release_dir}/"

    # 4. Обновление symlink
    ${SSH_CMD} "${DEPLOY_USER}@${DEPLOY_HOST}" "
        ln -sfn ${release_dir} ${DEPLOY_PATH}/current
        # Очистка старых релизов
        cd ${DEPLOY_PATH}/${RELEASES_DIR}
        ls -1t | tail -n +$((KEEP_RELEASES + 1)) | xargs -r rm -rf
    "

    log "Готово: ${release_dir}"
}

upload_env() {
    if [ ! -f .env.production ]; then
        log ".env.production не найден. Создаю шаблон..."
        cat > .env.production << 'EOF'
# Production environment
VITE_API_URL=https://api.example.com
VITE_SOLANA_RPC=https://api.mainnet-beta.solana.com
VITE_SOLANA_NETWORK=mainnet-beta
EOF
        log "Шаблон .env.production создан. Заполни и запусти снова."
        exit 1
    fi

    log "Загрузка .env.production на сервер..."
    ${SSH_CMD} "${DEPLOY_USER}@${DEPLOY_HOST}" "mkdir -p ${DEPLOY_PATH}"
    scp -i "${DEPLOY_SSH_KEY}" .env.production "${DEPLOY_USER}@${DEPLOY_HOST}:${DEPLOY_PATH}/.env.production"
    log "Готово"
}

rollback() {
    log "Откат к предыдущему релизу..."

    local releases
    releases=$(${SSH_CMD} "${DEPLOY_USER}@${DEPLOY_HOST}" "ls -1t ${DEPLOY_PATH}/${RELEASES_DIR}" 2>/dev/null || true)

    if [ -z "${releases}" ]; then
        log "Нет релизов для отката"
        exit 1
    fi

    local current
    current=$(${SSH_CMD} "${DEPLOY_USER}@${DEPLOY_HOST}" "readlink ${DEPLOY_PATH}/current" 2>/dev/null || echo "")

    local previous
    previous=$(echo "${releases}" | head -2 | tail -1)

    if [ -z "${previous}" ]; then
        log "Только один релиз — откат невозможен"
        exit 1
    fi

    log "Текущий: ${current}"
    log "Откат к: ${previous}"

    ${SSH_CMD} "${DEPLOY_USER}@${DEPLOY_HOST}" "ln -sfn ${DEPLOY_PATH}/${RELEASES_DIR}/${previous} ${DEPLOY_PATH}/current"

    log "Откат выполнен. Если нужно вернуться — запусти deploy.sh снова."
}

setup_ssl() {
    local domain="${DEPLOY_HOST}"

    log "Настройка SSL для ${domain}..."
    ${SSH_CMD} "${DEPLOY_USER}@${DEPLOY_HOST}" "
        sudo certbot --nginx -d ${domain} -d www.${domain} --non-interactive --agree-tos --email admin@${domain} || true
    "
    log "SSL настроен. Проверь автообновление: sudo certbot renew --dry-run"
}

check_health() {
    local domain="${DEPLOY_HOST}"

    log "Проверка здоровья сайта ${domain}..."
    if curl -sf -o /dev/null -w "%{http_code}" "https://${domain}/"; then
        log "✅ HTTPS работает (HTTP 200)"
    else
        log "❌ HTTPS не отвечает"
    fi
}

# --- main ---
case "${1:-quick}" in
    --full)     full_deploy ;;
    --env)      upload_env ;;
    --rollback) rollback ;;
    --ssl)      setup_ssl ;;
    --check)    check_health ;;
    *)          quick_deploy ;;
esac
