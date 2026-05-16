#!/bin/bash
set -euo pipefail

echo "🌊 waters-node v0.3.0 — Linux install"
echo ""

BIN="waters-node"
CONFIG="config.toml"
INSTALL_DIR="${HOME}/.local/bin"

mkdir -p "${INSTALL_DIR}"
cp "${BIN}" "${INSTALL_DIR}/"
cp "${CONFIG}" .
chmod +x "${INSTALL_DIR}/${BIN}"

echo "✅ waters-node установлен в ${INSTALL_DIR}/${BIN}"
echo ""
echo "🚀 Быстрый старт:"
echo "  waters-node                         # интерактивный режим"
echo "  waters-node --connect <ip:port>     # подключиться к ноде"
echo "  waters-node --demo                  # демо-режим"
echo ""
echo "📋 Команды в чате:"
echo "  chat создай группу <имя>            # создать группу"
echo "  chat создай задачу <описание>       # создать задачу"
echo "  режим план|выполнение|стоп          # режим задачи"
echo "  статус                              # состояние ноды"
echo "  connect <ip:port>                   # подключить пира"
echo ""
echo "🔗 После запуска нода слушает порт 42069"
echo "   Dashboard: http://localhost:42069"
