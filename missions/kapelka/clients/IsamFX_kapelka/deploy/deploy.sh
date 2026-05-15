#!/usr/bin/env bash
set -euo pipefail

# ═══════════════════════════════════════════════════════════════════
# Kapelka — Deploy Script
# ═══════════════════════════════════════════════════════════════════
# Обязательные переменные окружения:
#   REMOTE_HOST      — IP или домен сервера
#   REMOTE_USER      — пользователь SSH
#   REMOTE_PATH      — целевая директория на сервере (/var/www/kapelka)
#   SSH_KEY_PATH     — путь к приватному ключу (~/.ssh/id_ed25519)
# ═══════════════════════════════════════════════════════════════════

# ── Defaults ───────────────────────────────────────────────────────
REMOTE_HOST="${REMOTE_HOST:?REMOTE_HOST not set}"
REMOTE_USER="${REMOTE_USER:?REMOTE_USER not set}"
REMOTE_PATH="${REMOTE_PATH:?REMOTE_PATH not set}"
SSH_KEY_PATH="${SSH_KEY_PATH:-$HOME/.ssh/id_ed25519}"
SSH_CMD="ssh -i $SSH_KEY_PATH -o StrictHostKeyChecking=no"
SCP_CMD="scp -i $SSH_KEY_PATH -o StrictHostKeyChecking=no"
RSYNC_CMD="rsync -avz --delete --progress -e 'ssh -i $SSH_KEY_PATH -o StrictHostKeyChecking=no'"
BUILD_DIR="dist"
REMOTE_TMP="$REMOTE_PATH/dist"

# ── Colors ─────────────────────────────────────────────────────────
RED='\033[0;31m'; GREEN='\033[0;32m'; YELLOW='\033[1;33m'; NC='\033[0m'
info()  { echo -e "${GREEN}[INFO]${NC}  $*"; }
warn()  { echo -e "${YELLOW}[WARN]${NC}  $*"; }
error() { echo -e "${RED}[ERROR]${NC} $*"; exit 1; }

# ── Flags ──────────────────────────────────────────────────────────
FLAG_FULL=false
FLAG_SSL=false
FLAG_CHECK=false

while [[ $# -gt 0 ]]; do
    case "$1" in
        --full)  FLAG_FULL=true  ; shift ;;
        --ssl)   FLAG_SSL=true   ; shift ;;
        --check) FLAG_CHECK=true ; shift ;;
        --help)
            echo "Usage: $0 [--full] [--ssl] [--check]"
            echo ""
            echo "  --full   Install system dependencies (Docker, Node.js, Nginx, certbot)"
            echo "  --ssl    Obtain & install Let's Encrypt certificate via certbot"
            echo "  --check  Run healthcheck after deploy"
            exit 0
            ;;
        *) error "Unknown option: $1. Use --help for usage." ;;
    esac
done

# ── 1. System dependencies (--full) ───────────────────────────────
if $FLAG_FULL; then
    info "Installing system dependencies on $REMOTE_HOST..."

    $SSH_CMD "$REMOTE_USER@$REMOTE_HOST" bash -s <<'REMOTE'
        set -euo pipefail
        export DEBIAN_FRONTEND=noninteractive

        # Docker
        if ! command -v docker &>/dev/null; then
            curl -fsSL https://get.docker.com | sh
            sudo usermod -aG docker "$USER"
        fi

        # Docker Compose plugin
        if ! docker compose version &>/dev/null; then
            sudo apt-get update && sudo apt-get install -y docker-compose-plugin
        fi

        # Nginx
        if ! command -v nginx &>/dev/null; then
            sudo apt-get install -y nginx
            sudo systemctl enable nginx
        fi

        # Certbot
        if ! command -v certbot &>/dev/null; then
            sudo apt-get install -y certbot python3-certbot-nginx
        fi

        echo "Dependencies OK"
REMOTE

    info "Dependencies installed."
fi

# ── 2. Local build ─────────────────────────────────────────────────
info "Building project..."
npm ci --omit=dev --ignore-scripts
npm run build

if [ ! -d "$BUILD_DIR" ]; then
    error "Build directory '$BUILD_DIR' not found. Build failed."
fi
info "Build complete."

# ── 3. Rsync to server ────────────────────────────────────────────
info "Syncing dist/ to $REMOTE_USER@$REMOTE_HOST:$REMOTE_PATH ..."
$SSH_CMD "$REMOTE_USER@$REMOTE_HOST" "mkdir -p $REMOTE_PATH"
# shellcheck disable=SC2086
eval $RSYNC_CMD "$BUILD_DIR/" "$REMOTE_USER@$REMOTE_HOST:$REMOTE_TMP"
info "Sync complete."

# ── 4. Deploy via Docker (or fallback to bare nginx) ──────────────
info "Restarting application..."

$SSH_CMD "$REMOTE_USER@$REMOTE_HOST" bash -s <<REMOTE
    set -euo pipefail

    if command -v docker &>/dev/null; then
        cd "$REMOTE_PATH"
        cp "$REMOTE_TMP"/* . -r 2>/dev/null || true
        rm -rf "$REMOTE_TMP"

        if [ -f docker-compose.yml ]; then
            docker compose pull
            docker compose up -d --build
        elif [ -f Dockerfile ]; then
            docker build -t kapelka:latest .
            docker stop kapelka 2>/dev/null || true
            docker rm kapelka 2>/dev/null || true
            docker run -d --name kapelka -p 80:80 --restart unless-stopped kapelka:latest
        fi

        docker image prune -f
    else
        sudo rsync -avz --delete "$REMOTE_TMP/" /var/www/kapelka/
        sudo systemctl reload nginx || sudo nginx -s reload
    fi
REMOTE

info "Application deployed."

# ── 5. SSL via certbot (--ssl) ────────────────────────────────────
if $FLAG_SSL; then
    info "Obtaining Let's Encrypt certificate..."

    $SSH_CMD "$REMOTE_USER@$REMOTE_HOST" bash -s <<REMOTE
        set -euo pipefail
        DOMAIN="\$(echo "$REMOTE_HOST" | grep -E '^[a-zA-Z0-9.-]+\.[a-zA-Z]{2,}$' || true)"
        if [ -n "\$DOMAIN" ]; then
            sudo certbot --nginx -d "\$DOMAIN" --non-interactive --agree-tos --email admin@"\$DOMAIN"
        else
            echo "REMOTE_HOST is an IP, not a domain. Skipping certbot."
        fi
REMOTE

    info "SSL certificate installed."
fi

# ── 6. Healthcheck (--check) ──────────────────────────────────────
if $FLAG_CHECK; then
    info "Running healthcheck..."

    sleep 5

    HTTP_CODE=$($SSH_CMD "$REMOTE_USER@$REMOTE_HOST" \
        "curl -s -o /dev/null -w '%{http_code}' http://localhost/ 2>/dev/null || echo '000'")

    if [ "$HTTP_CODE" = "200" ] || [ "$HTTP_CODE" = "301" ] || [ "$HTTP_CODE" = "302" ]; then
        info "Healthcheck passed (HTTP $HTTP_CODE)."
    else
        error "Healthcheck failed (HTTP $HTTP_CODE). Check server logs."
    fi
fi

info "Deploy completed successfully."
