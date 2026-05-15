#!/bin/bash
set -e

SERVICE="${1:-all}"  # backend | frontend | all
SERVER="root@107.172.238.76"
REMOTE_DIR="/root/trending-agent"
SSH_OPTS="-o StrictHostKeyChecking=accept-new"

usage() {
  echo "Usage: ./deploy-racknerd.sh [backend|frontend|all]"
  echo "  backend   Build & deploy backend only"
  echo "  frontend  Build & deploy frontend only"
  echo "  all       Build & deploy both (default)"
  exit 1
}

case "$SERVICE" in
  backend|frontend|all) ;;
  *) usage ;;
esac

if [ ! -f ".env" ]; then
  echo "ERROR: .env file not found."
  exit 1
fi

echo "=== Deploying: $SERVICE → $SERVER ==="

# ─── Upload source ──────────────────────────────────────

sync_source() {
  echo ""
  echo "=== [sync] Syncing source code to server ==="
  ssh $SSH_OPTS "$SERVER" "mkdir -p $REMOTE_DIR"
  rsync -avz -e "ssh $SSH_OPTS" --delete \
    --exclude='target/' \
    --exclude='node_modules/' \
    --exclude='dist/' \
    --exclude='.git/' \
    --exclude='docker-images/' \
    --exclude='*.tar.gz' \
    --exclude='*.ico' \
    --exclude='*.icns' \
    --exclude='.DS_Store' \
    ./ "$SERVER:$REMOTE_DIR/"
}

# ─── Build on server ────────────────────────────────────

build_on_server() {
  local target="$1"
  echo ""
  echo "=== [build] Building $target on server ==="
  ssh $SSH_OPTS "$SERVER" "cd $REMOTE_DIR && docker compose build $target"
}

# ─── Deploy ─────────────────────────────────────────────

deploy_service() {
  local target="$1"
  echo ""
  echo "=== [deploy] Restarting $target ==="
  ssh $SSH_OPTS "$SERVER" "cd $REMOTE_DIR && docker compose up -d --no-deps $target"
}

deploy_all_services() {
  echo ""
  echo "=== [deploy] Restarting all services ==="
  ssh $SSH_OPTS "$SERVER" "cd $REMOTE_DIR && docker compose down && docker compose up -d"
}

# ─── Execute ────────────────────────────────────────────

sync_source

case "$SERVICE" in
  backend)
    build_on_server backend
    deploy_service backend
    ;;
  frontend)
    build_on_server frontend
    deploy_service frontend
    ;;
  all)
    build_on_server backend
    build_on_server frontend
    deploy_all_services
    ;;
esac

echo ""
echo "=== Deploy complete ($SERVICE) ==="
