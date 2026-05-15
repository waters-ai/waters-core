# SKILL.md — auditor-token2022 v1.0.1

## Идентификация

| Поле | Значение |
|------|----------|
| skill_id | `auditor-token2022` |
| версия | 1.0.1 |
| владелец | `agent.auditor.kapelka.v1` |
| тип | `support` |
| слой | I — Сеть |
| статус | `active` |

## Назначение

Аудит интеграции токенов в проекте Kapelka. ОБА токена — Token-2022.

## Программы

| Программа | ID |
|-----------|-----|
| **Token-2022** | `TokenzQdBNbLqP5VEhdkAS6EPFLC1PHnBqCXEpPxuEb` |
| **SPL Token** (не используется) | `TokenkegQfeZyiNwAJbNbGKPFXCWuBvf9Ss623VQ5DA` |

## Токены Kapelka

| Токен | Программа | Decimals | Описание |
|-------|-----------|----------|----------|
| **WATER** (H2O) | Token-2022 | 9 | Основной токен — стейкинг, инвайты, комиссии |
| **STABLE** (USDT) | Token-2022 | 6 | Стейблкоин — своп, сейл, комиссии |

## Примечание

SPL Token (`Tokenkeg...`) в проекте **НЕ ИСПОЛЬЗУЕТСЯ**. Крейт `spl_token_2022_interface` генерирует инструкции, совместимые с обоими программами, но все токены создаются через Token-2022 createMint.

`transfer_checked` из `spl_token_2022_interface` динамически выбирает программу через параметр `program_id: &Pubkey` — может работать с SPL Token, если передать соответствующий `program_id`. Но в Kapelka оба токена всегда Token-2022.

## Файлы с жёсткой привязкой к Token-2022

| Файл | Строка | Проверка |
|------|--------|----------|
| `utils/payment_manager.rs` | 52 | `payer_token.owner != spl_token_2022_interface::id()` |
| `instructions/utilits/deposit_withdraw.rs` | 54, 205 | `token_program.key != token_program_id()` |
| `instructions/user/swap_sol_to_stable.rs` | 101 | `token_program.key != token_program_id()` |

## Файлы с динамической поддержкой обоих программ

| Файл | Строка | Механизм |
|------|--------|----------|
| `instructions/user/transfer_tokens.rs` | 20 | `token_program_id = mint_account.owner` |
| `instructions/sale/sale.rs` | 97-102 | OR: Token-2022 или `sale_config.stablecoin_program` |
| `instructions/utils.rs` | 264 | `token_program_pubkey` из переданного `AccountInfo` |

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
- Solana CLI 3.0 (Agave)
- Anchor CLI
- spl-token CLI
