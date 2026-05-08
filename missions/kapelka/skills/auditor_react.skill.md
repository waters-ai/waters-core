# SKILL: auditor-react v1.0

## Назначение

Аудит фронтенд-части сайта H2O: React 18+, Vite, Anchor Framework, Solana/web3. Обеспечивает проверку безопасности, производительности и качества кода.

## Входные данные

| Поле | Тип | Обязательное | Описание |
|------|-----|-------------|----------|
| target_url | string | да | URL сайта H2O для аудита |
| run_lighthouse | boolean | нет | Запускать ли Lighthouse CI (default: true) |
| run_npm_audit | boolean | нет | Анализ уязвимостей npm (default: true) |
| run_anchor_check | boolean | нет | Статический анализ Anchor-программ (default: true) |
| source_dir | string | нет | Путь к исходному коду (если доступен) |
| depth | string | нет | "quick" / "full" (default: "quick") |

## Выходные данные

| Поле | Тип | Описание |
|------|-----|----------|
| status | string | "success", "vulnerabilities_found", "error" |
| summary | object | Общая оценка (A-F) по категориям |
| critical_issues | array | Критические уязвимости (CVSS >= 9.0) |
| high_issues | array | Высокие уязвимости (CVSS 7.0-8.9) |
| recommendations | array | Рекомендации по исправлению |

## Категории аудита

### 1. Производительность (Vite + React)
- Lighthouse CI: FCP, LCP, TBT, CLS, SI
- Vite bundle analysis: source map check, code splitting audit
- React render performance: unnecessary re-renders, memoization
- Tree-shaking effectiveness, dead code elimination

### 2. Зависимости (npm)
- `npm audit` — известные уязвимости
- Snyk/ Dependabot config check
- Outdated major versions
- Supply chain risk (malicious packages)

### 3. Безопасность (CSP, XSS, CSRF)
- Content Security Policy headers (missing, misconfigured)
- XSS vulnerabilities in React (dangerouslySetInnerHTML, refs)
- CSRF token validation in API calls
- CORS misconfiguration

### 4. Anchor Framework (Solana)
- Program IDL validation
- Signer authority checks (missing checks in instruction handlers)
- Account data size validation, rent exemption
- Cross-Program Invocation (CPI) safety
- Owner check on deserialized accounts
- Integer overflow/underflow protection

### 5. Solana/web3
- Wallet adapter configuration
- RPC endpoint security (private vs public)
- Transaction signing flow audit
- PDA derivation correctness

## Алгоритм работы

1. Получить приказ из `orders.constructor.v1`
2. Загрузить исходный код (если доступен) или просканировать target_url
3. Запустить проверки по категориям
4. Критические уязвимости → `alerts.security.v1`
5. Сформировать отчёт → `planners.answers.v1`
6. Предложить новые скиллы (если нужно) → `skills.proposed.v1`

## Инструменты

- Lighthouse CI / PageSpeed Insights API
- npm audit / yarn audit
- Vite bundle visualizer (`rollup-plugin-visualizer`)
- Anchor CLI (`anchor test`, `anchor build`)
- Solana CLI (`solana-validator --audit`)
- Mythos Agent (сканирование CVE)
- Snyk CLI (`snyk test`)

## Зависимости

- Node.js 18+, npm/yarn
- Anchor Framework 0.29+
- Solana CLI 1.17+
- Lighthouse CI
- Доступ к исходному коду или production-URL

## Ограничения

- `depth: quick` — только поверхностный анализ (15 мин)
- `depth: full` — полный аудит (до 2 часов)
- Не анализирует серверную часть (это auditor-nginx)
- Для Anchor-анализа требуется исходный код программы
- Секреты и ключи — только через `secrets.*` и Хранителя
