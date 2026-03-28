# Lettura

> 意大利语"阅读" — 一个轻量级自托管 read-it-later 应用

受 [wallabag](https://github.com/wallabag/wallabag) 启发，用 Rust 重新构建核心功能，追求极低资源占用和简单部署。

## 项目状态

🚧 **早期开发中** — 目前处于内容提取引擎 PoC 验证阶段

## 核心功能（规划中）

- URL 保存 + 智能内容提取（多层兜底策略）
- 文章内容可编辑
- 归档 / 收藏 / 标签管理
- 注释与高亮
- 全文搜索（tantivy）
- 快速捕获收集箱（Memo）
- 自动打标签规则
- RSS Feed 输出
- wallabag 数据导入 / 导出
- 浏览器扩展（Chrome/Firefox）
- PWA 移动端适配

## 技术架构

```
Rust (Axum) 单体服务
├── REST API
├── 内嵌 React SPA
├── 后台抓取队列
├── tantivy 全文搜索
└── PostgreSQL
```

## 文档

- [设计规格](docs/specs/2026-03-28-lettura-design.md)
- [实施计划 1: 内容提取 PoC](docs/plans/2026-03-28-plan1-extraction-poc.md)

## License

MIT
