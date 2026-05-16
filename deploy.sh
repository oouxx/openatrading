#!/bin/bash
set -e

SERVICE="${1:-all}"  # cli | gateway | web | all
SERVER="root@100.101.88.117"
REMOTE_DIR="/root/openatrading"
SSH_OPTS="-o StrictHostKeyChecking=accept-new"

usage() {
  echo "Usage: ./deploy.sh [cli|gateway|web|all]"
  echo "  cli        Build & deploy CLI gateway only"
  echo "  gateway    Same as cli"
  echo "  web        Build & deploy web backend + frontend"
  echo "  all        Build & deploy everything (default)"
  exit 1
}

case "$SERVICE" in
  cli) SERVICE=gateway ;;
  gateway|web|all) ;;
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

# ─── Gateway ─────────────────────────────────────────────

deploy_gateway() {
  echo ""
  echo "=== [gateway] Building & deploying openatrading gateway ==="
  ssh $SSH_OPTS "$SERVER" "cd $REMOTE_DIR && docker compose build && docker compose up -d"
}

# ─── Web ─────────────────────────────────────────────────

deploy_web() {
  echo ""
  echo "=== [web] Building & deploying web services ==="
  ssh $SSH_OPTS "$SERVER" "cd $REMOTE_DIR && docker compose -f docker-compose-web.yaml build"
  ssh $SSH_OPTS "$SERVER" "cd $REMOTE_DIR && docker compose -f docker-compose-web.yaml up -d"
}

# ─── Execute ────────────────────────────────────────────

sync_source

case "$SERVICE" in
  gateway)
    deploy_gateway
    ;;
  web)
    deploy_web
    ;;
  all)
    deploy_gateway
    deploy_web
    ;;
esac

echo ""
echo "=== Deploy complete ($SERVICE) ==="
