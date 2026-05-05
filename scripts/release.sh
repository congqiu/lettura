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

# ── Export variables for python subprocesses ─────────────────────────
export WORKSPACE_ROOT

# ── Read current version ────────────────────────────────────────────
CURRENT_VERSION="$(python3 -c "
import os, re
with open(os.path.join(os.environ['WORKSPACE_ROOT'], 'Cargo.toml'), 'r') as f:
    content = f.read()
m = re.search(r'\[workspace\.package\][^\[]*?version\s*=\s*\"([^\"]+)\"', content, re.DOTALL)
print(m.group(1) if m else '')
")"
info "current version: $CURRENT_VERSION"

# ── Export version variables for python subprocesses ─────────────────
export CURRENT_VERSION NEW_VERSION

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
  CURRENT_EXT_VERSION="$(python3 -c "import os,json; print(json.load(open(os.path.join(os.environ['WORKSPACE_ROOT'],'extension','package.json')))['version'])")"
  version_gt "$EXT_VERSION" "$CURRENT_EXT_VERSION" || err "ext version $EXT_VERSION is not greater than current $CURRENT_EXT_VERSION"
  export EXT_VERSION CURRENT_EXT_VERSION
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
    if [[ "$msg" =~ ^[a-z]+(\(.+\))?!: ]]; then
      is_breaking=true
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

  # Also check bodies for BREAKING CHANGE
  local subject_arr=()
  while IFS= read -r line; do
    [[ -z "$line" ]] && continue
    subject_arr+=("$line")
  done <<< "$commits"

  local body_chunks=()
  while IFS= read -r chunk; do
    [[ -z "$chunk" ]] && continue
    body_chunks+=("$chunk")
  done < <(echo "$bodies" | awk -F'---COMMIT_SEP---' '{for(i=1;i<=NF;i++) if($i) print $i}')

  for i in "${!subject_arr[@]}"; do
    if [[ "${body_chunks[$i]:-}" == *"BREAKING CHANGE"* ]]; then
      local desc="${subject_arr[$i]}"
      if [[ "$desc" =~ ^[a-z]+(\(.+\))?!?: ]]; then
        desc="${desc#*: }"
      fi
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

HEAD_BEFORE="$(git -C "$WORKSPACE_ROOT" rev-parse HEAD)"
info "HEAD before release: ${HEAD_BEFORE:0:8}"

# ── Update version numbers ──────────────────────────────────────────
info "updating version to $NEW_VERSION ..."

# Cargo.toml — workspace.package.version
# Use python for reliable multi-line replacement in TOML
python3 -c "
import os, re
path = os.path.join(os.environ['WORKSPACE_ROOT'], 'Cargo.toml')
with open(path, 'r') as f:
    content = f.read()
content = re.sub(
    r'(\[workspace\.package\][^\[]*?)version\s*=\s*\"' + os.environ['CURRENT_VERSION'] + r'\"',
    r'\1version = \"' + os.environ['NEW_VERSION'] + r'\"',
    content,
    count=1,
    flags=re.DOTALL
)
with open(path, 'w') as f:
    f.write(content)
"
ok "Cargo.toml updated"

# web/package.json
python3 -c "
import os, json
p = os.path.join(os.environ['WORKSPACE_ROOT'], 'web', 'package.json')
with open(p, 'r') as f:
    pkg = json.load(f)
pkg['version'] = os.environ['NEW_VERSION']
with open(p, 'w') as f:
    json.dump(pkg, f, indent=2)
    f.write('\n')
"
ok "web/package.json updated"

# extension/package.json (only if --ext-version)
if [[ -n "$EXT_VERSION" ]]; then
  python3 -c "
  import os, json
  p = os.path.join(os.environ['WORKSPACE_ROOT'], 'extension', 'package.json')
  with open(p, 'r') as f:
      pkg = json.load(f)
  pkg['version'] = os.environ['EXT_VERSION']
  with open(p, 'w') as f:
      json.dump(pkg, f, indent=2)
      f.write('\n')
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
export CHANGELOG_PATH CHANGELOG_TMP

python3 -c "
import os, sys

changelog_path = os.environ['CHANGELOG_PATH']
entry_path = os.environ['CHANGELOG_TMP']

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

# ── Rollback hints (on failure) ──────────────────────────────────────
# If the script fails after modifying files:
#   - Version files changed but no commit:  git checkout -- Cargo.toml web/package.json [extension/package.json]
#   - Changelog changed but no commit:      git checkout -- CHANGELOG.md
#   - Commit created but no tag:            git reset --soft HEAD~1
#   - Tag created but not pushed:           git tag -d v${NEW_VERSION}
#   - Tag pushed to remote:                 git push origin :refs/tags/v${NEW_VERSION}

ok "release v${NEW_VERSION} complete!"
