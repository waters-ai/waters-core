# SKILL.md — auditor-token2022 v1.0.0

## Идентификация

| Поле | Значение |
|------|----------|
| skill_id | `auditor-token2022` |
| версия | 1.0.0 |
| владелец | `agent.auditor.kapelka.v1` |
| тип | `support` |
| слой | I — Сеть |
| статус | `active` |

## Назначение

Аудит интеграции Token-2022 на фронте. Проверка всех SPL Token Extensions.

**Program ID:** `TokenzQdBNbLqP5VEhdkAS6EPFLC1PHnBqCXEpPxuEb`

## Категории аудита

### 1. Confidential Transfer
- ZK proof generation на клиенте
- Auditor configuration
- Equality proofs
- Nullifier tracking

### 2. Transfer Fee
- Maximum fee calculation
- Fee recipient validation
- Fee иерархия (global vs transfer)

### 3. Metadata Extension
- Token Metadata (mpl-token-metadata integration)
- Отображение на фронте
- Update authority

### 4. Group/Member Pointers
- Group pointer валидация
- Member pointer корректность
- Group membership proofs

### 5. CPI Guard
- Enabled/disabled состояние
- CPI call whitelist
- Allow/deny списки

### 6. Pausable
- Freeze authority
- Pause/unpause состояние
- Trading freeze

### 7. Account checks
- Owner: Token-2022 Program ID
- Rent exemption
- Mint authority
- Freeze authority
- Close authority

## Инструменты

- `@solana/spl-token` (Token-2022)
- Solana CLI 3.0
- Anchor CLI
- spl-token CLI
