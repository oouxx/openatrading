#!/bin/bash
set -e

REPO="oouxx/openatrading"
BIN_NAME="openatrading"
INSTALL_DIR="${INSTALL_DIR:-/usr/local/bin}"
PROJECT_DIR="${PROJECT_DIR:-/etc/openatrading}"

# ── Usage ─────────────────────────────────────────────────

usage() {
  echo "Usage:"
  echo "  ./install.sh          安装 openatrading + systemd 服务"
  echo "  ./install.sh --uninstall  卸载 openatrading + systemd 服务"
  exit 0
}

UNINSTALL=false
case "${1:-}" in
  --uninstall|-u) UNINSTALL=true ;;
  --help|-h) usage ;;
esac

# ── Uninstall ─────────────────────────────────────────────

if $UNINSTALL; then
  echo "🗑  正在卸载 openatrading..."

  # 停止并禁用 systemd 服务
  if command -v systemctl &>/dev/null; then
    echo "  停止 systemd 服务..."
    systemctl --user stop "$BIN_NAME" 2>/dev/null || true
    systemctl --user disable "$BIN_NAME" 2>/dev/null || true
    rm -f "$HOME/.config/systemd/user/${BIN_NAME}.service"
    systemctl --user daemon-reload 2>/dev/null || true
  fi

  # 移除二进制
  if [ -f "$INSTALL_DIR/$BIN_NAME" ]; then
    if [ -w "$INSTALL_DIR" ]; then
      rm -f "$INSTALL_DIR/$BIN_NAME"
    else
      sudo rm -f "$INSTALL_DIR/$BIN_NAME"
    fi
    echo "  ✓ 已移除 $INSTALL_DIR/$BIN_NAME"
  fi

  # 移除项目目录
  if [ -d "$PROJECT_DIR" ]; then
    read -rp "  是否删除项目目录 $PROJECT_DIR ? [y/N] " yn
    case "$yn" in
      [yY]|[yY][eE][sS]) sudo rm -rf "$PROJECT_DIR"; echo "  ✓ 已删除 $PROJECT_DIR" ;;
      *) echo "  跳过" ;;
    esac
  fi

  # 移除用户配置
  if [ -d "$HOME/.atrading" ]; then
    read -rp "  是否删除用户配置 $HOME/.atrading ? [y/N] " yn
    case "$yn" in
      [yY]|[yY][eE][sS]) rm -rf "$HOME/.atrading"; echo "  ✓ 已删除 $HOME/.atrading" ;;
      *) echo "  跳过" ;;
    esac
  fi

  echo ""
  echo "✅ openatrading 已卸载"
  exit 0
fi

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

# ── Install binary ─────────────────────────────────────

if [ -w "$INSTALL_DIR" ]; then
    mv "$TMP_DIR/$BIN_NAME" "$INSTALL_DIR/$BIN_NAME"
else
    sudo mv "$TMP_DIR/$BIN_NAME" "$INSTALL_DIR/$BIN_NAME"
fi
chmod +x "$INSTALL_DIR/$BIN_NAME"

echo ""
echo "✅ openatrading 已安装到 $INSTALL_DIR/$BIN_NAME"

# ── Init config ────────────────────────────────────────

echo ""
echo "⚙  正在初始化配置..."
"$INSTALL_DIR/$BIN_NAME" init

# ── Ensure project dir exists ──────────────────────────

if [ ! -d "$PROJECT_DIR" ]; then
    echo "📁 创建项目目录: $PROJECT_DIR"
    if [ -w "$(dirname "$PROJECT_DIR")" ]; then
        mkdir -p "$PROJECT_DIR"
    else
        sudo mkdir -p "$PROJECT_DIR"
    fi
fi

# ── Install systemd service ────────────────────────────

if ! command -v systemctl &>/dev/null; then
    echo ""
    echo "⚠ systemctl 不可用，跳过 systemd 服务安装"
    echo "  手动运行: openatrading gateway run"
    exit 0
fi

echo ""
echo "⚙  正在安装 systemd 服务..."
cd "$PROJECT_DIR"
"$INSTALL_DIR/$BIN_NAME" gateway install

echo ""
echo "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━"
echo "✅ 安装完成"
echo ""
echo "   openatrading gateway status   # 查看服务状态"
echo "   openatrading gateway stop     # 停止服务"
echo "   journalctl --user -u openatrading -f  # 查看日志"
echo ""
echo "   卸载: ./install.sh --uninstall"
echo "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━"
