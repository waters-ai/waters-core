# SKILL.md — auditor-solana-frontend v1.0.0

## Идентификация

| Поле | Значение |
|------|----------|
| skill_id | `auditor-solana-frontend` |
| версия | 1.0.0 |
| владелец | `agent.auditor.kapelka.v1` |
| тип | `support` |
| слой | I — Сеть |
| статус | `active` |

## Назначение

Аудит интеграции Solana 3.0 на фронте: wallet adapter, Anchor TS SDK (@anchor-lang/core), RPC-вызовы, транзакции, PDA.

## Входные данные

| Поле | Тип | Описание |
|------|-----|----------|
| target_url | string | URL сайта для аудита |
| solana_network | string | devnet / mainnet |
| anchor_idl_path | string | Путь к IDL (если доступен) |
| run_wallet_test | boolean | Тестировать wallet adapter |
| run_rpc_test | boolean | Тестировать RPC endpoint |
| depth | string | quick / full |

## Категории аудита

### 1. Wallet Adapter
- Поддержка Phantom, Backpack, Solflare
- connect/disconnect — обработка ошибок
- Смена сети (devnet → mainnet)
- publicKey — проверка наличия перед отправкой
- Обработка rejected connection

### 2. Anchor TS SDK (@anchor-lang/core)
- Provider: wallet + connection
- Program: загрузка IDL, правильный programId
- Instruction builder: параметры, PDA seeds
- Transaction: версионность (v0), priority fees, dupe signer check
- Duplicate mutable accounts — теперь ошибка в Anchor 1.0

### 3. RPC
- Endpoint production (не devnet)
- Commitment: processed/confirmed/finalized
- Error handling: таймауты, fallback RPC
- Rate limiting
- sendTransaction — обработка ошибок

### 4. PDA
- Правильность seed'ов
- findProgramDerivedAddress vs findProgramAddress
- Bump seed — verify
- Проверка owner на deserialized accounts

### 5. Token-2022
- @solana/spl-token с Program ID: TokenzQdBNbLqP5VEhdkAS6EPFLC1PHnBqCXEpPxuEb
- Confidential Transfer proof generation на фронте
- Metadata extension — отображение
- Transfer Fee — расчёт на фронте

## Инструменты

- Lighthouse CI
- `@solana/web3.js` / `@anchor-lang/core`
- `@solana/wallet-adapter-*`
- Anchor CLI
- solana CLI 3.0 (agave)
