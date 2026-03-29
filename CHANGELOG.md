# Changelog

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
