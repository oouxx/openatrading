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

# Source .env to get GITHUB_TOKEN (fallback to existing env var)
set -a
source .env
set +a

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

    # ── Try Tauri from GitHub Releases ────────────────
    echo '⬇ Fetching Tauri clients from GitHub Releases...'
    AUTH=''
    if [ -n \"\${GITHUB_TOKEN:-}\" ]; then
      AUTH=\"-H 'Authorization: Bearer '\$GITHUB_TOKEN''\"
    fi
    RELEASE_JSON=\$(curl -fsSL \$AUTH https://api.github.com/repos/oouxx/openatrading/releases/latest 2>/dev/null || echo '{}')
    LATEST_TAG=\$(echo \"\$RELEASE_JSON\" | grep '\"tag_name\"' | head -1 | cut -d'\"' -f4 || echo '')
    if [ -n \"\$LATEST_TAG\" ]; then
      echo \"Latest release: \$LATEST_TAG\"
      # Parse asset IDs and names from JSON, download each via asset API
      echo \"\$RELEASE_JSON\" | grep -E '\"(id|name)\":' | paste - - 2>/dev/null | while read line; do
        ASSET_ID=\$(echo \"\$line\" | grep -o '\"id\": [0-9]*' | grep -o '[0-9]*')
        ASSET_NAME=\$(echo \"\$line\" | grep -o '\"name\": \"[^\"]*\"' | cut -d'\"' -f4)
        if [ -n \"\$ASSET_ID\" ] && [ -n \"\$ASSET_NAME\" ]; then
          echo \"  ⬇ \$ASSET_NAME\"
          curl -fsSL \$AUTH -H 'Accept: application/octet-stream' \
            -o \"downloads/\$ASSET_NAME\" \
            \"https://api.github.com/repos/oouxx/openatrading/releases/assets/\$ASSET_ID\" 2>/dev/null || echo \"  ⚠ failed\"
        fi
      done
    else
      echo '⚠ No GitHub releases found'
    fi
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

    if [ -f \"\$SRC\" ]; then
      cp \"\$SRC\" /usr/local/bin/openatrading
      chmod +x /usr/local/bin/openatrading
      echo \"✅ CLI updated: /usr/local/bin/openatrading (\$SERVER_ARCH)\"
      /usr/local/bin/openatrading --version 2>/dev/null || true
    else
      echo \"⚠ \$SRC not found, skipping CLI update\"
    fi

    # Restart systemd gateway if installed
    if systemctl --user is-enabled openatrading 2>/dev/null; then
      echo '🔄 Restarting gateway...'
      systemctl --user restart openatrading
      echo '✅ Gateway restarted'
      sleep 1
      systemctl --user status openatrading --no-pager 2>/dev/null || true
    else
      echo '⚠ systemd service openatrading not enabled, skip restart'
    fi
  "
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
    deploy_gateway
    deploy_web
    ;;
esac

echo ""
echo "=== Deploy complete ($SERVICE) ==="
