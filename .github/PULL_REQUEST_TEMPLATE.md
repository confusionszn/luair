## Summary
Explain what this PR changes and link any related issue.

## Scope & Safety Checklist
- [ ] Remains strictly an independent desktop overlay (zero game memory access or process injection)
- [ ] Frontend typecheck & production build pass (`pnpm run build`)
- [ ] Rust unit tests pass with zero warnings (`cargo test --manifest-path src-tauri/Cargo.toml`)
- [ ] Verified cross-platform compatibility guards (`#[cfg(target_os = ...)]`) where applicable
