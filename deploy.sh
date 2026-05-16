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

# ─── Build CLI + pull Tauri → downloads/ ────────────────

build_downloads() {
  echo ""
  echo "=== [downloads] Cross-compiling CLI + Tauri ==="
  ssh $SSH_OPTS "$SERVER" "
    set -e
    cd $REMOTE_DIR
    mkdir -p downloads

    # Ensure Rust is available
    if ! command -v cargo &>/dev/null; then
      curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y
      source \$HOME/.cargo/env
    fi

    # ── CLI: cross-compile Linux x86_64 + aarch64 ──────
    echo '🔨 Building CLI (x86_64-unknown-linux-gnu)...'
    rustup target add x86_64-unknown-linux-gnu 2>/dev/null || true
    cargo build --release -p trending-agent-cli --target x86_64-unknown-linux-gnu
    cp target/x86_64-unknown-linux-gnu/release/openatrading downloads/openatrading-x86_64-unknown-linux-gnu
    echo '✅ CLI x86_64 done'

    echo '🔨 Building CLI (aarch64-unknown-linux-gnu)...'
    if ! dpkg -l gcc-aarch64-linux-gnu &>/dev/null; then
      apt-get update -qq && apt-get install -y -qq gcc-aarch64-linux-gnu 2>/dev/null || true
    fi
    rustup target add aarch64-unknown-linux-gnu 2>/dev/null || true
    cargo build --release -p trending-agent-cli --target aarch64-unknown-linux-gnu && \
      cp target/aarch64-unknown-linux-gnu/release/openatrading downloads/openatrading-aarch64-unknown-linux-gnu && \
      echo '✅ CLI aarch64 done' || echo '⚠ CLI aarch64 skipped'

    # ── Tauri: Linux builds ────────────────────────────
    echo '🔨 Building Tauri (Linux)...'
    if cargo tauri build --target x86_64-unknown-linux-gnu 2>&1 | tail -3; then
      cp src-tauri/target/x86_64-unknown-linux-gnu/release/bundle/deb/*.deb downloads/ 2>/dev/null || true
      cp src-tauri/target/x86_64-unknown-linux-gnu/release/bundle/appimage/*.AppImage downloads/ 2>/dev/null || true
      echo '✅ Tauri Linux done'
    else
      echo '⚠ Tauri Linux skipped (need webkit2gtk dev libs)'
    fi

    # ── Tauri: download macOS/Windows from GitHub ──────
    echo '⬇ Fetching macOS/Windows Tauri from GitHub Releases...'
    AUTH=''
    if [ -n \"\${GITHUB_TOKEN:-}\" ]; then
      AUTH=\"-H 'Authorization: Bearer '\$GITHUB_TOKEN''\"
    fi
    LATEST_TAG=\$(curl -fsSL \$AUTH https://api.github.com/repos/oouxx/openatrading/releases/latest 2>/dev/null | grep '\"tag_name\"' | head -1 | cut -d'\"' -f4 || echo '')
    if [ -n \"\$LATEST_TAG\" ]; then
      echo \"Latest release: \$LATEST_TAG\"
      ASSETS=\$(curl -fsSL \$AUTH \"https://api.github.com/repos/oouxx/openatrading/releases/tags/\$LATEST_TAG\" 2>/dev/null | grep '\"browser_download_url\"' | cut -d'\"' -f4 || echo '')
      for url in \$ASSETS; do
        name=\$(basename \"\$url\")
        case \"\$name\" in
          *.dmg|*.msi|*.exe)
            echo \"  ⬇ \$name\"
            curl -fsSL \$AUTH -o \"downloads/\$name\" \"\$url\" 2>/dev/null || echo \"  ⚠ failed\"
            ;;
        esac
      done
    else
      echo '⚠ No GitHub releases found'
    fi
    echo '✅ downloads ready'
    ls -lh downloads/
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
