# Changelog

## [Unreleased]

### Added
- Personal Access Tokens (PAT) management UI and API (`POST/GET /api/v1/tokens`, `DELETE /api/v1/tokens/:id`). Tokens authenticate alongside JWT via `Authorization: Bearer lta_...`. Fine-grained scope: `read` (GET-only) or `write` (full API).
- `lettura-cli` AI-first CLI with subcommands: `login`, `whoami`, `config`, `list`, `search`, `get` (markdown/json/html/text), `save` (with `--wait`), `tag` / `untag` / `archive` / `star` (single + batch via `--filter`), `tags`, `skill print` / `skill install`.
- Filter DSL for `list` / batch operations: AND-combined conditions like `tag:rust,untagged,since:7d`.
- Bulk endpoints `POST /api/v1/entries/bulk/{tag,untag,archive,star}` with required `dry_run` preview step.
- Skill source `skills/lettura.md`, served publicly at `GET /skills/lettura.md` with server-version + base-URL substitution, and bundled into the CLI binary via rust-embed (`lettura-cli skill install`).
- GitHub Actions release workflow building `lettura-cli` for linux-x86_64, darwin-x86_64, and darwin-aarch64; `scripts/install-cli.sh` downloads the matching tarball.
- Cursor-based keyset pagination for `GET /api/v1/entries` (`cursor` query param, `X-Next-Cursor` response header). Cursor mode bypasses the page≤50 guard.
- Audit log system: migration 015 (`audit_logs` table + ENUMs + indexes), `GET /api/v1/audit-logs` with filtering and pagination, fire-and-forget helper, instrumentation across all major write endpoints.
- `lettura-cli audit-logs` subcommand for querying audit logs from the terminal.
- `dev.sh dev` command for combined backend (Docker) + frontend (Vite) development with unified logs.

### Changed
- Entry save (`POST /api/v1/entries`) is now idempotent: same URL returns existing entry with `already_existed: true`, tag set merged as union.
- `GET /api/v1/entries` list endpoint gained filters: `tag`, `exclude_tag`, `untagged`, `domain`, `since`, `before`, `is_read` (alias for `is_archived`).
- Repo is now a Cargo workspace (`.` and `cli/`); server crate is still `lettura`.
- Search index switched to buffered writes with a 3-second background flush and graceful-shutdown flush, eliminating per-write fsync. Permanent-delete and admin reindex still commit synchronously.
- `process_images` downloads images in parallel (max 8 in flight) and applies URL replacements longest-first to avoid substring collisions.
- Web client `staleTime` raised to 30s and `refetchOnWindowFocus` disabled by default. Mutations still call `invalidateQueries` so cache stays fresh after writes; visible behavior change is that switching tabs no longer triggers an immediate refetch.
- `process_body` in the fetch pipeline now takes `body` by value, eliminating a full `to_string()` clone for large HTML pages before `spawn_blocking`.
- `parse_retry_after_header` now supports RFC 7231 HTTP-date form (e.g. "Wed, 21 Oct 2099 07:28:00 GMT") in addition to the seconds-from-now form.
- Web client token-refresh interceptor now skips auth endpoints (login/register) to prevent infinite retry loops on 401 responses.

### Fixed
- `admin reindex` now commits the index clear before the rebuild, preventing a half-cleared index if the rebuild phase fails.
- PAT `last_used_at` update no longer aborts the request on transient DB errors.

### Security
- PAT tokens stored as SHA-256 hash only; only the first 12 bytes (`lta_…`) kept in plaintext for UI display.
- PAT and feed tokens now use `OsRng` with rejection sampling for uniform character distribution (replaces biased modulo over `thread_rng`).
- Path-traversal hardening on `/storage/*` and `/p/<slug>/<file>`: reject parent/root/empty path segments and percent-encoded escapes.
- Prometheus metric labels normalize `/p/<slug>` → `/p/{slug}` and `/feed/<token>` → `/feed/{token}` to prevent unbounded label cardinality and feed-token leakage.

## [0.1.0] - 2026-03-29

### Added
- Content extraction engine (pure Rust readability algorithm)
- JWT authentication with refresh token rotation
- Entry CRUD with async fetch queue and per-domain rate limiting
- Full-text search via tantivy
- Tags, annotations, memos system
- Auto-tagging rules engine
- Site-specific extraction rules
- Wallabag JSON and browser bookmarks import
- JSON export
- RSS feeds (unread/starred/archived)
- Admin user management and search reindex
- Admin backup/restore API
- React SPA frontend with Tiptap editor
- Browser extension (Chrome/Firefox) for quick save
- Docker deployment with embedded SPA
- API versioning (/api/v1/) with legacy redirect
- JWT secret startup validation
- Security response headers
- Health check endpoint (/api/health)
- Feed token rotation
- Configurable CORS
- Global and auth-specific rate limiting
- Soft delete with restore and permanent delete
- Prometheus metrics (optional)
- Centralized request validation
- Two-layer ErrorBoundary (app + page level)
- Network offline detection
- Code splitting for low-frequency pages

### Security
- Argon2 password hashing
- JWT access token (15min) + refresh token (30d) rotation
- Security headers (X-Content-Type-Options, X-Frame-Options, etc.)
- Rate limiting (100 req/min global, 10 req/min auth)
- HTML sanitization via ammonia
- SQL injection prevention (parameterized queries)
