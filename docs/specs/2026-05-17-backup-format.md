# Admin Backup Format Spec

> 创建日期: 2026-05-17
> 版本: 2.0 (NDJSON)

## 概述

Admin backup endpoint (`GET /api/v1/admin/backup`) 输出 NDJSON 流，每行一个 JSON 对象。相比 v1.0 的单一大 JSON bundle，NDJSON 格式允许逐行流式输出，避免大量数据时 OOM。

## NDJSON 行类型

每行 JSON 对象包含 `"type"` 字段用于区分数据类型：

| type | 说明 | 对应结构体 |
|------|------|-----------|
| `metadata` | 备份元信息（首行） | `{type, version, created_at}` |
| `user` | 用户记录 | `BackupUser` |
| `entry` | 文章记录 | `BackupEntry` |
| `tag` | 标签记录 | `BackupTag` |
| `entry_tag` | 文章-标签关联 | `BackupEntryTag` |
| `annotation` | 批注记录 | `BackupAnnotation` |
| `memo` | 备忘录记录 | `BackupMemo` |
| `tagging_rule` | 自动标签规则 | `BackupTaggingRule` |
| `site_rule` | 站点规则 | `BackupSiteRule` |

## 示例

```ndjson
{"type":"metadata","version":"2.0","created_at":"2026-05-17T12:00:00Z"}
{"type":"user","id":"...","username":"admin","email":"admin@example.com","is_admin":true,"feed_token":"abc","created_at":"...","updated_at":"..."}
{"type":"entry","id":"...","user_id":"...","url":"https://example.com","given_url":"https://example.com","hashed_url":"...","hashed_given_url":"...","title":"Example","content":"<p>...</p>","text_content":"...","content_type":"article","extract_method":"readability","is_content_edited":false,"language":null,"http_status":200,"reading_time":5,"preview_picture":null,"domain_name":"example.com","published_by":null,"metadata":{},"is_archived":false,"archived_at":null,"is_starred":false,"starred_at":null,"published_at":null,"created_at":"...","updated_at":"...","deleted_at":null}
{"type":"tag","id":"...","user_id":"...","label":"rust","slug":"rust","created_at":"..."}
{"type":"entry_tag","entry_id":"...","tag_id":"..."}
{"type":"annotation","id":"...","entry_id":"...","user_id":"...","quote":"...","text":"...","ranges":[],"is_orphaned":false,"created_at":"...","updated_at":"..."}
{"type":"memo","id":"...","user_id":"...","content":"...","source_url":null,"promoted_entry_id":null,"created_at":"..."}
{"type":"tagging_rule","id":"...","user_id":"...","rule":{},"tags":["rust"],"priority":1,"created_at":"..."}
{"type":"site_rule","id":"...","user_id":"...","domain":"example.com","content_selector":".content","title_selector":"h1","strip_selectors":[".ad"],"created_at":"..."}
```

## 版本兼容

- **v2.0**: NDJSON 格式，admin backup/restore 使用
- **v1.0**: 单一 JSON bundle 格式，用户级 export/import 使用

Restore endpoint 自动检测格式：首行包含 `"type":"metadata"` → NDJSON v2.0；否则 → legacy JSON v1.0。

## 安全

- `BackupUser` 不包含 `password_hash` 字段
- Restore 后用户密码设为不可登录的占位值 `!restored`，需重置密码