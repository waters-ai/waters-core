---
name: solana-kapelka-contract
description: Аудит и разбор Solana-контрактов Kapelka — stake, claim, rank, referrals
---

# SKILL.md — solana-kapelka-contract v1.0.0

## Идентификация

| Поле | Значение |
|------|----------|
| skill_id | `solana-kapelka-contract` |
| версия | 1.0.0 |
| владелец | `agent.constructor.v1` |
| тип | `support` |
| слой | III — Воля |
| статус | `active` |

## Назначение

Работа с контрактом Kapelka (`testw` v0.3.0, Solana 3.0) — системная инженерия, деплой, инициализация, тестирование, отладка.

## Версии

- Solana CLI: 3.1.15 (Agave)
- solana-program: 3.0.0
- solana-system-interface: 3.0.0 (bincode)
- spl-token-2022-interface: 2.1.0
- borsh: 1.6.0
- Rust: 1.89.0 (SBPF target)

## Программа

**Program ID:** `FxAtkaNUEDVHB2PaXD9hE5ZUWp7eD4Stq9ByTkc3y5Ea` (localnet)
**Админ:** `HgNtSk2WqBf4sVZXJTTwuGwU5P5cqw5P3xmxmMYoSm8u`
**WATER (Token-2022):** `Bkw6Guu3uB4XRCrYtJFE4jZWukKkjN8nUu2HKXhxsuWe`
**STABLE (SPL):** `g8YpArqEX8VuLvFXxxNvDjBUtXb1JN2KBuRCecjwKUk`
**Root PDA:** `BM4u9T57F4A6pvXoUq8WiabyFXFFt2464me8qPPredjL`
**Seed:** `water_seed_1778662412938_1235880b`
**Deploy:** 2026-05-13
**Git:** `e96bd33`

## Токены

Оба токена создаются через `createMint` с `TOKEN_2022_PROGRAM_ID`.

| Токен | Программа | Decimals |
|-------|-----------|----------|
| **WATER** (H2O) | `TokenzQdBNbLqP5VEhdkAS6EPFLC1PHnBqCXEpPxuEb` | 9 |
| **STABLE** (USDT) | `TokenzQdBNbLqP5VEhdkAS6EPFLC1PHnBqCXEpPxuEb` (или SPL `Tokenkeg...`) | 6 |

`transfer_checked` из крейта `spl_token_2022_interface` генерирует инструкции, совместимые с ОБОИМИ токен-программами. Параметр `program_id` в инструкции определяет целевую программу (Token-2022 или SPL).

**Файлы с проверкой токен-программы:**
- `payment_manager.rs:52` — жёстко Token-2022 (используется только для WATER/invite fee)
- `deposit_withdraw.rs:54,205` — жёстко Token-2022 (только WATER)
- `swap_sol_to_stable.rs:101` — жёстко Token-2022 (падёт при SPL STABLE)
- `transfer_tokens.rs:20` — **динамический**: `token_program_id = mint_account.owner`
- `sale.rs:97-102` — **двойной**: Token-2022 **или** `sale_config.stablecoin_program`

## Инвайты (Chunk System)

### Новая формула (v2)

```rust
chunk_index = invite_shard + overflow_level * 50
// Диапазон: 0-249 (250 чанков, 256-bit active_mask)
```

### chunk_utils.rs

```rust
get_chunk_index(shard: u8, overflow: u8) -> u32     // shard + overflow * 50
get_shard_from_chunk(chunk: u32) -> u8               // chunk % 50
get_overflow_from_chunk(chunk: u32) -> u8             // chunk / 50
```

### send_invite (tag 12) — 28 аккаунтов

```
[0]  inviter (signer, writable)
[1]  invitee
[2]  inviter_member (writable)
[3]  invitee_member (writable)
[4]  rent_pda (writable)
[5]  system_program
[6]  program_config_account
[7]  token_mint
[8]  inviter_token (writable)
[9]  invitee_token (writable)
[10] fee_vault_token (writable)
[11] immutable_seed_account
[12] rent_mema (writable)
[13] sol_fund (writable)
[14] token_program
[15] invite_sent_counter (writable)
[16] invite_receiver_counter (writable)
[17] invite_pda_acc (writable)
[18] invite_registry_chunk (writable)
[19] chunk_manager_acc (writable)
[20] chunk_manager_acc (writable) — повтор
[21] sent_counter (writable)
[22] receiver_counter (writable)
[23-27] expired_invite PDAs (writable, до 5 шт, или ZERO_PUBKEY)
```

**Данные:** `Buffer.from([12])`

### accept_invite (tag 13) — 23+4 аккаунта

- `chunk_manager_acc` **без** подчёркивания (используется для `decrement_filled_slots`)
- cleanup: registry.remove_entry + decrement_filled_slots(1) + lamports → rent_pda
- +4 extra_invites

### connect (tag 15) — 20 аккаунтов

**Параметры** (4 байта, Borsh):
```rust
ConnectParams {
    is_renewal: bool,          // 1 байт
    invite_shard: u8,          // 1 байт
    invite_overflow_level: u8, // 1 байт
}
// + 1 байт padding = 4 байта
```

**Тип 1 (новый юзер):** `shard = hash(pubkey) % active_chunks`, overflow=0
**Тип 2 (существующий):** `invite_shard` из User PDA

## ChunkManager

```rust
pub struct ChunkManager {
    pub total_chunks: u32,          // всего созданных
    pub active_chunks: u32,         // активных (фронт читает для hash % active_chunks)
    pub active_mask: [u64; 4],      // 256 бит
    pub fill_threshold: u8,         // 30%
    pub total_slots_filled: u32,    // всего занятых слотов
}
// LEN = 45 (было 49, удалён next_batch_threshold)
```

**Seeds:** `derive_chunk_manager_pda(program_id, root_pda)`
**Seeds:** `derive_invite_registry_chunk_pda(program_id, root_pda, chunk_index)`

## AdminAuth (Security)

### 5 администраторов

| Админ | Роль | Scope |
|-------|------|-------|
| Admin 1 | ProgramConfig | invite_expiry, комиссии, лимиты |
| Admin 2 | SaleConfig | параметры сейла |
| Admin 3 | StakingConfig | стейкинг |
| Admin 4 | FundsAndRanks | фонды, бонусы, ранги |
| Admin 5 | ColdStorage | экстренные, recovery |

### Авторизация

```rust
solo_hash = SHA256("WATER_ADMIN_SOLO_V1:" + pubkey_bytes + password_bytes)
```

**update_program_config (tag 0)** — проверяет:
1. `signer == AdminAuth.admin_1` (ProgramConfig)
2. `!is_blocked?` (3 failed → бан 1ч)
3. `verify_solo(admin.key, &solo_hash)?`

**Смена ключа — carousel (3/5 + timelock):**
`propose_change → confirm × 3 → timelock 24ч → apply_proposal`

### Аккаунты для tag 0

```
[0] admin (signer, writable)
[1] program_config_account (writable)
[2] admin_auth_account (writable)
[3] immutable_seed_account
```

## Команды

- `MAX_TEAM_MEMBERS = 940`
- `CHUNK_GROWTH_STEP = 94` — расширение чанка
- `MAX_DIRECT_REFERRALS_PER_PAGE = 17`, `MAX_REFERRAL_PAGES = 5` → 85 макс
- `TEAMS_PER_CHUNK = 67`, `MAX_ACTIVE_CHUNKS = 4`
- **create_team (tag 47):** leader платит 3 WATER, создаётся TeamChunk, TeamAccount, TeamRegistry

## User PDA

Размер: 512 байт (Borsh)

Поля инвайтов:
```rust
pub invite_shard: u8,             // 0-99 (базовый шард)
pub invite_overflow_level: u8,    // 0-3
```

Позиции в байтах (для чтения с фронта):
- `data[482]` = invite_shard
- `data[483]` = invite_overflow_level

## Полезные константы

| Константа | Значение | Описание |
|-----------|----------|----------|
| `OVERFLOW_STEP` | 50 | Шаг оверлея |
| `MAX_BASE_CHUNKS` | 100 | Макс базовых чанков |
| `INVITES_PER_CHUNK` | 96 | Слотов в чанке |
| `INITIAL_CHUNKS` | 10 | Начальных чанков |
| `INVITE_EXPIRY_SECONDS` | 900 | 15 минут (тест) |
| `MAX_OVERFLOW_LEVEL` | 3 | Уровней оверлея |
| `MAX_INVITES` | 13 | Макс инвайтов |
| `MAX_INVITEE` | 5 | Макс полученных |

## ChunkManager PDA

**derive:** `["chunk_manager", root_pda]`
**LEN:** 45 байт

```
offset  size  поле
0       4     total_chunks (u32)
4       4     active_chunks (u32)
8       32    active_mask ([u64; 4])
40      1     fill_threshold (u8)
41      4     total_slots_filled (u32)
```

## Штатный деплой и инит

### Порядок (строго)

```bash
# 1. Program keypair
solana-keygen new -o deploy/program-keypair.json --no-bip39-passphrase --force

# 2. Записать PROGRAM_ID в .env + обновить declare_id! в src/lib.rs
NEWPID=$(solana-keygen pubkey deploy/program-keypair.json)
sed -i "s/PROGRAM_ID=.*/PROGRAM_ID=$NEWPID/" .env scripts/initialization/.env 2>/dev/null
sed -i "s/declare_id!(\"[A-Za-z0-9]*\")/declare_id!(\"$NEWPID\")/" src/lib.rs

# 3. Build
cargo build-sbf

# 4. Fresh validator
pkill -f solana-test-validator 2>/dev/null; sleep 3; rm -rf /tmp/test-ledger
nohup solana-test-validator --reset > /tmp/v.log 2>&1 &
sleep 30

# 5. Deploy
solana config set --keypair deploy/testw-keypair.json
solana program deploy target/sbpf-solana-solana/release/testw.so \
  --program-id deploy/program-keypair.json

# 6. Seed + PDA (штатный генератор)
npx tsx scripts/generate-all-deploy.ts

# 7. Mint WATER (Token-2022) + STABLE (SPL)
npx tsx scripts/create-mints.ts

# 8. Копировать .env в инит
cp .env scripts/initialization/.env

# 9. Init (штатный раннер)
cd scripts/initialization
rm -f state.json
npx tsx ../run-deployment.ts

# 10. Mint 10M токенов + фонды
npx tsx scripts/fund-all-accounts.ts  # после init
  
# 11. Gen wallets + connect (для теста)
cd ../stress
npx tsx gen-940-wallets.ts
npx tsx connect-all-940-v2.ts
```

### Важно
- `generate-all-deploy.ts` читает `PROGRAM_ID` из `.env` — должен быть установлен до запуска
- `create-mints.ts` создаёт WATER (Token-2022) + STABLE (SPL) — обновляет .env
- После create-mints обязательно скопировать .env в initialization/
- `run-deployment.ts` запускает все фазы 1-10
- state.json удалять перед каждым полным перезапуском init
