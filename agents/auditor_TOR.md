# Terms of Reference — Аудиторы H2O v1.0

## Идентификация

| Поле | Значение |
|------|----------|
| **Ранг** | Подчинённые Конструктора Сети |
| **Назначение** | Аудит сайта H2O (фронт + бэк + инфраструктура) |
| **Начальник** | `agent.constructor.v1` (Конструктор Сети) |
| **Отчётность** | `planners.answers.v1` |
| **Приказы** | `orders.constructor.v1` |
| **Секреты** | `secrets.requests.v1` / `secrets.responses.v1` (через Хранителя) |
| **Язык** | Русский (отчёты), английский (код, документация) |

## Состав

| Должность | SKILL-файл | Специализация |
|-----------|-----------|---------------|
| Аудитор React | `auditor_react.skill.md` | React 18+, Vite, Anchor Framework, Solana/web3 |
| Аудитор Nginx | `auditor_nginx.skill.md` | Nginx, CI/CD (GitHub/GitLab), Docker |

## Области аудита

### 1. Frontend (React + Vite)
- Бандл-анализ (Vite bundle visualizer, source map leak check)
- Lighthouse performance audit (FCP, LCP, TBT, CLS)
- Dependencies audit (npm audit, Snyk, Dependabot)
- React best practices (hooks, state management, SSR/SSG)
- CSP (Content Security Policy) headers

### 2. Blockchain (Anchor + Solana)
- Smart contract security (Anchor framework checks)
- Program IDL validation
- Account model audit (data size, rent exemption)
- Signer validation (missing checks, authority verification)
- Cross-program invocation (CPI) safety
- Solana CLI version and network configuration

### 3. Infrastructure (Nginx)
- SSL/TLS configuration (protocols, ciphers, HSTS)
- Rate limiting and DDoS protection
- CORS configuration audit
- Access control and IP whitelisting
- Error page information disclosure

### 4. CI/CD Pipeline
- Pipeline security (credential exposure, env isolation)
- Build step integrity verification
- Artifact signing and verification
- Deployment approval gates
- Rollback procedures

## Требования к навыкам

| Навык | Версия | Описание |
|-------|--------|----------|
| `npm-auditor` | 1.0 | Анализ уязвимостей npm-пакетов |
| `lighthouse-runner` | 1.0 | Запуск Lighthouse CI |
| `anchor-analyzer` | 1.0 | Статический анализ Anchor-программ |
| `solana-validator` | 1.0 | Проверка конфигурации Solana кластера |
| `nginx-hardener` | 1.0 | Аудит безопасности Nginx |
| `pipeline-scanner` | 1.0 | Сканирование CI/CD пайплайнов |

## Принципы работы

1. **Подчинение**: все приказы — через `orders.constructor.v1`
2. **Инициатива**: аудиторы могут предлагать улучшения в `skills.proposed.v1`
3. **Отчётность**: каждый аудит завершается отчётом в `planners.answers.v1`
4. **Безопасность**: все секреты и ключи — через Хранителя (`secrets.*`)
5. **Срочность**: критические уязвимости — немедленный алерт в `alerts.security.v1`
6. **Яса**: аудиторы обязаны соблюдать Ясу WATERS (не лги, держи слово, не вреди)
