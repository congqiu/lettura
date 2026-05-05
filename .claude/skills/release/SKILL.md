---
name: release
description: 发布新版本：更新版本号、生成 changelog、创建 tag 并推送
---

# Release Skill

一站式完成版本发布。

## 使用方式

```
/release                                   # 交互式选择版本类型
/release patch                             # 发布补丁版本
/release minor                             # 发布次要版本
/release major                             # 发布主要版本
/release 1.0.0                             # 发布指定版本号
/release patch --ext-version 1.3.0           # 同时更新扩展版本
```

## 执行步骤

### 1. 检查当前状态

运行以下命令了解项目状态：

```bash
git status
git log --oneline -5
```

如果工作区不干净，提示用户先提交或暂存变更，然后停止。

### 2. 确认版本号

读取当前版本号：

```bash
python3 -c "import re; m=re.search(r'\[workspace\.package\][^\[]*?version\s*=\s*\"([^\"]+)\"', open('Cargo.toml').read(), re.DOTALL); print(m.group(1))"
```

- 如果用户指定了版本类型（patch/minor/major），计算新版本号
- 如果用户指定了具体版本号，直接使用
- 如果用户未指定，根据自上次 tag 以来的 commit 内容推荐版本类型：
  - 存在 `feat:` 类型 commit → 推荐 `minor`
  - 仅有 `fix:` / `refactor:` 等 → 推荐 `patch`
  - 存在 breaking change → 推荐 `major`
- 向用户确认最终版本号

### 3. 预览变更

运行 dry-run 预览：

```bash
./scripts/release.sh <version> --dry-run
```

如果用户同时要求更新扩展版本：

```bash
./scripts/release.sh <version> --ext-version <ext_version> --dry-run
```

将 dry-run 输出展示给用户，确认后继续。

### 4. 执行发布

确认后执行：

```bash
./scripts/release.sh <version> [--ext-version <ext_version>]
```

脚本会自动完成：
- 更新版本号（Cargo.toml、web/package.json，以及可选的 extension/package.json）
- 生成 CHANGELOG.md 条目（按 Added/Changed/Fixed/Security 分类）
- 创建 release commit（`chore: release v<version>`）
- 创建 git tag（`v<version>`）
- 推送 commit 和 tag 到远程仓库

### 5. 验证发布结果

发布完成后验证：

```bash
git log --oneline -3
git tag -l --sort=-v:refname | head -5
git status
```

向用户报告发布结果，包括：
- 新版本号
- 推送状态
- GitHub Actions 将自动构建 CLI binary、Docker 镜像和浏览器扩展

## 注意事项

- 发布前必须确保所有变更已提交（工作区干净）
- 版本号遵循语义化版本规范（Semantic Versioning）
- Cargo workspace 和 web/package.json 版本号保持一致
- 扩展版本号独立管理（通过 `--ext-version` 参数）
- 发布过程中每个关键步骤都需要用户确认
- 如果发布失败，参考 `scripts/release.sh` 中的错误处理说明进行回滚
