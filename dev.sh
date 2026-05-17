#!/usr/bin/env bash
set -euo pipefail

cd "$(dirname "$0")"

# Colors
G='\033[0;32m' Y='\033[0;33m' R='\033[0;31m' B='\033[0;34m' N='\033[0m'

log()  { echo -e "${G}[dev]${N} $*"; }
warn() { echo -e "${Y}[dev]${N} $*"; }
err()  { echo -e "${R}[dev]${N} $*" >&2; }

usage() {
  cat <<EOF
${B}Usage:${N} $(basename "$0") <command>

${B}Commands:${N}
  dev          Start backend (Docker) + frontend (Vite) with combined logs
  build        Rebuild image and restart
  up           Start services (no rebuild)
  down         Stop and remove containers
  restart      Restart app container (no rebuild)
  logs         Tail all service logs
  status       Show container status and health
  psql         Open psql shell
  cache-stats  Show sccache stats + build volume sizes
  clean-inc    Prune cargo incremental caches (keeps deps + sccache)
  clean-cache  Wipe ALL build caches (target, sccache, registry) — slow next build
  clean        Remove containers, images, and volumes
EOF
}

cmd_dev() {
  if [[ ! -d "web/node_modules" ]]; then
    warn "web/node_modules not found. Run 'npm install' in web/ first."
    warn "Starting backend only..."
    docker compose up -d
    cmd_status
    return
  fi

  log "Starting backend..."
  docker compose up -d

  log "Starting frontend (Vite)..."
  (cd web && npm run dev) &
  local vite_pid=$!

  cleanup() {
    kill $vite_pid 2>/dev/null || true
    docker compose stop -t 5 2>/dev/null || true
  }
  trap cleanup EXIT

  log "Dev servers running. Backend: http://localhost:3330  Frontend: http://localhost:5173"
  log "Press Ctrl+C to stop all."
  echo ""

  docker compose logs -f --tail 10
}

cmd_build() {
  log "Rebuilding image..."
  docker compose build --pull lettura 2>&1 | tail -1
  log "Restarting services..."
  docker compose up -d --force-recreate
  cmd_status
}

cmd_up() {
  docker compose up -d
  cmd_status
}

cmd_down() {
  docker compose down
}

cmd_restart() {
  log "Restarting lettura..."
  docker compose restart lettura
  cmd_status
}

cmd_logs() {
  docker compose logs -f --tail 50
}

cmd_status() {
  echo ""
  docker compose ps --format "table {{.Name}}\t{{.Status}}\t{{.Ports}}"
  echo ""
}

cmd_psql() {
  docker compose exec postgres psql -U lettura -d lettura
}

cmd_clean() {
  warn "This will remove containers, images, and volumes."
  read -rp "Continue? [y/N] " confirm
  [[ "$confirm" != "y" && "$confirm" != "Y" ]] && { log "Aborted."; exit 0; }
  docker compose down -v --rmi local
  log "Cleaned up."
}

# Print sccache hit/miss stats from the running dev container (if up) + show
# the on-disk size of cargo / sccache / target volumes so it's obvious which
# one is eating the most space.
cmd_cache_stats() {
  if docker compose ps -q lettura-dev 2>/dev/null | grep -q .; then
    log "sccache stats (inside lettura-dev container):"
    docker compose exec -T lettura-dev sccache --show-stats 2>/dev/null || \
      warn "sccache not initialized yet (no compiles have run)"
  else
    warn "lettura-dev container not running — start with './dev.sh dev' to see hit rate"
  fi

  echo ""
  log "Build volume sizes (anything matching lettura-* / cargo / target / sccache):"
  local args=""
  local found=0
  while read -r vol; do
    [[ -z "$vol" ]] && continue
    args="$args -v $vol:/v/$vol"
    found=$((found + 1))
  done < <(docker volume ls -q | grep -E 'lettura|cargo|target|sccache' || true)
  if [[ $found -eq 0 ]]; then
    warn "No matching build volumes found"
  else
    docker run --rm $args alpine sh -c 'du -sh /v/* 2>/dev/null | sort -h'
  fi

  echo ""
  log "BuildKit cache mounts (cargo registry, sccache, target — survive across docker builds):"
  docker buildx du --filter type=exec.cachemount 2>/dev/null | head -10 || \
    warn "BuildKit cache info unavailable"
}

# Drop cargo's per-crate incremental caches across all build volumes. These
# are the easiest 5-10 GB to reclaim — incremental is for "small change, fast
# rebuild" but accumulates indefinitely; sccache covers the same need with a
# hard size cap. Safe to run any time; next build just recompiles a bit slower.
cmd_clean_inc() {
  log "Pruning cargo incremental caches..."
  local cleaned=0
  for vol in $(docker volume ls -q | grep -E 'lettura.*target|^target$'); do
    log "  $vol"
    docker run --rm -v "$vol:/t" alpine sh -c '
      du -sh /t/*/incremental 2>/dev/null
      rm -rf /t/*/incremental 2>/dev/null
    ' && cleaned=$((cleaned + 1))
  done
  if [[ $cleaned -eq 0 ]]; then
    warn "No matching target volumes found"
  else
    log "Pruned incremental caches from $cleaned volume(s)"
  fi
}

# Nuclear option: drop every build cache (cargo registry, sccache, target).
# Next build takes the full 5+ minutes. Use when something is really wedged.
cmd_clean_cache() {
  warn "This wipes ALL build caches (target, sccache, cargo registry)."
  warn "Next build will take several minutes from scratch."
  read -rp "Continue? [y/N] " confirm
  [[ "$confirm" != "y" && "$confirm" != "Y" ]] && { log "Aborted."; exit 0; }
  local removed=0
  for vol in $(docker volume ls -q | grep -E 'lettura.*(target|cargo|sccache)|^(target|cargo-registry|sccache)$'); do
    log "  removing $vol"
    docker volume rm "$vol" >/dev/null && removed=$((removed + 1))
  done
  log "Removed $removed volume(s)"
}

case "${1:-build}" in
  dev)         cmd_dev         ;;
  build)       cmd_build       ;;
  up)          cmd_up          ;;
  down)        cmd_down        ;;
  restart)     cmd_restart     ;;
  logs)        cmd_logs        ;;
  status)      cmd_status      ;;
  psql)        cmd_psql        ;;
  cache-stats) cmd_cache_stats ;;
  clean-inc)   cmd_clean_inc   ;;
  clean-cache) cmd_clean_cache ;;
  clean)       cmd_clean       ;;
  -h|--help|help) usage ;;
  *)       err "Unknown command: $1"; usage; exit 1 ;;
esac
