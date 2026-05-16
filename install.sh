#!/bin/bash
set -e

REPO="oouxx/openatrading"
BIN_NAME="openatrading"
INSTALL_DIR="${INSTALL_DIR:-/usr/local/bin}"

# ── Detect platform ────────────────────────────────────

UNAME_S=$(uname -s)
UNAME_M=$(uname -m)

if [ "$UNAME_S" != "Linux" ]; then
    echo "❌ openatrading 仅支持 Linux (x86_64 / aarch64)"
    exit 1
fi

case "$UNAME_M" in
    x86_64)  ASSET="openatrading-x86_64-unknown-linux-gnu" ;;
    aarch64) ASSET="openatrading-aarch64-unknown-linux-gnu" ;;
    arm64)   ASSET="openatrading-aarch64-unknown-linux-gnu" ;;
    *)       echo "❌ 不支持的架构: $UNAME_M"; exit 1 ;;
esac

# ── Auth header ──────────────────────────────────────────

AUTH=()
if [ -n "${GITHUB_TOKEN:-}" ]; then
    AUTH=(-H "Authorization: Bearer ${GITHUB_TOKEN}")
fi

# ── Fetch release info ───────────────────────────────────

echo "⬇ 正在获取最新版本..."
RELEASE_URL="https://api.github.com/repos/${REPO}/releases/latest"
RELEASE_JSON=$(curl -fsSL "${AUTH[@]}" "$RELEASE_URL")

TAG=$(echo "$RELEASE_JSON" | grep '"tag_name"' | head -1 | cut -d '"' -f 4)
echo "📦 版本: $TAG"

# Find asset ID by name
ASSET_ID=$(echo "$RELEASE_JSON" | grep -B 3 "\"name\": \"$ASSET\"" | grep '"id"' | head -1 | grep -o '[0-9]*')

if [ -z "$ASSET_ID" ]; then
    echo "❌ 未找到适用于 $ASSET 的二进制文件"
    exit 1
fi
echo "📦 平台: $ASSET (asset #$ASSET_ID)"

# ── Download via asset API (works for private repos) ───

TMP_DIR=$(mktemp -d)
trap 'rm -rf "$TMP_DIR"' EXIT

echo "⬇ 正在下载..."
curl -fsSL "${AUTH[@]}" -H "Accept: application/octet-stream" \
    "https://api.github.com/repos/${REPO}/releases/assets/${ASSET_ID}" \
    -o "$TMP_DIR/$BIN_NAME"

# ── Install ─────────────────────────────────────────────

if [ -w "$INSTALL_DIR" ]; then
    mv "$TMP_DIR/$BIN_NAME" "$INSTALL_DIR/$BIN_NAME"
else
    sudo mv "$TMP_DIR/$BIN_NAME" "$INSTALL_DIR/$BIN_NAME"
fi
chmod +x "$INSTALL_DIR/$BIN_NAME"

echo ""
echo "✅ openatrading 已安装到 $INSTALL_DIR/$BIN_NAME"
echo "   运行 openatrading --help 开始使用"
