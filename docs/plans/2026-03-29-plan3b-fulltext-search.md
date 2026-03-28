# Plan 3b: 全文搜索 (tantivy)

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task.

**Goal:** 集成 tantivy 全文搜索引擎，支持在 entries 列表 API 中通过 `search` 参数搜索标题和正文内容。

**Architecture:** tantivy 索引存储在本地目录（Docker volume 挂载 /data/tantivy）。SearchIndex 封装索引的创建、写入、搜索。Entry 创建/更新/删除时异步更新索引。提供 reindex 函数从 DB 全量重建。

**Tech Stack:** tantivy 0.22, 已有 Axum/SQLx 栈

---

## 文件结构

```
lettura/
├── Cargo.toml                    — 添加 tantivy
├── src/
│   ├── search.rs                 — SearchIndex (create, add, delete, search, reindex)
│   ├── lib.rs                    — 添加 search 模块
│   ├── auth/middleware.rs        — AppState 添加 SearchIndex
│   ├── api/mod.rs                — 传递 SearchIndex
│   ├── api/entries.rs            — list_entries 支持 search 参数
│   ├── models/entry.rs           — ListParams 添加 search 字段
│   └── tasks/fetcher.rs          — 抓取完成后更新索引
├── tests/
│   └── integration_search.rs     — 搜索集成测试
```
