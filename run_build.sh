#!/usr/bin/env bash
# Запуск Строительной сборки: @constructor (14b) + @architect @integrator (7b)
set -euo pipefail
REPO_DIR="$(cd "$(dirname "$0")" && pwd)"

if [ -f "$REPO_DIR/.env" ]; then
  set -a; source "$REPO_DIR/.env"; set +a
fi

fuser 11434/tcp 2>/dev/null || ssh -fNL 11434:localhost:11434 ubuntu@171.22.180.237
fuser 11435/tcp 2>/dev/null || ssh -fNL 11435:localhost:11435 ubuntu@171.22.180.237
fuser 17687/tcp 2>/dev/null || ssh -fNL 17687:localhost:7687 ubuntu@171.22.180.237

echo "🌐 Строительная сборка — @constructor (14b) лидирует"
cp "$REPO_DIR/agents/constructor_AGENTS.md" "$REPO_DIR/AGENTS.md"
opencode -c "$REPO_DIR/opencode-build.json"
