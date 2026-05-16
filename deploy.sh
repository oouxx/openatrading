#!/bin/bash
set -e

SERVICE="${1:-all}"  # cli | gateway | web | all
SERVER="root@203.0.113.12"
REMOTE_DIR="/root/openatrading"
SSH_OPTS="-o StrictHostKeyChecking=accept-new"
REPO="oouxx/openatrading"

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

# ─── Build binaries for downloads ────────────────────────

build_binaries() {
  echo ""
  echo "=== [build] Building CLI binary ==="
  # Build CLI on server
  local arch
  arch=$(ssh $SSH_OPTS "$SERVER" "uname -m")
  local target
  case "$arch" in
    x86_64)  target="x86_64-unknown-linux-gnu" ;;
    aarch64) target="aarch64-unknown-linux-gnu" ;;
    *) echo "Unknown arch: $arch"; exit 1 ;;
  esac

  ssh $SSH_OPTS "$SERVER" "
    set -e
    cd $REMOTE_DIR
    if ! command -v cargo &>/dev/null; then
      curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y
      source \$HOME/.cargo/env
    fi
    cargo build --release -p trending-agent-cli --target $target
    cp target/$target/release/openatrading downloads/openatrading-$target
    echo '✅ CLI binary built'
  "

  echo ""
  echo "=== [build] Downloading Tauri desktop clients ==="
  ssh $SSH_OPTS "$SERVER" "
    set -e
    cd $REMOTE_DIR/downloads
    LATEST_TAG=\$(curl -fsSL https://api.github.com/repos/$REPO/releases/latest | grep '\"tag_name\"' | head -1 | cut -d'\"' -f4)
    echo \"Latest: \$LATEST_TAG\"

    # Download Tauri bundles
    for target in x86_64-apple-darwin aarch64-apple-darwin x86_64-unknown-linux-gnu; do
      # Try .dmg first (macOS), then .deb (Linux), then .AppImage
      url=\"https://github.com/$REPO/releases/download/\$LATEST_TAG/trending-agent_\${target}.dmg\"
      status=\$(curl -fsSL -o /dev/null -w '%{http_code}' \"\$url\" 2>/dev/null || echo '404')
      if [ \"\$status\" = '200' ]; then
        curl -fsSL \"\$url\" -O && echo \"  ✔ \${target}.dmg\"
        continue
      fi
      url=\"https://github.com/$REPO/releases/download/\$LATEST_TAG/trending-agent_\${target}.deb\"
      status=\$(curl -fsSL -o /dev/null -w '%{http_code}' \"\$url\" 2>/dev/null || echo '404')
      if [ \"\$status\" = '200' ]; then
        curl -fsSL \"\$url\" -O && echo \"  ✔ \${target}.deb\"
        continue
      fi
      url=\"https://github.com/$REPO/releases/download/\$LATEST_TAG/trending-agent_\${target}.AppImage\"
      status=\$(curl -fsSL -o /dev/null -w '%{http_code}' \"\$url\" 2>/dev/null || echo '404')
      if [ \"\$status\" = '200' ]; then
        curl -fsSL \"\$url\" -O && echo \"  ✔ \${target}.AppImage\"
        continue
      fi
      echo \"  ⚠ \${target}: no release asset found\"
    done
    echo '✅ Desktop clients synced'
  "
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
  ssh $SSH_OPTS "$SERVER" "mkdir -p $REMOTE_DIR/downloads"
  ssh $SSH_OPTS "$SERVER" "cd $REMOTE_DIR && docker compose -f docker-compose-web.yaml build"
  ssh $SSH_OPTS "$SERVER" "cd $REMOTE_DIR && docker compose -f docker-compose-web.yaml up -d"
}

# ─── Execute ────────────────────────────────────────────

sync_source

case "$SERVICE" in
  gateway)
    build_binaries
    deploy_gateway
    ;;
  web)
    build_binaries
    deploy_web
    ;;
  all)
    build_binaries
    deploy_gateway
    deploy_web
    ;;
esac

echo ""
echo "=== Deploy complete ($SERVICE) ==="
