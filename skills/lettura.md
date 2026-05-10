---
name: lettura
description: AI-first interface to Lettura (read-it-later). Use when the user asks to save/retrieve/organize saved articles, e.g. "save this link", "summarize what I read last week", "tag my untagged entries".
<!-- lettura-skill-version: 0.1.0 -->
---

# Lettura CLI

Lettura is the user's read-it-later vault. You use `lettura-cli` over HTTP to read and write it.

Server: {{BASE_URL}}
Version: {{SERVER_VERSION}}

## When to trigger

Keywords: 收藏、稍后读、"save this", "my saved articles", "read-it-later", 整理文章, 打标签, "summarize what I read".

## Prerequisites

If the user hasn't set up `lettura-cli` yet, guide them:
1. `curl -sSL <install-script> | sh` to install the CLI
2. Open `{{BASE_URL}}/settings` and generate an API token
3. Run `lettura-cli login` and paste the token

Verify: `lettura-cli whoami` should return the current user info.

## Scenario 1: Retrieve a saved article's markdown

The user gives you a URL or a keyword → locate the id → fetch markdown.

```sh
# Search by keyword
lettura-cli search "rust async" --limit 5
# List with filter
lettura-cli list --filter "tag:backlog,since:7d" --limit 10 --fields id,title,url
# Get markdown (default format)
lettura-cli get <id>
# Get as JSON (for programmatic processing)
lettura-cli get <id> --format json
```

## Scenario 2: Organize entries (auto-tag)

**Always --dry-run before applying.** Typical workflow:

```sh
# Find untagged entries (capped at 20 to keep context manageable)
ids=$(lettura-cli list --filter untagged --output ids --limit 20)
for id in $ids; do
  entry=$(lettura-cli get $id --format json)
  # Read entry, decide tags (e.g. "tech/rust", "career")
  lettura-cli tag $id <tag1> <tag2>
done
```

Bulk-tag all Medium articles at once:

```sh
# 1. See what would be affected
lettura-cli tag --add medium --filter "domain:medium.com,untagged" --dry-run
# 2. After user confirms, apply
lettura-cli tag --add medium --filter "domain:medium.com,untagged" --yes
```

**Forbidden**: Do not delete entries (the CLI does not expose `delete`).

## Scenario 3: Save a link

```sh
lettura-cli save https://example.com/post
lettura-cli save https://example.com/post --tag rust,async --title "Custom title"
# Wait for fetch to complete (if you need content immediately)
lettura-cli save https://example.com/post --wait
```

Saving the same URL twice is safe: the server returns `already_existed: true`, and tags are merged as a set union.

## Scenario 4: Publish and manage static pages

Publish an HTML file as a shareable page:

```sh
lettura-cli pages publish ./site/index.html --title "My Page"
lettura-cli pages publish ./site/          # directory → auto-zip
lettura-cli pages publish https://example.com  # URL → fetch & publish
```

List, update, and share pages:

```sh
lettura-cli pages list
lettura-cli pages list --status disabled
lettura-cli pages update <id> --title "New Title"
lettura-cli pages update <id> --files ./new-site/  # replace files
lettura-cli pages share <id>
```

Delete and restore:

```sh
lettura-cli pages delete <id>
lettura-cli pages restore <id>
```

## Command cheatsheet

| Command | Description |
|---------|-------------|
| `save <url> [--title X] [--tag a,b] [--wait]` | Save a URL (idempotent) |
| `list [--filter E] [--limit N] [--output json|ids] [--fields ...]` | List entries |
| `search <query> [--limit N]` | Full-text search |
| `get <id> [--format markdown|json|html|text]` | Fetch single entry content |
| `tag <id> <name>...` | Add tags (single entry) |
| `tag --add X --filter E [--dry-run|--yes]` | Batch add tag |
| `untag <id> <name>...` | Remove tags (single entry) |
| `archive <id>` / `unarchive <id>` | Archive state toggle |
| `star <id>` / `unstar <id>` | Star state toggle |
| `tags` | List all tags |
| `audit-logs [--action X] [--resource-type Y] [--status Z] [--limit N] [--offset N]` | Query audit logs |
| `whoami` | Verify login |
| `pages publish <path\|url> [--title X] [--description D] [--password P] [--expires-at E]` | Publish a page from file, directory, or URL |
| `pages list [--status S] [--page N] [--limit N]` | List published pages |
| `pages update <id> [--title X] [--description D] [--password P] [--clear-password] [--status S] [--expires-at E] [--files F] [--entry-file E]` | Update a page |
| `pages delete <id>` | Delete a page (soft) |
| `pages restore <id>` | Restore a deleted page |
| `pages share <id>` | Get share URL for a page |

## Filter DSL

AND-combined, comma-separated. No OR or parens (use multiple commands if you need OR).

| Key | Meaning |
|-----|---------|
| `tag:<name>` / `!tag:<name>` | has / doesn't have tag |
| `untagged` | no tags at all |
| `domain:<host>` | by domain |
| `since:<rel|abs>` | from time (`7d` / `24h` / `2026-01-01`) |
| `older-than:<rel>` | older than |
| `starred` / `!starred` | starred state |
| `archived` / `!archived` | archived state |
| `read` / `unread` | read state (aliased to archived) |
| `search:<query>` | nested full-text search |

## Audit Logs

Server-side audit log API (`GET /api/v1/audit-logs`). Query params:

| Param | Type | Description |
|-------|------|-------------|
| `action` | enum | Filter by action e.g. `create_entry`, `delete_entry`, `login`, `bulk_tag_add` |
| `resource_type` | enum | `entry`, `tag`, `page`, `user`, etc. |
| `resource_id` | UUID | Exact resource match |
| `status` | string | `success`, `failure`, `forbidden` |
| `limit` | int | Page size (1–200, default 50) |
| `offset` | int | Pagination offset |

Response: `{ data: [...], total: N, limit: N, offset: N }`. Each log has `action`, `auth_source` (`jwt`/`pat`), `resource_type`, `resource_id`, `status`, `details` (JSON), `created_at`.

## Output schema (list / get fields)

Key fields (use with `--fields`):
`id, url, title, domain_name, tags, is_starred, is_archived, created_at, reading_time, language`

`get --format markdown` outputs front-matter YAML followed by the markdown body. The first lines of front-matter include id/url/title/tags/saved_at.

## Error codes

| Exit | code | Typical cause | Self-recovery |
|------|------|---------------|---------------|
| 2 | not_found | id does not exist | Run `list` to find ids |
| 3 | unauthorized/forbidden | token invalid or scope insufficient | Prompt user to re-login or upgrade scope |
| 4 | bad_args | bad params / filter syntax | Fix parameters |
| 5 | server_error | 5xx | Retry once or twice, then report to user |
| 6 | rate_limited | 429 | Wait per `hint.retry_after_sec` |
| 7 | conflict | business conflict | Read `message` and decide |

On failure, stderr carries `{"error": {"code": "...", "message": "...", "hint": "..."}}`. Follow `hint` when present.

## Safety rules

1. Any batch write (`--filter` + a write action) **must first run with `--dry-run`** and present the matched count to the user for confirmation. Only then run `--yes`.
2. `list` defaults to `--limit 20` to avoid context blowup — raise it only when justified.
3. Do not delete.
4. For organize-type tasks, present a plan first ("I'll tag these 20 entries with `X`") and let the user approve before executing.
5. `save` is idempotent — reruns are safe.

## Common pitfalls

- URLs with tracking params (`?utm_source=…`) count as different URLs. Strip them if dedup matters.
- `reading_time` may be null for older entries or failed fetches.
- `content` may be empty while a save is still queued — use `--wait` at save time, or check status via `get <id>` later.
