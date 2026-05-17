# Task 002: Fix unwrap() in media_bridge.rs

**Priority:** P0
**Assigned to:** implementer
**Verified by:** reviewer + verifier
**Branch:** fix-unwrap-mediabridge

## Problem
`media_bridge.rs:107,114,119,153` uses `.unwrap()` on mutex locks.

## Solution
Replace `.unwrap()` on Mutex with pattern matching or `.ok()`:
```rust
// Before:
self.devices.lock().unwrap()
// After:
self.devices.lock().map_err(|e| anyhow!("Mutex error: {}", e))?
```

## Files to change
- `src/media_bridge.rs`

## Verification
- [ ] `cargo check` passes
- [ ] No unwrap() in media_bridge.rs remains
- [ ] Tests pass: `cargo test`
