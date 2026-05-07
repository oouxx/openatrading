#!/bin/bash
set -e

SERVICE="${1:-all}"  # backend | frontend | all
SERVER="root@47.116.170.125"
REMOTE_DIR="/root/trending-agent"
IMAGES_DIR="./docker-images"

usage() {
  echo "Usage: ./deploy-ali.sh [backend|frontend|all]"
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

rm -rf "$IMAGES_DIR"
mkdir -p "$IMAGES_DIR"

# ─── Build ────────────────────────────────────────────

build_backend() {
  echo ""
  echo "=== [build] Building backend image (linux/amd64) ==="
  docker buildx build --platform linux/amd64 \
    -t trending-agent-backend:latest \
    -f Dockerfile.backend \
    -o type=docker .
}

build_frontend() {
  echo ""
  echo "=== [build] Building frontend image (linux/amd64) ==="
  docker buildx build --platform linux/amd64 \
    -t trending-agent-frontend:latest \
    -f Dockerfile.frontend \
    -o type=docker .
}

# ─── Save ──────────────────────────────────────────────

save_backend() {
  echo ""
  echo "=== [save] Saving backend image ==="
  docker save trending-agent-backend:latest | gzip > "$IMAGES_DIR/backend.tar.gz"
}

save_frontend() {
  echo ""
  echo "=== [save] Saving frontend image ==="
  docker save trending-agent-frontend:latest | gzip > "$IMAGES_DIR/frontend.tar.gz"
}

# ─── Upload ────────────────────────────────────────────

upload() {
  echo ""
  echo "=== [upload] Uploading to server ==="
  ssh "$SERVER" "mkdir -p $REMOTE_DIR"

  local files=""
  [ -f "$IMAGES_DIR/backend.tar.gz" ] && files="$files $IMAGES_DIR/backend.tar.gz"
  [ -f "$IMAGES_DIR/frontend.tar.gz" ] && files="$files $IMAGES_DIR/frontend.tar.gz"

  if [ -n "$files" ]; then
    scp $files "$SERVER:$REMOTE_DIR/"
  fi

  # Always upload config files (docker-compose, nginx, .env)
  scp docker-compose.yaml nginx.conf .env "$SERVER:$REMOTE_DIR/"
}

# ─── Load & Start ──────────────────────────────────────

deploy_backend() {
  echo ""
  echo "=== [deploy] Loading backend image & restarting ==="
  ssh "$SERVER" "cd $REMOTE_DIR && \
    gunzip -c backend.tar.gz | docker load && \
    rm -f backend.tar.gz && \
    docker compose up -d --no-deps backend"
}

deploy_frontend() {
  echo ""
  echo "=== [deploy] Loading frontend image & restarting ==="
  ssh "$SERVER" "cd $REMOTE_DIR && \
    gunzip -c frontend.tar.gz | docker load && \
    rm -f frontend.tar.gz && \
    docker compose up -d --no-deps frontend"
}

deploy_all() {
  echo ""
  echo "=== [deploy] Loading images & starting all services ==="
  ssh "$SERVER" "cd $REMOTE_DIR && \
    gunzip -c backend.tar.gz | docker load && \
    gunzip -c frontend.tar.gz | docker load && \
    rm -f backend.tar.gz frontend.tar.gz && \
    docker compose down && \
    docker compose up -d"
}

# ─── Execute ───────────────────────────────────────────

case "$SERVICE" in
  backend)
    build_backend
    save_backend
    upload
    deploy_backend
    ;;
  frontend)
    build_frontend
    save_frontend
    upload
    deploy_frontend
    ;;
  all)
    build_backend
    build_frontend
    save_backend
    save_frontend
    upload
    deploy_all
    ;;
esac

# Cleanup
rm -rf "$IMAGES_DIR"

echo ""
echo "=== Deploy complete ($SERVICE) ==="
echo "Frontend: http://47.116.170.125:18080"
