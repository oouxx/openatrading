#!/bin/bash
set -e

SERVICE="${1:-all}"
SERVER="root@203.0.113.10"
REMOTE_DIR="/root/openatrading"
SSH_OPTS="-o StrictHostKeyChecking=accept-new"

usage() {
  echo "Usage: ./deploy.sh [gateway|web|all]"
  echo "  gateway     Build & deploy gateway container only"
  echo "  web         Build & deploy frontend container only"
  echo "  all         Build & deploy everything (default)"
  exit 1
}

case "$SERVICE" in
  gateway|web|all) ;;
  *) usage ;;
esac

if [ ! -f ".env" ]; then
  echo "ERROR: .env file not found."
  exit 1
fi

# Source .env to get GITHUB_TOKEN (fallback to existing env var)
set -a
source .env
set +a

# Default deploy branch is main; override with DEPLOY_BRANCH=<branch>.
BRANCH="${DEPLOY_BRANCH:-main}"

echo "=== Deploying: $SERVICE → $SERVER (branch: $BRANCH) ==="

REPO_URL="git@github.com:oouxx/openatrading.git"

clone_or_pull() {
  echo ""
  echo "=== [git] Fetching latest code from GitHub ==="
  ssh $SSH_OPTS "$SERVER" "
    if [ -d $REMOTE_DIR/.git ]; then
      cd $REMOTE_DIR && git fetch origin && git reset --hard origin/$BRANCH
      echo '✅ git pull done (branch: $BRANCH)'
    elif [ -d $REMOTE_DIR ]; then
      rm -rf $REMOTE_DIR
      git clone -b $BRANCH $REPO_URL $REMOTE_DIR
      echo '✅ old dir removed, git clone done'
    else
      git clone -b $BRANCH $REPO_URL $REMOTE_DIR
      echo '✅ git clone done'
    fi
  "
  # Upload .env separately (not in git); also mirror to ~/.atrading for host-side tooling.
  echo "=== [env] Uploading .env ==="
  scp $SSH_OPTS .env "$SERVER:$REMOTE_DIR/.env"
  ssh $SSH_OPTS "$SERVER" "mkdir -p /root/.atrading && cp $REMOTE_DIR/.env /root/.atrading/.env"
  echo '✅ .env uploaded (and copied to /root/.atrading/)'
}

deploy_compose() {
  echo ""
  echo "=== [compose] Building & starting: $SERVICE ==="
  case "$SERVICE" in
    gateway) ssh $SSH_OPTS "$SERVER" "cd $REMOTE_DIR && docker compose up -d --build gateway" ;;
    web)     ssh $SSH_OPTS "$SERVER" "cd $REMOTE_DIR && docker compose up -d --build frontend" ;;
    all)     ssh $SSH_OPTS "$SERVER" "cd $REMOTE_DIR && docker compose up -d --build" ;;
  esac
  echo "✅ compose done ($SERVICE)"
}

clone_or_pull
deploy_compose

echo ""
echo "=== Deploy complete ($SERVICE, branch $BRANCH) ==="
