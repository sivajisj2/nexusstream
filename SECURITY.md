# Security policy

Report vulnerabilities privately to the repository owner's GitHub profile contact before public disclosure. Do not open a public issue containing credentials or real event payloads. There is no supported-version SLA yet; use the latest reviewed main branch.

Protected routes require a constant-time-compared API key. Keys are never embedded in UI assets and are kept only in dashboard memory. Source-controlled examples contain placeholders or explicitly local CI credentials. SQL uses bound parameters; event text is rendered with DOM text nodes. Request bodies and payloads are bounded.

This service is intended for a trusted workspace. The shared API key has full producer/operator privileges. Public deployment needs TLS, access controls, edge rate limits/timeouts, secret management, and a retention plan. OpenAPI/health/static assets are public by design; event data and metrics are protected. Review docs/OPERATIONS.md before exposing the service.
