# Task 001: Fix unwrap() in security.rs

**Priority:** P0
**Assigned to:** implementer
**Verified by:** reviewer + verifier
**Branch:** fix-unwrap-security

## Problem
`security.rs:130` uses `.unwrap()` which will crash if the key doesn't exist.

## Solution
```rust
// Before:
self.policies.get(group).unwrap().clone()

// After:
self.policies.get(group).cloned().unwrap_or_default()
```

## Files to change
- `src/security.rs`

## Verification
- [ ] `cargo check` passes
- [ ] No unwrap() in security.rs remains
- [ ] Tests pass: `cargo test`
