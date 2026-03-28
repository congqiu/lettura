# Lettura - AI Agent Context

## 项目简介

Lettura（意大利语"阅读"）是一个受 wallabag 启发的 read-it-later 应用。Rust 后端 + React SPA 前端，容器优先部署。

## 关键文档

- 设计规格: `docs/specs/2026-03-28-lettura-design.md` — **必读**，包含所有数据模型、API、架构决策
- 实施计划 1: `docs/plans/2026-03-28-plan1-extraction-poc.md` — 内容提取 PoC，当前执行阶段
- 参考项目: `/home/cc/workspace/github/wallabag` — 原始 PHP 项目，可参考其设计但不要复制代码

## 当前状态

项目处于 **Plan 1: 内容提取 PoC** 阶段，尚未开始编码。Plan 1 包含 9 个 Task，严格 TDD 流程。

## 实施计划路线图

| 计划 | 内容 | 状态 |
|------|------|------|
| Plan 1 | 项目脚手架 + 内容提取 PoC | 计划已写，待执行 |
| Plan 2 | 核心后端（DB + Auth + Entry CRUD + 抓取队列） | 待 Plan 1 完成后编写 |
| Plan 3 | 高级功能（Tags, Memos, Annotations, Search, Rules, Import/Export, RSS, Admin） | 待编写 |
| Plan 4 | 前端 SPA（React + Vite + Tailwind） | 待编写 |
| Plan 5 | 浏览器扩展 + Docker 部署 | 待编写 |

**关键决策门**: Plan 1 的 Task 8 是 PoC 评估点。如果纯 Rust 内容提取通过率 < 80%，需执行 Task 9 切换到 Node.js 备选方案。这会影响 Docker 镜像大小（50-80MB → 120-150MB）。

## 技术栈

**后端:** Rust 2024, Axum, SQLx, PostgreSQL, tantivy, scraper, ammonia, reqwest, argon2, jsonwebtoken
**前端:** React 18+ (TypeScript), Vite, Tiptap, TanStack Query, Zustand, Tailwind CSS

## 关键设计决策（不可随意更改）

1. **单体架构** — 一个 Rust 二进制 + 内嵌 SPA 静态文件，不要拆微服务
2. **PostgreSQL only** — 不支持 SQLite，不做多数据库兼容
3. **JWT 认证** — 无 OAuth2，access token (15min) + refresh token (30 days in DB)
4. **内容提取优先** — 这是产品核心，必须先验证再建其他功能
5. **UUID 主键** — 所有表用 UUID，不用自增 ID
6. **不做 i18n** — 界面不做多语言
7. **不做 2FA** — 不做两步验证
8. **前后端分离但单容器** — SPA 编译后用 rust-embed 嵌入二进制

## 编码规范

- Rust: 每个模块严格 TDD（先写测试 → 确认失败 → 实现 → 确认通过 → 提交）
- 每个 commit 应该是一个原子性的、可编译的变更
- 测试命令: `cargo test`
- 不要跳过 Plan 中的 TDD 步骤，即使看起来很简单
- 代码注释用英文，文档用中文

## 不要做的事

- 不要引入 boa_engine（JS 引擎），已明确放弃此方案
- 不要添加 OAuth2 / i18n / 2FA
- 不要把前端拆成独立容器
- 不要用 SQLite
- 不要修改已通过评审的设计文档中的核心架构决策，除非有充分理由并记录
