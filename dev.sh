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
  build     Rebuild image and restart (default)
  up        Start services (no rebuild)
  down      Stop and remove containers
  restart   Restart app container (no rebuild)
  logs      Tail all service logs
  status    Show container status and health
  psql      Open psql shell
  clean     Remove build artifacts and volumes
EOF
}

cmd_build() {
  log "Rebuilding image..."
  docker compose build --pull lettura 2>&1 | tail -1
  log "Restarting services..."
  docker compose up -d
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

case "${1:-build}" in
  build)   cmd_build   ;;
  up)      cmd_up      ;;
  down)    cmd_down    ;;
  restart) cmd_restart ;;
  logs)    cmd_logs    ;;
  status)  cmd_status  ;;
  psql)    cmd_psql    ;;
  clean)   cmd_clean   ;;
  -h|--help|help) usage ;;
  *)       err "Unknown command: $1"; usage; exit 1 ;;
esac
