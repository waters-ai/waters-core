# Task 003: Add tests to bridge.rs

**Priority:** P0
**Assigned to:** verifier
**Verified by:** reviewer
**Branch:** add-tests-bridge

## Problem
`bridge.rs` has 0 tests. It's the core transport layer (LLM, chat, voice, MCP).

## Required tests
- test_llm_bridge_creation
- test_chat_bridge_stdin
- test_bridge_pool_register_get
- test_bridge_info_new
- test_link_profile_measure

## Files to change
- `src/bridge.rs` (add `#[cfg(test)] mod tests` at end)

## Verification
- [ ] `cargo test` passes all new tests
- [ ] Coverage for bridge.rs > 0%
