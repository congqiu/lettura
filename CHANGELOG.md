# Changelog

## [Unreleased]

## [1.0.0] - 2026-05-16

### Added
- add pages subcommands and expand contract tests
- implement pages command with upload support
- add release.sh script for version bumping and changelog generation
- localize all UI text to Chinese
- add PAT token authentication with tab-based login UI
- overhaul visual design system, layout and mobile experience
- add refresh token replay detection, manual SSRF redirect validation, and harden multiple attack surfaces
- improve search layout, scrollbar styling, and code block overflow
- add input length limits, regex size limit, reduce import body max
- fix RSS CDATA injection, add CSP and Permissions-Policy headers
- hide internal error details from health endpoint, add bearer token auth for metrics
- hash page passwords with argon2, use constant-time comparison, remove password from share URLs
- revoke all refresh tokens on password change, harden refresh token rotation
- reject wildcard CORS in production mode
- add SSRF protection — block private IP ranges in fetch pipeline
- add swipe gestures, share target, and mobile Sheet
- responsive layout, safe area, and mobile navigation
- add title and tag options to createEntry API
- make operational parameters configurable via env vars

### Changed
- 拆分设置页为独立面板组件并优化交互体验
- overhaul design system and unify component patterns
- 消除所有 clippy 警告，参数结构体化替代多参数函数 #none
- 简化 expires_at 类型并统一 API 响应 #none
- extract auth_source_str, add log_success helper, fix tag cache invalidation

### Fixed
- 隐藏 password hash 并统一 API 响应类型 #none
- resolve CI failures and update dependencies
- read version from package.json instead of hardcoding
- map image_process_status as PG enum instead of String
- add host_permissions to fix CORS errors on API requests
- harden zip path validation, default registration to disabled, and inject ConnectInfo for rate limiting
- harden SSRF, rate limiting, HSTS, cookies, and deployment defaults
- bind postgres to localhost, fix .env.example, restrict DOMPurify, use unified API client
- remove unused imports and fix useRef argument
- pin pnpm version to resolve corepack prepare error
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
