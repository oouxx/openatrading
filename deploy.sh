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

REPO_URL="https://github.com/oouxx/openatrading.git"

# ─── Clone / pull from GitHub ────────────────────────────

clone_or_pull() {
  echo ""
  echo "=== [git] Fetching latest code from GitHub ==="
  ssh $SSH_OPTS "$SERVER" "
    if [ -d $REMOTE_DIR/.git ]; then
      cd $REMOTE_DIR && git fetch origin && git reset --hard origin/main
      echo '✅ git pull done'
    else
      git clone $REPO_URL $REMOTE_DIR
      echo '✅ git clone done'
    fi
  "
  # Upload .env separately (not in git)
  echo "=== [env] Uploading .env ==="
  scp $SSH_OPTS .env "$SERVER:$REMOTE_DIR/.env"
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

clone_or_pull

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
