# Memory validation

`memory-test-api` is a feature-gated, offline validation service for the permanent-memory
runtime. It is intentionally not part of the normal Tauri application.

```bash
cargo run --manifest-path src-tauri/Cargo.toml --features memory-test-api --bin memory-test-api
```

The service binds only to `127.0.0.1:0` and prints one authenticated ready JSON line.
The default scripted provider is deterministic and never accesses the production database.
Generated reports belong in `test/artifacts/` (ignored by git).
