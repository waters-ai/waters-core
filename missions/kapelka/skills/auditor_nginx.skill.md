# SKILL: auditor-nginx v1.0

## Назначение

Аудит инфраструктурной части сайта H2O: Nginx, CI/CD (GitHub Actions / GitLab CI), Docker. Обеспечивает проверку безопасности сервера, SSL/TLS, пайплайнов и контейнеризации.

## Входные данные

| Поле | Тип | Обязательное | Описание |
|------|-----|-------------|----------|
| target_url | string | да | URL сайта H2O для аудита |
| nginx_config_path | string | нет | Путь к конфигу Nginx (если доступен) |
| ci_config_path | string | нет | Путь к CI/CD конфигу (если доступен) |
| run_ssl_check | boolean | нет | Проверка SSL/TLS (default: true) |
| run_pipeline_audit | boolean | нет | Аудит CI/CD пайплайна (default: true) |
| depth | string | нет | "quick" / "full" (default: "quick") |

## Выходные данные

| Поле | Тип | Описание |
|------|-----|----------|
| status | string | "success", "vulnerabilities_found", "error" |
| summary | object | Общая оценка (A-F) по категориям |
| critical_issues | array | Критические уязвимости |
| high_issues | array | Высокие уязвимости |
| recommendations | array | Рекомендации по исправлению |

## Категории аудита

### 1. Nginx Configuration
- SSL/TLS: протоколы (TLS 1.2/1.3 only), ciphers (Mozilla Modern), HSTS
- Rate limiting: `limit_req_zone`, `limit_conn_zone`, burst config
- CORS headers: `Access-Control-Allow-Origin`, credentials
- Security headers: `X-Frame-Options`, `X-Content-Type-Options`, CSP
- Error pages: information disclosure (`server_tokens off`)
- Access control: IP whitelisting, basic auth, JWT validation
- Proxy config: `proxy_pass`, buffer overflow, timeouts
- Logging: access_log format, error_log level

### 2. SSL/TLS
- SSLyze / testssl.sh analysis: protocol support, cipher strength
- Certificate validity: expiry, chain completeness, SAN coverage
- OCSP stapling configuration
- HSTS preload readiness
- Key exchange (PFS support, DH parameter strength)

### 3. CI/CD Pipeline (GitHub Actions / GitLab CI)
- Secrets exposure: env vars in logs, hardcoded credentials
- Third-party action pinning (commit SHA vs version tags)
- Workflow permissions: minimum necessary (readonly where possible)
- Build integrity: artifact signing, checksum verification
- Approval gates: required reviews, environment protection rules
- OIDC trust configuration: `aws`, `gcp`, `azure` federation
- Self-hosted runner security: network isolation, cleanup

### 4. Docker (если применимо)
- Dockerfile best practices: multi-stage builds, non-root user
- Image vulnerability scanning (Trivy, Grype, Docker Scout)
- Supply chain: base image provenance, pinning digests
- Runtime security: `--read-only`, `--no-new-privileges`, seccomp
- Docker Compose: network isolation, exposed ports audit

## Алгоритм работы

1. Получить приказ из `orders.constructor.v1`
2. Загрузить конфиги (если доступны) или просканировать target_url
3. Запустить внешние проверки (SSLyze, testssl) из DMZ-зоны
4. Критические уязвимости → `alerts.security.v1`
5. Сформировать отчёт → `planners.answers.v1`
6. Предложить новые скиллы (если нужно) → `skills.proposed.v1`

## Инструменты

- SSLyze (`sslyze --regular target_url:443`)
- testssl.sh (`testssl.sh --quiet target_url:443`)
- Mozilla Observatory API
- actionlint (проверка GitHub Actions workflows)
- Trivy (`trivy image`, `trivy config`)
- Hadolint (Dockerfile linting)
- Nginx -t (синтаксическая проверка конфига)
- `nginx -T` (дампинг полной конфигурации)

## Зависимости

- Доступ к target_url через DMZ (mcp-proxy)
- python3, sslyze, actionlint, trivy, hadolint
- Nginx (для синтаксической проверки)
- Docker (для сканирования образов)

## Ограничения

- `depth: quick` — только внешние проверки (SSL, заголовки) — 10 мин
- `depth: full` — полный аудит с конфигами — до 1 часа
- Без доступа к конфигам Nginx — аудит ограничен внешними сканерами
- Без доступа к CI/CD конфигу — аудит пайплайна невозможен
- Все внешние сканы — только через DMZ-зону (mcp-proxy)
- Секреты CI/CD — только через `secrets.*` и Хранителя
