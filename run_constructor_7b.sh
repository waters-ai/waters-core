#!/usr/bin/env bash
# Запуск Конструктора Сети v1.0 на qwen2.5:7b (CPU)
# Гарантирует модель 7B независимо от opencode.json
# Использование:
#   ./run_constructor_7b.sh            # обычный запуск
#   ./run_constructor_7b.sh --tmux     # в tmux-сессии
set -euo pipefail

cd "$(dirname "$0")"

echo "🌐 Конструктор Сети v1.0 — принудительно qwen2.5:7b"
echo ""

# Принудительно ставим 7B в конфиге
python3 -c "
import json
with open('opencode.json') as f:
    cfg = json.load(f)
cfg['model'] = 'ollama/qwen2.5:7b'
with open('opencode.json', 'w') as f:
    json.dump(cfg, f, indent=2, ensure_ascii=False)
print('   ✅ Модель: qwen2.5:7b')
"

exec ./run_constructor.sh "$@"
