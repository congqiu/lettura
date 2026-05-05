# Release Skill Design

## 概述

为 Lettura 实现一站式发布流程：Claude Skill 交互引导 + Shell 脚本核心逻辑 + CI 自动构建发布。

## 版本号管理

### 统一版本号

Cargo workspace（server + cli）+ `web/package.json` 共享同一版本号，来源为 `Cargo.toml` 的 `workspace.package.version`。

### 扩展独立版本号

扩展版本号的唯一来源为 `extension/package.json`。`postbuild.mjs` 在构建时从 `package.json` 读取版本号（`import pkg from '../package.json'`），不再硬编码。源目录下的 `extension/manifest.json` 仅作为开发参考，不参与构建输出，不纳入版本号更新清单。

### 更新文件清单

主版本号更新时修改：
- `Cargo.toml` → `workspace.package.version`
- `web/package.json` → `version`

扩展版本号更新时修改（通过 `--ext-version` 参数触发）：
- `extension/package.json` → `version`

### 版本号校验

release.sh 在更新前校验：
1. 新版本号必须匹配 semver 格式：`^\d+\.\d+\.\d+$`（不含预发布后缀，lettura 暂不需要）
2. 新版本号必须大于当前版本号（按 major > minor > patch 逐段比较）

## Changelog 生成

### 自动分类规则

从 git log 提取上次 tag 到 HEAD 的 commit，按 conventional commit 前缀分类：

| 前缀（正则） | 分类 |
|------|------|
| `^(feat)(\(.+\))?!:` | Added |
| `^(fix)(\(.+\))?!:` | Fixed |
| `^(refactor|perf)(\(.+\))?!:` | Changed |
| `^(docs|chore|ci|test|style)(\(.+\))?!:` | 不记录 |
| `^(feat|fix)(\(.+\))?!:` + 含 `BREAKING CHANGE` | Breaking Changes（顶部）+ 原分类 |
| 无前缀 | Changed |
| `^security(\(.+\))?:` | Security |

Breaking change 检测：commit 前缀含 `!`（如 `feat(api)!:`）或 body 含 `BREAKING CHANGE` 时，除归入原分类外，还在顶部添加 `### Breaking Changes` 段落。

### 生成流程

1. `git describe --tags --abbrev=0 --match 'v*'` 找上一个版本 tag（过滤非版本 tag 如 `backup-*`）
2. `git log <last_tag>..HEAD --pretty=format:"%s" --no-merges` 获取 commit 列表
3. 按前缀分类，生成 `## [version] - date` 段落
4. 插入到 `CHANGELOG.md` 的 `## [Unreleased]` 下方
5. 清空 `## [Unreleased]` 的内容（保留标题和空行）
6. 若 `## [Unreleased]` 标题不存在，报错退出并提示手动添加

### CI Release Notes

在 release workflow 中用同样的分类逻辑生成 GitHub Release body，额外包含：
- Docker 镜像拉取命令
- CLI 安装命令
- 扩展下载说明
- Full Changelog 比较链接

## Release Skill 交互流程

### 使用方式

```
/release                    # 交互式选择版本类型
/release patch              # 发布补丁版本
/release minor              # 发布次要版本
/release major              # 发布主要版本
/release 1.0.0              # 发布指定版本号
/release patch --ext 1.3.0  # 同时更新扩展版本
```

### 执行步骤

1. **检查状态** — `git status` 确认工作区干净，`git log --oneline -5` 查看最近提交
2. **处理未提交变更** — 如果有未提交变更，提示用户先提交或暂存，不自动调用其他 skill
3. **确认版本号** — 调用 `scripts/release.sh --dry-run <version>` 获取变更预览和版本推荐，向用户确认
4. **预览变更** — 显示 dry-run 输出（将要更新的文件、新版本号、changelog 内容），确认后执行
5. **执行 `scripts/release.sh`** — 更新版本号 + 生成 changelog + git commit + git tag
6. **推送** — `git push origin $(git branch --show-current) && git push origin v<version>`（使用当前分支，不硬编码 main）
7. **验证** — 检查 tag 是否创建成功，报告结果

## scripts/release.sh

### 参数

```
scripts/release.sh <version> [--ext-version <version>] [--dry-run] [--no-push]
```

### 功能

1. 验证工作区干净
2. 校验版本号格式和单调递增
3. 更新版本号到所有目标文件
4. 生成 changelog 并更新 CHANGELOG.md
5. 创建 release commit（`chore: release v<version>`）
6. 创建 git tag（`v<version>`）
7. 可选推送到远程

### dry-run 模式

计算所有版本号变更和 changelog 内容，输出到终端（统一预览格式），但不写入文件、不创建 commit/tag。输出包含：
- 当前版本 → 新版本
- 每个将修改的文件路径
- 生成的 changelog 条目预览

### 错误处理

脚本使用 `set -euo pipefail`。在开始执行前，记录当前 HEAD commit hash。如果中途失败：
- 版本号文件变更：提示用户 `git checkout -- <files>` 恢复
- changelog 变更：同上
- commit 已创建但 tag 未创建：提示用户 `git reset --soft HEAD~1` 撤销 commit
- tag 已创建：提示用户 `git tag -d v<version>` 删除本地 tag

## CI Release Workflow 实现

### 触发条件

- `v*` tag push
- 手动 `workflow_dispatch`（带 `tag` 输入参数）

### Jobs

#### build-cli

3 平台构建 CLI binary：
- `x86_64-unknown-linux-gnu`（ubuntu-latest）
- `x86_64-apple-darwin`（macos-latest）
- `aarch64-apple-darwin`（macos-latest）

产物：`lettura-cli-v<version>-<target>.tar.gz`

#### build-docker

构建 Docker 镜像并推送到 GHCR。

权限：`packages: write`

步骤：
1. Checkout
2. 获取版本号（从 tag）
3. Docker Buildx 设置
4. GHCR 登录（`docker/login-action` with `ghcr.io`）
5. 构建并推送，打 semver 标签：`<version>`、`<major>.<minor>`、`<major>`、`latest`
6. 镜像名：`ghcr.io/<owner>/lettura`

#### build-extension

构建 Chrome 扩展 zip 包。

步骤：
1. Checkout
2. 安装 Node.js
3. 安装依赖（`npm ci` in extension/）
4. 构建（`npm run build`）
5. 打包为 zip
6. 从 `extension/package.json` 读取扩展版本号用于产物命名

产物：`lettura-extension-v<ext_version>.zip`

#### create-release

依赖前三个 job，生成 release notes 并创建 GitHub Release，上传所有产物。

步骤：
1. Checkout（fetch-depth: 0）
2. 获取版本号（从 tag）
3. 获取扩展版本号（从 `extension/package.json`）
4. 生成 release notes（与 changelog 同样的分类逻辑）
5. 创建 GitHub Release（`softprops/action-gh-release@v2`）
6. 上传 CLI binary + 扩展 zip

### Release Notes 模板

```markdown
## Lettura v<version>

### Breaking Changes
- ...（如有）

### Added
- ...

### Changed
- ...

### Fixed
- ...

### Security
- ...（如有）

### Docker

docker pull ghcr.io/${{ github.repository }}:<version>

### CLI Install

curl -fsSL https://raw.githubusercontent.com/${{ github.repository }}/main/scripts/install-cli.sh | bash

### Browser Extension

Download `lettura-extension-v<ext_version>.zip` from assets below.

---

**Full Changelog**: https://github.com/${{ github.repository }}/compare/<prev_tag>...v<version>
```

## 文件清单

| 文件 | 用途 |
|------|------|
| `.claude/skills/release/SKILL.md` | Claude Skill 定义 |
| `scripts/release.sh` | 发布核心脚本 |
| `.github/workflows/release.yml` | CI 发布 workflow（重写） |
| `extension/scripts/postbuild.mjs` | 改为从 package.json 读取版本号 |
| `CHANGELOG.md` | 自动更新 |
