# Release Skill Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Implement a one-stop release flow for Lettura: shell script for core logic, Claude Skill for interactive guidance, and CI workflow for automated builds.

**Architecture:** `scripts/release.sh` handles version bumping, changelog generation, git commit/tag. `.claude/skills/release/SKILL.md` provides the interactive Claude skill that calls the script. `.github/workflows/release.yml` is rewritten to build CLI + Docker + extension and create a GitHub Release with categorized notes.

**Tech Stack:** Bash, GitHub Actions, Docker, Node.js (extension build only)

**Spec:** `docs/superpowers/specs/2026-05-05-release-skill-design.md`

---

## File Structure

| File | Action | Responsibility |
|------|--------|----------------|
| `scripts/release.sh` | Create | Core release logic: version bump, changelog, commit, tag |
| `.claude/skills/release/SKILL.md` | Create | Claude Skill definition for `/release` command |
| `.github/workflows/release.yml` | Rewrite | CI: build CLI + Docker + extension, create GitHub Release |
| `extension/scripts/postbuild.mjs` | Modify | Read version from `package.json` instead of hardcoding |
| `extension/manifest.json` | Modify | Sync version to match `package.json` (cosmetic, not used in build) |

---

### Task 1: Fix extension postbuild.mjs to read version from package.json

**Files:**
- Modify: `extension/scripts/postbuild.mjs`
- Modify: `extension/manifest.json`

Currently `postbuild.mjs` hardcodes `version: '1.2.0'`. Change it to import from `package.json`. Also sync `manifest.json` version to match.

- [ ] **Step 1: Update postbuild.mjs to read version from package.json**

Replace the entire content of `extension/scripts/postbuild.mjs`:

```javascript
// Post-build script: copy manifest and icons to dist
import { copyFileSync, mkdirSync, existsSync, writeFileSync, readFileSync } from 'fs';
import { resolve, dirname } from 'path';
import { fileURLToPath } from 'url';

const __dirname = dirname(fileURLToPath(import.meta.url));
const root = resolve(__dirname, '..');
const dist = resolve(root, 'dist');

// Read version from package.json
const pkg = JSON.parse(readFileSync(resolve(root, 'package.json'), 'utf8'));
const version = pkg.version;

// Create manifest.json for the extension
const manifest = {
  manifest_version: 3,
  name: 'Lettura',
  version: version,
  description: '保存文章到你的 Lettura 实例',
  permissions: ['activeTab', 'contextMenus', 'storage'],
  host_permissions: ['<all_urls>'],
  action: {
    default_popup: 'src/popup/index.html',
    default_icon: {
      '16': 'icons/icon16.png',
      '48': 'icons/icon48.png',
      '128': 'icons/icon128.png',
    },
  },
  background: {
    service_worker: 'background.js',
  },
  icons: {
    '16': 'icons/icon16.png',
    '48': 'icons/icon48.png',
    '128': 'icons/icon128.png',
  },
};

// Write manifest
writeFileSync(resolve(dist, 'manifest.json'), JSON.stringify(manifest, null, 2));
console.log(`Created manifest.json (version ${version})`);

// Copy icons
const iconsDir = resolve(dist, 'icons');
if (!existsSync(iconsDir)) {
  mkdirSync(iconsDir, { recursive: true });
}

const iconSizes = [16, 48, 128];
for (const size of iconSizes) {
  const src = resolve(root, 'src', 'icons', `icon${size}.png`);
  const dest = resolve(iconsDir, `icon${size}.png`);
  if (existsSync(src)) {
    copyFileSync(src, dest);
    console.log(`Copied icon${size}.png`);
  } else {
    console.warn(`Warning: icon${size}.png not found`);
  }
}

console.log('Build complete!');
```

- [ ] **Step 2: Sync manifest.json version to match package.json**

Replace the entire content of `extension/manifest.json`:

```json
{
  "manifest_version": 3,
  "name": "Lettura",
  "version": "1.2.0",
  "description": "Save articles to your Lettura instance",
  "permissions": ["activeTab", "contextMenus", "storage"],
  "host_permissions": ["<all_urls>"],
  "action": {
    "default_popup": "popup.html",
    "default_icon": {
      "16": "icons/icon16.png",
      "48": "icons/icon48.png",
      "128": "icons/icon128.png"
    }
  },
  "background": {
    "service_worker": "background.js"
  },
  "icons": {
    "16": "icons/icon16.png",
    "48": "icons/icon48.png",
    "128": "icons/icon128.png"
  }
}
```

- [ ] **Step 3: Verify extension builds correctly**

Run: `cd extension && npm run build`
Expected: Build succeeds, `dist/manifest.json` contains `"version": "1.2.0"`

- [ ] **Step 4: Commit**

```bash
git add extension/scripts/postbuild.mjs extension/manifest.json
git commit -m "fix(extension): read version from package.json instead of hardcoding"
```

---

### Task 2: Create scripts/release.sh

**Files:**
- Create: `scripts/release.sh`

This is the core release script. It handles version bumping, changelog generation, git commit, and tag creation.

- [ ] **Step 1: Write the release.sh script**

```bash
#!/usr/bin/env bash
# NOTE: This script uses GNU sed syntax. Run inside Docker or on Linux.
# macOS users: use `docker compose run --rm lettura scripts/release.sh ...`
set -euo pipefail

# ── Release script for Lettura ──────────────────────────────────────
# Usage: scripts/release.sh <version> [--ext-version <version>] [--dry-run] [--no-push]

WORKSPACE_ROOT="$(cd "$(dirname "$0")/.." && pwd)"
DRY_RUN=false
NO_PUSH=false
NEW_VERSION=""
EXT_VERSION=""

# ── Parse arguments ─────────────────────────────────────────────────
while [[ $# -gt 0 ]]; do
  case "$1" in
    --dry-run)   DRY_RUN=true; shift ;;
    --no-push)   NO_PUSH=true; shift ;;
    --ext-version)
      [[ $# -lt 2 ]] && { echo "error: --ext-version requires a value" >&2; exit 1; }
      EXT_VERSION="$2"; shift 2 ;;
    -*)
      echo "error: unknown option $1" >&2; exit 1 ;;
    *)
      if [[ -z "$NEW_VERSION" ]]; then
        NEW_VERSION="$1"; shift
      else
        echo "error: unexpected argument $1" >&2; exit 1
      fi ;;
  esac
done

if [[ -z "$NEW_VERSION" ]]; then
  echo "usage: scripts/release.sh <version> [--ext-version <version>] [--dry-run] [--no-push]" >&2
  exit 1
fi

# ── Helpers ──────────────────────────────────────────────────────────
info()  { printf '\033[36m==> %s\033[0m\n' "$*" >&2; }
ok()    { printf '\033[32m✓ %s\033[0m\n' "$*" >&2; }
err()   { printf '\033[31m✗ %s\033[0m\n' "$*" >&2; exit 1; }
warn()  { printf '\033[33m⚠ %s\033[0m\n' "$*" >&2; }

# ── Validate version format ─────────────────────────────────────────
validate_semver() {
  local v="$1"
  [[ "$v" =~ ^[0-9]+\.[0-9]+\.[0-9]+$ ]] || err "invalid semver: $v (expected X.Y.Z)"
}

validate_semver "$NEW_VERSION"
[[ -n "$EXT_VERSION" ]] && validate_semver "$EXT_VERSION"

# ── Read current version ────────────────────────────────────────────
CURRENT_VERSION="$(grep -oP 'version\s*=\s*"\K[^"]+' "$WORKSPACE_ROOT/Cargo.toml" | head -1)"
info "current version: $CURRENT_VERSION"

# ── Validate monotonic increase ─────────────────────────────────────
version_gt() {
  local IFS='.'
  local -a a=($1) b=($2)
  [[ ${a[0]} -gt ${b[0]} ]] && return 0
  [[ ${a[0]} -lt ${b[0]} ]] && return 1
  [[ ${a[1]} -gt ${b[1]} ]] && return 0
  [[ ${a[1]} -lt ${b[1]} ]] && return 1
  [[ ${a[2]} -gt ${b[2]} ]] && return 0
  return 1
}

version_gt "$NEW_VERSION" "$CURRENT_VERSION" || err "new version $NEW_VERSION is not greater than current $CURRENT_VERSION"

if [[ -n "$EXT_VERSION" ]]; then
  CURRENT_EXT_VERSION="$(node -e "console.log(require('$WORKSPACE_ROOT/extension/package.json').version)")"
  version_gt "$EXT_VERSION" "$CURRENT_EXT_VERSION" || err "ext version $EXT_VERSION is not greater than current $CURRENT_EXT_VERSION"
fi

# ── Check working directory ─────────────────────────────────────────
if [[ "$(git -C "$WORKSPACE_ROOT" status --porcelain)" ]]; then
  err "working directory is not clean. Commit or stash changes first."
fi

# ── Find previous tag ───────────────────────────────────────────────
PREV_TAG="$(git -C "$WORKSPACE_ROOT" describe --tags --abbrev=0 --match 'v*' 2>/dev/null || true)"
if [[ -n "$PREV_TAG" ]]; then
  info "previous tag: $PREV_TAG"
else
  info "no previous version tag found"
fi

# ── Generate changelog ──────────────────────────────────────────────
generate_changelog() {
  local range="${PREV_TAG:+$PREV_TAG..HEAD}"
  local commits
  if [[ -n "$range" ]]; then
    commits="$(git -C "$WORKSPACE_ROOT" log "$range" --pretty=format:"%s" --no-merges 2>/dev/null || true)"
  else
    commits="$(git -C "$WORKSPACE_ROOT" log --pretty=format:"%s" --no-merges -50 2>/dev/null || true)"
  fi

  [[ -z "$commits" ]] && { warn "no commits found since $PREV_TAG"; return; }

  # Also get commit bodies for BREAKING CHANGE detection
  local bodies
  if [[ -n "$range" ]]; then
    bodies="$(git -C "$WORKSPACE_ROOT" log "$range" --pretty=format:"%B---COMMIT_SEP---" --no-merges 2>/dev/null || true)"
  else
    bodies="$(git -C "$WORKSPACE_ROOT" log --pretty=format:"%B---COMMIT_SEP---" --no-merges -50 2>/dev/null || true)"
  fi

  local breaking="" added="" changed="" fixed="" security=""
  while IFS= read -r line; do
    [[ -z "$line" ]] && continue
    local msg="${line}"

    # Check for breaking change: prefix with ! or body contains BREAKING CHANGE
    local is_breaking=false
    if [[ "$msg" =~ ^[a-z]+(\(.+\))?!: ]] || echo "$bodies" | grep -q "BREAKING CHANGE"; then
      # Only mark as breaking if THIS commit's subject has ! or its body has BREAKING CHANGE
      if [[ "$msg" =~ ^[a-z]+(\(.+\))?!: ]]; then
        is_breaking=true
      fi
    fi

    # Strip prefix for description
    local desc="${msg}"
    if [[ "$msg" =~ ^[a-z]+(\(.+\))?!?: ]]; then
      desc="${msg#*: }"
    fi

    if [[ "$msg" =~ ^security(\(.+\))?: ]]; then
      security="${security}- ${desc}"$'\n'
    elif [[ "$msg" =~ ^feat(\(.+\))?!?: ]]; then
      added="${added}- ${desc}"$'\n'
    elif [[ "$msg" =~ ^fix(\(.+\))?!?: ]]; then
      fixed="${fixed}- ${desc}"$'\n'
    elif [[ "$msg" =~ ^(refactor|perf)(\(.+\))?!?: ]]; then
      changed="${changed}- ${desc}"$'\n'
    elif [[ "$msg" =~ ^(docs|chore|ci|test|style)(\(.+\))?!?: ]]; then
      : # skip internal changes
    else
      changed="${changed}- ${msg}"$'\n'
    fi

    if [[ "$is_breaking" == true ]]; then
      breaking="${breaking}- ${desc}"$'\n'
    fi
  done <<< "$commits"

  # Also check bodies for BREAKING CHANGE and add matching subjects
  local commit_idx=0
  local subject_arr=()
  while IFS= read -r line; do
    [[ -z "$line" ]] && continue
    subject_arr+=("$line")
  done <<< "$commits"

  local sep_idx=0
  local body_arr=()
  while IFS= read -r chunk; do
    body_arr+=("$chunk")
  done < <(echo "$bodies" | awk -F'---COMMIT_SEP---' '{print $0}')

  for i in "${!subject_arr[@]}"; do
    if [[ "${body_arr[$i]:-}" == *"BREAKING CHANGE"* ]]; then
      local desc="${subject_arr[$i]}"
      if [[ "$desc" =~ ^[a-z]+(\(.+\))?!?: ]]; then
        desc="${desc#*: }"
      fi
      # Avoid duplicate if already detected via ! prefix
      if [[ "$breaking" != *"- ${desc}"* ]]; then
        breaking="${breaking}- ${desc}"$'\n'
      fi
    fi
  done

  local date="$(date +%Y-%m-%d)"
  local entry="## [${NEW_VERSION}] - ${date}"$'\n'$'\n'

  [[ -n "$breaking" ]] && entry+="### Breaking Changes"$'\n'"${breaking}"$'\n'
  [[ -n "$added" ]]    && entry+="### Added"$'\n'"${added}"$'\n'
  [[ -n "$changed" ]]  && entry+="### Changed"$'\n'"${changed}"$'\n'
  [[ -n "$fixed" ]]    && entry+="### Fixed"$'\n'"${fixed}"$'\n'
  [[ -n "$security" ]] && entry+="### Security"$'\n'"${security}"$'\n'

  echo "$entry"
}

CHANGELOG_ENTRY="$(generate_changelog)"

# ── Dry-run: preview and exit ───────────────────────────────────────
if [[ "$DRY_RUN" == true ]]; then
  echo "=== DRY RUN ===" >&2
  echo "" >&2
  info "version: $CURRENT_VERSION → $NEW_VERSION"
  if [[ -n "$EXT_VERSION" ]]; then
    info "ext version: $CURRENT_EXT_VERSION → $EXT_VERSION"
  fi
  echo "" >&2
  info "files to update:"
  echo "  Cargo.toml (workspace.package.version)"
  echo "  web/package.json (version)"
  if [[ -n "$EXT_VERSION" ]]; then
    echo "  extension/package.json (version)"
  fi
  echo "" >&2
  info "changelog entry:"
  echo "$CHANGELOG_ENTRY"
  echo "" >&2
  info "git operations:"
  echo "  commit: chore: release v${NEW_VERSION}"
  echo "  tag:    v${NEW_VERSION}"
  [[ "$NO_PUSH" == false ]] && echo "  push:   origin $(git -C "$WORKSPACE_ROOT" branch --show-current) + v${NEW_VERSION}"
  ok "dry run complete — no changes made"
  exit 0
fi

# ── Record HEAD for rollback ────────────────────────────────────────
HEAD_BEFORE="$(git -C "$WORKSPACE_ROOT" rev-parse HEAD)"
info "HEAD before release: ${HEAD_BEFORE:0:8}"

# ── Update version numbers ──────────────────────────────────────────
info "updating version to $NEW_VERSION ..."

# Cargo.toml — workspace.package.version
sed -i -e "0,/^version = \"${CURRENT_VERSION}\"/s//version = \"${NEW_VERSION}\"/" "$WORKSPACE_ROOT/Cargo.toml"
ok "Cargo.toml updated"

# web/package.json
node -e "
const fs = require('fs');
const p = '$WORKSPACE_ROOT/web/package.json';
const pkg = JSON.parse(fs.readFileSync(p, 'utf8'));
pkg.version = '$NEW_VERSION';
fs.writeFileSync(p, JSON.stringify(pkg, null, 2) + '\n');
"
ok "web/package.json updated"

# extension/package.json (only if --ext-version)
if [[ -n "$EXT_VERSION" ]]; then
  node -e "
  const fs = require('fs');
  const p = '$WORKSPACE_ROOT/extension/package.json';
  const pkg = JSON.parse(fs.readFileSync(p, 'utf8'));
  pkg.version = '$EXT_VERSION';
  fs.writeFileSync(p, JSON.stringify(pkg, null, 2) + '\n');
  "
  ok "extension/package.json updated to $EXT_VERSION"
fi

# ── Update CHANGELOG.md ─────────────────────────────────────────────
info "updating CHANGELOG.md ..."

CHANGELOG_PATH="$WORKSPACE_ROOT/CHANGELOG.md"
if [[ ! -f "$CHANGELOG_PATH" ]]; then
  err "CHANGELOG.md not found"
fi

# Verify ## [Unreleased] exists
if ! grep -q '^## \[Unreleased\]' "$CHANGELOG_PATH"; then
  err "CHANGELOG.md is missing '## [Unreleased]' header. Add it before releasing."
fi

# Pass changelog entry via temp file to avoid shell injection
CHANGELOG_TMP="$(mktemp)"
echo "$CHANGELOG_ENTRY" > "$CHANGELOG_TMP"

python3 -c "
import sys

changelog_path = '$CHANGELOG_PATH'
entry_path = '$CHANGELOG_TMP'

with open(entry_path, 'r') as f:
    entry = f.read()

with open(changelog_path, 'r') as f:
    lines = f.readlines()

# Find ## [Unreleased] line
unreleased_idx = None
for i, line in enumerate(lines):
    if line.strip() == '## [Unreleased]':
        unreleased_idx = i
        break

if unreleased_idx is None:
    print('error: ## [Unreleased] not found', file=sys.stderr)
    sys.exit(1)

# Find next ## [ line (start of previous version section)
next_section_idx = None
for i in range(unreleased_idx + 1, len(lines)):
    if lines[i].startswith('## ['):
        next_section_idx = i
        break

# Clear content between [Unreleased] and next section
# Keep the [Unreleased] header and one blank line
new_lines = lines[:unreleased_idx + 1]
new_lines.append('\n')
new_lines.append(entry)
if next_section_idx is not None:
    new_lines.extend(lines[next_section_idx:])

with open(changelog_path, 'w') as f:
    f.writelines(new_lines)
"
rm -f "$CHANGELOG_TMP"
ok "CHANGELOG.md updated"

# ── Git commit and tag ───────────────────────────────────────────────
info "creating release commit and tag ..."
git -C "$WORKSPACE_ROOT" add -A
git -C "$WORKSPACE_ROOT" commit -m "chore: release v${NEW_VERSION}"
git -C "$WORKSPACE_ROOT" tag "v${NEW_VERSION}"
ok "commit and tag v${NEW_VERSION} created"

# ── Push ─────────────────────────────────────────────────────────────
if [[ "$NO_PUSH" == false ]]; then
  info "pushing to remote ..."
  BRANCH="$(git -C "$WORKSPACE_ROOT" branch --show-current)"
  git -C "$WORKSPACE_ROOT" push origin "$BRANCH"
  git -C "$WORKSPACE_ROOT" push origin "v${NEW_VERSION}"
  ok "pushed to origin/$BRANCH and tag v${NEW_VERSION}"
else
  warn "skipping push (--no-push)"
  info "to push manually:"
  echo "  git push origin $(git -C "$WORKSPACE_ROOT" branch --show-current)"
  echo "  git push origin v${NEW_VERSION}"
fi

ok "release v${NEW_VERSION} complete!"
```

- [ ] **Step 2: Make script executable**

Run: `chmod +x scripts/release.sh`

- [ ] **Step 3: Test dry-run mode**

Run: `./scripts/release.sh 0.2.0 --dry-run`
Expected: Shows version change preview, files to update, changelog entry, and git operations. No files modified.

- [ ] **Step 4: Verify no side effects from dry-run**

Run: `git status`
Expected: Working directory clean (no changes from dry-run)

- [ ] **Step 5: Commit**

```bash
git add scripts/release.sh
git commit -m "feat: add release.sh script for version bumping and changelog generation"
```

---

### Task 3: Create Claude Skill definition

**Files:**
- Create: `.claude/skills/release/SKILL.md`

- [ ] **Step 1: Create the skill directory and file**

```bash
mkdir -p .claude/skills/release
```

Write `.claude/skills/release/SKILL.md`:

```markdown
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
/release patch --ext 1.3.0                 # 同时更新扩展版本
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
grep -oP 'version\s*=\s*"\K[^"]+' Cargo.toml | head -1
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
- 扩展版本号独立管理（通过 `--ext` 参数）
- 发布过程中每个关键步骤都需要用户确认
- 如果发布失败，参考 `scripts/release.sh` 中的错误处理说明进行回滚
```

- [ ] **Step 2: Verify skill is recognized**

Run: `cat .claude/skills/release/SKILL.md`
Expected: File content matches what was written

- [ ] **Step 3: Commit**

```bash
git add .claude/skills/release/SKILL.md
git commit -m "feat: add /release Claude skill definition"
```

---

### Task 4: Rewrite CI release workflow

**Files:**
- Rewrite: `.github/workflows/release.yml`

The current workflow only builds CLI binary. Rewrite it to build CLI + Docker + extension and create a GitHub Release with categorized notes.

- [ ] **Step 1: Write the new release.yml**

```yaml
name: Release

on:
  push:
    tags:
      - 'v*'
  workflow_dispatch:
    inputs:
      tag:
        description: 'Tag to release (e.g., v1.0.0)'
        required: true
        type: string

permissions:
  contents: write
  packages: write

jobs:
  # ── Build CLI binaries ──────────────────────────────────────────
  build-cli:
    name: build-cli ${{ matrix.target }}
    strategy:
      fail-fast: false
      matrix:
        include:
          - os: ubuntu-latest
            target: x86_64-unknown-linux-gnu
          - os: macos-latest
            target: x86_64-apple-darwin
          - os: macos-latest
            target: aarch64-apple-darwin
    runs-on: ${{ matrix.os }}
    steps:
      - uses: actions/checkout@v4
        with:
          ref: ${{ github.event_name == 'workflow_dispatch' && github.event.inputs.tag || '' }}

      - name: Get version from tag
        id: get_version
        run: |
          if [[ "${{ github.event_name }}" == "workflow_dispatch" ]]; then
            VERSION="${{ github.event.inputs.tag }}"
          else
            VERSION="${GITHUB_REF#refs/tags/}"
          fi
          echo "version=${VERSION}" >> "$GITHUB_OUTPUT"

      - uses: dtolnay/rust-toolchain@stable
        with:
          targets: ${{ matrix.target }}

      - uses: Swatinem/rust-cache@v2
        with:
          workspaces: |
            . -> target

      - name: build
        run: cargo build --release -p lettura-cli --target ${{ matrix.target }}

      - name: package
        id: package
        shell: bash
        run: |
          set -euo pipefail
          VERSION="${{ steps.get_version.outputs.version }}"
          NAME="lettura-cli-${VERSION}-${{ matrix.target }}"
          mkdir -p dist
          cp "target/${{ matrix.target }}/release/lettura-cli" "dist/lettura-cli"
          tar -czf "dist/${NAME}.tar.gz" -C dist lettura-cli
          echo "asset_path=dist/${NAME}.tar.gz" >> "$GITHUB_OUTPUT"
          echo "asset_name=${NAME}.tar.gz" >> "$GITHUB_OUTPUT"

      - uses: actions/upload-artifact@v4
        with:
          name: cli-${{ matrix.target }}
          path: ${{ steps.package.outputs.asset_path }}

  # ── Build and push Docker image ─────────────────────────────────
  build-docker:
    name: Build Docker Image
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4

      - name: Get version from tag
        id: get_version
        run: |
          if [[ "${{ github.event_name }}" == "workflow_dispatch" ]]; then
            VERSION="${{ github.event.inputs.tag }}"
          else
            VERSION="${GITHUB_REF#refs/tags/}"
          fi
          echo "version=${VERSION}" >> "$GITHUB_OUTPUT"
          echo "version_number=${VERSION#v}" >> "$GITHUB_OUTPUT"

      - name: Set up Docker Buildx
        uses: docker/setup-buildx-action@v3

      - name: Log in to GitHub Container Registry
        uses: docker/login-action@v3
        with:
          registry: ghcr.io
          username: ${{ github.actor }}
          password: ${{ secrets.GITHUB_TOKEN }}

      - name: Extract metadata for Docker
        id: meta
        uses: docker/metadata-action@v5
        with:
          images: ghcr.io/${{ github.repository }}
          tags: |
            type=semver,pattern={{version}}
            type=semver,pattern={{major}}.{{minor}}
            type=semver,pattern={{major}}
            type=raw,value=latest

      - name: Build and push Docker image
        uses: docker/build-push-action@v6
        with:
          context: .
          push: true
          tags: ${{ steps.meta.outputs.tags }}
          labels: ${{ steps.meta.outputs.labels }}
          cache-from: type=gha
          cache-to: type=gha,mode=max

  # ── Build browser extension ─────────────────────────────────────
  build-extension:
    name: Build Extension
    runs-on: ubuntu-latest
    defaults:
      run:
        working-directory: extension
    steps:
      - uses: actions/checkout@v4

      - uses: actions/setup-node@v4
        with:
          node-version: 22

      - uses: pnpm/action-setup@v4
        with:
          version: 10

      - name: Get extension version
        id: ext
        run: echo "version=$(jq -r .version package.json)" >> "$GITHUB_OUTPUT"

      - run: pnpm install --frozen-lockfile

      - run: pnpm run build

      - name: Package extension
        run: |
          cd dist
          zip -r "../lettura-extension-v${{ steps.ext.outputs.version }}.zip" .

      - uses: actions/upload-artifact@v4
        with:
          name: extension
          path: extension/lettura-extension-v${{ steps.ext.outputs.version }}.zip

  # ── Create GitHub Release ───────────────────────────────────────
  create-release:
    name: Create Release
    needs: [build-cli, build-docker, build-extension]
    if: always() && needs.build-cli.result == 'success' && needs.build-docker.result == 'success' && needs.build-extension.result == 'success'
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
        with:
          fetch-depth: 0

      - name: Get version from tag
        id: get_version
        run: |
          if [[ "${{ github.event_name }}" == "workflow_dispatch" ]]; then
            VERSION="${{ github.event.inputs.tag }}"
          else
            VERSION="${GITHUB_REF#refs/tags/}"
          fi
          echo "version=${VERSION}" >> "$GITHUB_OUTPUT"
          echo "version_number=${VERSION#v}" >> "$GITHUB_OUTPUT"

      - name: Get extension version
        id: ext
        run: echo "version=$(jq -r .version extension/package.json)" >> "$GITHUB_OUTPUT"

      - name: Find previous tag
        id: prev_tag
        run: |
          PREV_TAG=$(git describe --tags --abbrev=0 --match 'v*' HEAD~1 2>/dev/null || echo "")
          echo "tag=${PREV_TAG}" >> "$GITHUB_OUTPUT"

      - name: Generate release notes
        id: release_notes
        run: |
          VERSION="${{ steps.get_version.outputs.version }}"
          VERSION_NUM="${{ steps.get_version.outputs.version_number }}"
          EXT_VERSION="${{ steps.ext.outputs.version }}"
          PREV_TAG="${{ steps.prev_tag.outputs.tag }}"

          # Get commits since last tag
          if [[ -n "$PREV_TAG" ]]; then
            COMMITS=$(git log ${PREV_TAG}..HEAD --pretty=format:"%s" --no-merges)
            BODIES=$(git log ${PREV_TAG}..HEAD --pretty=format:"%B---COMMIT_SEP---" --no-merges)
          else
            COMMITS=$(git log --pretty=format:"%s" --no-merges -50)
            BODIES=$(git log --pretty=format:"%B---COMMIT_SEP---" --no-merges -50)
          fi

          # Categorize commits
          BREAKING=""
          ADDED=""
          CHANGED=""
          FIXED=""
          SECURITY=""

          # Build parallel arrays of subjects and bodies
          SUBJECTS=()
          BODIES_ARR=()
          while IFS= read -r line; do
            [[ -z "$line" ]] && continue
            SUBJECTS+=("$line")
          done <<< "$COMMITS"

          local_idx=0
          while IFS= read -r chunk; do
            [[ -z "$chunk" ]] && continue
            BODIES_ARR+=("$chunk")
          done < <(echo "$BODIES" | awk -F'---COMMIT_SEP---' '{for(i=1;i<=NF;i++) if($i) print $i}')

          for i in "${!SUBJECTS[@]}"; do
            line="${SUBJECTS[$i]}"
            body="${BODIES_ARR[$i]:-}"

            # Check for breaking change: ! in prefix or body contains BREAKING CHANGE
            IS_BREAKING=false
            if [[ "$line" =~ ^[a-z]+(\(.+\))?!: ]]; then
              IS_BREAKING=true
            fi
            if [[ "$body" == *"BREAKING CHANGE"* ]]; then
              IS_BREAKING=true
            fi

            # Strip prefix for description
            DESC="$line"
            if [[ "$line" =~ ^[a-z]+(\(.+\))?!?: ]]; then
              DESC="${line#*: }"
            fi

            if [[ "$line" =~ ^security(\(.+\))?: ]]; then
              SECURITY="${SECURITY}- ${DESC}"$'\n'
            elif [[ "$line" =~ ^feat(\(.+\))?!?: ]]; then
              ADDED="${ADDED}- ${DESC}"$'\n'
            elif [[ "$line" =~ ^fix(\(.+\))?!?: ]]; then
              FIXED="${FIXED}- ${DESC}"$'\n'
            elif [[ "$line" =~ ^(refactor|perf)(\(.+\))?!?: ]]; then
              CHANGED="${CHANGED}- ${DESC}"$'\n'
            elif [[ "$line" =~ ^(docs|chore|ci|test|style)(\(.+\))?!?: ]]; then
              : # skip
            else
              CHANGED="${CHANGED}- ${line}"$'\n'
            fi

            if [[ "$IS_BREAKING" == true ]]; then
              BREAKING="${BREAKING}- ${DESC}"$'\n'
            fi
          done

          # Build release notes
          {
            echo "## Lettura ${VERSION}"
            echo ""
            [[ -n "$BREAKING" ]] && { echo "### Breaking Changes"; echo "$BREAKING"; echo ""; }
            [[ -n "$ADDED" ]]    && { echo "### Added"; echo "$ADDED"; echo ""; }
            [[ -n "$CHANGED" ]]  && { echo "### Changed"; echo "$CHANGED"; echo ""; }
            [[ -n "$FIXED" ]]    && { echo "### Fixed"; echo "$FIXED"; echo ""; }
            [[ -n "$SECURITY" ]] && { echo "### Security"; echo "$SECURITY"; echo ""; }
            echo "### Docker"
            echo ""
            echo "\`\`\`"
            echo "docker pull ghcr.io/${{ github.repository }}:${VERSION_NUM}"
            echo "\`\`\`"
            echo ""
            echo "### CLI Install"
            echo ""
            echo "\`\`\`"
            echo "curl -fsSL https://raw.githubusercontent.com/${{ github.repository }}/main/scripts/install-cli.sh | bash"
            echo "\`\`\`"
            echo ""
            echo "### Browser Extension"
            echo ""
            echo "Download \`lettura-extension-v${EXT_VERSION}.zip\` from assets below."
            echo ""
            echo "---"
            echo ""
            if [[ -n "$PREV_TAG" ]]; then
              echo "**Full Changelog**: https://github.com/${{ github.repository }}/compare/${PREV_TAG}...${VERSION}"
            fi
          } > release_notes.md

      - name: Download CLI artifacts
        uses: actions/download-artifact@v4
        with:
          pattern: cli-*
          path: cli-artifacts
          merge-multiple: true

      - name: Download extension artifact
        uses: actions/download-artifact@v4
        with:
          name: extension
          path: extension-artifacts

      - name: Create GitHub Release
        uses: softprops/action-gh-release@v2
        with:
          tag_name: ${{ steps.get_version.outputs.version }}
          name: ${{ steps.get_version.outputs.version }}
          body_path: release_notes.md
          draft: false
          prerelease: ${{ contains(steps.get_version.outputs.version, '-') }}
          files: |
            cli-artifacts/*.tar.gz
            extension-artifacts/*.zip
          token: ${{ secrets.GITHUB_TOKEN }}
```

- [ ] **Step 2: Validate YAML syntax**

Run: `python3 -c "import yaml; yaml.safe_load(open('.github/workflows/release.yml'))"`
Expected: No errors

- [ ] **Step 3: Commit**

```bash
git add .github/workflows/release.yml
git commit -m "feat: rewrite release workflow with Docker, extension, and categorized release notes"
```

---

### Task 5: End-to-end verification

**Files:** None (verification only)

- [ ] **Step 1: Verify release.sh dry-run works correctly**

Run: `./scripts/release.sh 0.2.0 --dry-run`
Expected: Shows version change from 0.1.0 to 0.2.0, files to update, changelog entry, and git operations

- [ ] **Step 2: Verify release.sh rejects invalid versions**

Run: `./scripts/release.sh 1.0.0a --dry-run`
Expected: Error message about invalid semver format

Run: `./scripts/release.sh 0.0.1 --dry-run`
Expected: Error message about version not being greater than current

- [ ] **Step 3: Verify release.sh with --ext-version in dry-run**

Run: `./scripts/release.sh 0.2.0 --ext-version 1.3.0 --dry-run --no-push`
Expected: Shows both main and ext version changes, no-push flag reflected

- [ ] **Step 4: Verify extension build still works with dynamic version**

Run: `cd extension && npm run build`
Expected: Build succeeds, `dist/manifest.json` contains correct version from `package.json`

- [ ] **Step 5: Verify skill file is valid**

Run: `head -5 .claude/skills/release/SKILL.md`
Expected: Shows frontmatter with name and description

- [ ] **Step 6: Verify CI workflow YAML is valid**

Run: `python3 -c "import yaml; yaml.safe_load(open('.github/workflows/release.yml'))"`
Expected: No errors