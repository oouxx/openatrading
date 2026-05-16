#!/bin/bash
set -e

SERVICE="${1:-all}"  # cli | gateway | web | all
MODE="${2:-pull}"    # pull | build
SERVER="root@203.0.113.12"
REMOTE_DIR="/root/openatrading"
SSH_OPTS="-o StrictHostKeyChecking=accept-new"

usage() {
  echo "Usage: ./deploy.sh [cli|gateway|web|all] [pull|build]"
  echo "  cli        Build & deploy CLI gateway only"
  echo "  gateway    Same as cli"
  echo "  web        Build & deploy web backend + frontend"
  echo "  all        Build & deploy everything (default)"
  echo ""
  echo "  pull       Pull prebuilt image from ghcr.io (default)"
  echo "  build      Build image locally on server"
  exit 1
}

case "$SERVICE" in
  cli) SERVICE=gateway ;;
  gateway|web|all) ;;
  *) usage ;;
esac

case "$MODE" in
  pull) DOCKER_CMD="pull && up -d" ;;
  build) DOCKER_CMD="build && up -d" ;;
  *) usage ;;
esac

if [ ! -f ".env" ]; then
  echo "ERROR: .env file not found."
  exit 1
fi

echo "=== Deploying: $SERVICE → $SERVER ==="

REPO_URL="git@github.com:oouxx/openatrading.git"

# ─── Clone / pull from GitHub ────────────────────────────

# ─── ghcr.io login ─────────────────────────────────────

ghcr_login() {
  if [ -n "${GITHUB_TOKEN:-}" ]; then
    echo ""
    echo "=== [auth] Logging into ghcr.io ==="
    ssh $SSH_OPTS "$SERVER" "echo '$GITHUB_TOKEN' | docker login ghcr.io -u oouxx --password-stdin"
  else
    echo "⚠ GITHUB_TOKEN not set, skipping ghcr.io login"
  fi
}

# ─── Clone / pull from GitHub ────────────────────────────

clone_or_pull() {
  echo ""
  echo "=== [git] Fetching latest code from GitHub ==="
  ssh $SSH_OPTS "$SERVER" "
    if [ -d $REMOTE_DIR/.git ]; then
      cd $REMOTE_DIR && git fetch origin && git reset --hard origin/main
      echo '✅ git pull done'
    elif [ -d $REMOTE_DIR ]; then
      rm -rf $REMOTE_DIR
      git clone $REPO_URL $REMOTE_DIR
      echo '✅ old dir removed, git clone done'
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
  echo "=== [gateway] Deploying openatrading gateway (mode: $MODE) ==="
  ssh $SSH_OPTS "$SERVER" "cd $REMOTE_DIR && docker compose $DOCKER_CMD"
}

# ─── Web ─────────────────────────────────────────────────

deploy_web() {
  echo ""
  echo "=== [web] Deploying web services (mode: $MODE) ==="
  ssh $SSH_OPTS "$SERVER" "cd $REMOTE_DIR && docker compose -f docker-compose-web.yaml $DOCKER_CMD"
}

# ─── Execute ────────────────────────────────────────────

clone_or_pull
ghcr_login

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
