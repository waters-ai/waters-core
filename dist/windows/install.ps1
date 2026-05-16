Write-Host "🌊 waters-node v0.3.0 — Windows install" -ForegroundColor Cyan
Write-Host ""

$Bin = "waters-node.exe"
$InstallDir = "$env:USERPROFILE\.local\bin"

if (-not (Test-Path $InstallDir)) {
    New-Item -ItemType Directory -Path $InstallDir -Force | Out-Null
}

Copy-Item $Bin -Destination "$InstallDir\$Bin" -Force
Copy-Item "config.toml" -Destination ".\config.toml" -Force

Write-Host "✅ waters-node установлен в $InstallDir\$Bin" -ForegroundColor Green
Write-Host ""
Write-Host "🚀 Быстрый старт:" -ForegroundColor Yellow
Write-Host "  waters-node.exe                         # интерактивный режим"
Write-Host "  waters-node.exe --connect <ip:port>     # подключиться к ноде"
Write-Host ""
Write-Host "📋 Команды в чате:" -ForegroundColor Yellow
Write-Host "  chat создай группу <имя>            # создать группу"
Write-Host "  chat создай задачу <описание>       # создать задачу"
Write-Host "  режим план|выполнение|стоп          # режим задачи"
Write-Host "  статус                              # состояние ноды"
Write-Host "  connect <ip:port>                   # подключить пира"
Write-Host ""
Write-Host "🔗 После запуска нода слушает порт 42069"
Write-Host "   Dashboard: http://localhost:42069"
