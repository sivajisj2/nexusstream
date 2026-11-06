# Contributing

Keep protocol types independent from HTTP/storage. Add schema changes as new migrations. Include a test for any changed guarantee, especially concurrency, idempotency, and transaction boundaries.

Run formatting, strict Clippy, workspace tests, PostgreSQL tests, and HTTP smoke checks using docs/TESTING.md. Do not suppress warnings to hide unused production code. Commit Cargo.lock for reproducibility and never commit .env, credentials, local database files, or build artifacts.

Describe the observable before/after behavior and the validation performed in pull requests. Performance claims must include measured workload and environment. External destination adapters need a documented duplicate-delivery and crash-recovery contract.
