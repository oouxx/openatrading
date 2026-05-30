#!/bin/bash
set -e

SERVICE="${1:-gateway}"
SERVER="root@107.172.238.76"
REMOTE_DIR="/root/openatrading"
SSH_OPTS="-o StrictHostKeyChecking=accept-new"

usage() {
  echo "Usage: ./deploy.sh [gateway|web|all]"
  echo "  gateway     Build & deploy gateway (default)"
  echo "  web         Build & deploy frontend only"
  echo "  all         Build & deploy everything"
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

echo "=== Deploying: $SERVICE → $SERVER ==="

REPO_URL="git@github.com:oouxx/openatrading.git"
BRANCH="${DEPLOY_BRANCH:-$(git branch --show-current)}"

echo "=== Deploying branch: $BRANCH ==="

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
  # Upload .env separately (not in git)
  echo "=== [env] Uploading .env ==="
  scp $SSH_OPTS .env "$SERVER:$REMOTE_DIR/.env"
}

# ─── Build CLI + pull Tauri → downloads/ ────────────────

build_downloads() {
  echo ""
  echo "=== [downloads] Docker cross-compile CLI + Tauri ==="
  ssh $SSH_OPTS "$SERVER" "
    set -e
    cd $REMOTE_DIR
    mkdir -p downloads

    # ── CLI: Docker cross-compile x86_64 ──────────────
    echo '🔨 Docker build CLI (x86_64-unknown-linux-gnu)...'
    docker build -f Dockerfile --platform linux/amd64 -t openatrading-cli-amd64 .
    docker run --rm --entrypoint cp -v \$PWD/downloads:/out openatrading-cli-amd64 /usr/local/bin/openatrading /out/openatrading-x86_64-unknown-linux-gnu
    echo '✅ CLI x86_64 done'

    # ── CLI: Docker cross-compile aarch64 ─────────────
    echo '🔨 Docker build CLI (aarch64-unknown-linux-gnu)...'
    docker build -f Dockerfile --platform linux/arm64 -t openatrading-cli-arm64 . 2>/dev/null && \
      docker run --rm --entrypoint cp -v \$PWD/downloads:/out openatrading-cli-arm64 /usr/local/bin/openatrading /out/openatrading-aarch64-unknown-linux-gnu && \
      echo '✅ CLI aarch64 done' || echo '⚠ CLI aarch64 skipped (need qemu or arm64 host)'

    echo '✅ downloads ready'
    ls -lh downloads/
  "
}

# ─── Update system CLI binary ───────────────────────────

update_system_cli() {
  echo ""
  echo "=== [cli] Updating system-installed CLI ==="
  ssh $SSH_OPTS "$SERVER" "
    set -e
    cd $REMOTE_DIR

    # Detect server arch
    SERVER_ARCH=\$(uname -m)
    case \"\$SERVER_ARCH\" in
      x86_64)  SRC='downloads/openatrading-x86_64-unknown-linux-gnu' ;;
      aarch64) SRC='downloads/openatrading-aarch64-unknown-linux-gnu' ;;
      arm64)   SRC='downloads/openatrading-aarch64-unknown-linux-gnu' ;;
      *) echo '❌ Unknown arch: \$SERVER_ARCH'; exit 1 ;;
    esac

    # Stop gateway first (binary is running)
    NEED_RESTART=false
    if systemctl --user is-active openatrading 2>/dev/null; then
      echo '⏸  Stopping gateway...'
      systemctl --user stop openatrading
      NEED_RESTART=true
    fi

    if [ -f \"\$SRC\" ]; then
      cp \"\$SRC\" /usr/local/bin/openatrading
      chmod +x /usr/local/bin/openatrading
      echo \"✅ CLI updated: /usr/local/bin/openatrading (\$SERVER_ARCH)\"
      /usr/local/bin/openatrading --version 2>/dev/null || true
    else
      echo \"⚠ \$SRC not found, skipping CLI update\"
    fi

    if \$NEED_RESTART; then
      echo '🔄 Restarting gateway...'
      systemctl --user restart openatrading
      echo '✅ Gateway restarted'
      sleep 1
      systemctl --user status openatrading --no-pager 2>/dev/null || true
    fi
  "
}

# ─── Gateway ─────────────────────────────────────────────

deploy_gateway() {
  echo ""
  echo "=== [gateway] Deploying gateway ==="
  ssh $SSH_OPTS "$SERVER" "cd $REMOTE_DIR && docker compose up -d --build gateway"
}

# ─── Web ─────────────────────────────────────────────────

deploy_web() {
  echo ""
  echo "=== [web] Deploying frontend ==="
  ssh $SSH_OPTS "$SERVER" "cd $REMOTE_DIR && docker compose up -d --build frontend"
}

# ─── Execute ────────────────────────────────────────────

clone_or_pull
build_downloads

# Update system CLI and restart gateway (always, for gateway/all)
case "$SERVICE" in
  gateway|all)
    update_system_cli
    ;;
esac

case "$SERVICE" in
  gateway)
    deploy_gateway
    ;;
  web)
    deploy_web
    ;;
  all)
    ssh $SSH_OPTS "$SERVER" "cd $REMOTE_DIR && docker compose up -d --build"
    ;;
esac

echo ""
echo "=== Deploy complete ($SERVICE) ==="
