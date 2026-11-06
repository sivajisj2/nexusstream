# Testing guide

## Repeatable quality gates

```bash
cargo fmt --all -- --check
cargo clippy --locked --workspace --all-targets -- -D warnings
cargo test --locked --workspace
node --check web/app.js
```

The offline suite covers encoding/decoding, invalid/oversized envelopes, name validation, API-key rejection, malformed JSON, request limits, pagination limits, OpenAPI auth metadata, and serving the dashboard. HTTP tests use a lazy unreachable pool so validation/auth tests cannot accidentally rely on a live database.

## Real database suite

Set `DATABASE_URL` to a disposable PostgreSQL server using a role with `CREATEDB` privileges:

```bash
cargo test --locked -p nexus-api --test database -- --ignored
```

Each `sqlx::test` creates a separate database, applies migrations, and cleans up. Tests are ignored by the offline command intentionally; CI runs them explicitly. Run against a local/test PostgreSQL server rather than a production database role.

Covered invariants:

1. Identical event retries create one row and one delivery.
2. Conflicting content rolls back every newly inserted event in its batch.
3. Concurrent ingestion of the same UUID accepts exactly once.
4. Worker archive bytes decode to the original event; acknowledgement persists.
5. Parallel workers do not duplicate archive records.
6. Keyset pagination is disjoint and exact source filters work.
7. A test-only archive constraint forces failure, backoff, five-attempt dead lettering, and successful replay after repair.
8. A rolled-back claim becomes available to the next worker.

## Running-instance acceptance

```bash
set -a; . ./.env; set +a
python3 scripts/smoke.py
```

Exercises public health/docs, protected routes, ingestion, deduplication, conflict handling, worker completion, and metrics through actual HTTP. It writes a synthetic `smoke` event and does not remove data.

## Workload measurement

```bash
python3 scripts/load.py --events 1000 --concurrency 8
```

Writes synthetic `loadtest` events and prints measured events/second, median, and p95 request latency. Results depend on CPU, storage, PostgreSQL settings, database size, concurrency, and network latency. This is a small acceptance workload, not a substitute for soak tests or a formal capacity report.

## Automated dashboard tests

With a running instance and `NEXUS_API_KEY` exported:

```bash
npm ci
npx playwright install chromium
npm run test:e2e
```

Playwright covers connection, publishing, payload inspection with untrusted HTML rendered safely, exact source filtering, mobile overflow, key clearing after reload, and Swagger rendering. These are test-only npm dependencies; the shipped UI has no npm runtime dependency.

## Dashboard checks

Connect with a valid key; verify counts and table. Publish a JSON object; inspect the stored payload. Filter by source; move between cursor pages. Submit malformed JSON and verify an actionable error. Try an invalid key and confirm records are not shown as freshly loaded. Reload the page and verify that the API key must be entered again. Inspect mobile layout at 390 px. External event fields render with `textContent`, never HTML.

## CI

The workflow runs formatting, strict Clippy, offline tests, isolated PostgreSQL tests, JavaScript syntax, HTTP smoke checks, and a container build. Dependabot tracks Cargo, Docker, and GitHub Actions updates. A green local suite is not a claim that GitHub CI or the Docker build has run; see the validation record for actual evidence.
