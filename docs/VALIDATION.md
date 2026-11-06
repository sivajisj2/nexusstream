# Initial validation — 6 November 2026

## Executed and passed

| Check | Result |
|---|---|
| `cargo check --workspace --all-targets` | Pass |
| `cargo fmt --all -- --check` | Pass |
| `cargo clippy --locked --workspace --all-targets -- -D warnings` | Pass |
| `cargo test --locked --workspace` | 18 passed; 8 database tests explicitly ignored |
| `cargo test --locked -p nexus-api --test database -- --ignored` | All 8 database tests passed |
| Running-service `scripts/smoke.py` | All HTTP acceptance assertions passed |
| `npm run test:e2e` | Both Playwright tests passed |
| Additional browser acceptance | Publish, safe HTML payload display, filters, desktop/mobile layout, key not retained, Swagger rendering passed |
| `node --check web/app.js` | Pass |
| Python script compilation and shell syntax | Pass |

Rust 1.99.1, Linux x86_64, PostgreSQL 18.6, and Chromium were used for local validation. The test environment only maps uid 0, so an external test-only identity adapter allowed an isolated PostgreSQL process to initialize; this adapter is not part of the repository or deployment. Database and API processes ran together in an isolated local network. The deployment configuration targets PostgreSQL 17 and CI is configured to test that version; CI execution remains unverified until publication.

## Measured local workload

A running debug build accepted 1,000 individual synthetic events with eight concurrent clients against a local PostgreSQL instance:

- Wall time: 0.920 seconds
- Throughput: 1,086.52 events/second
- Median request latency: 5.97 milliseconds
- p95 request latency: 11.95 milliseconds

These numbers are one small local run, including Python client overhead. They do not establish production capacity, sustained throughput, or a service-level objective. The database already held synthetic acceptance events. Reproduce with `scripts/load.py` and record hardware, build profile, database configuration, and environment before comparing results.

## Not executed here

- Docker image/Compose runtime validation: Docker is unavailable in the authoring environment.
- GitHub Actions: requires the repository to be created and pushed.
- Long-duration soak, disk-full, failover, and internet-facing security/load tests.

The checked-in dashboard screenshot contains synthetic local test data only.
