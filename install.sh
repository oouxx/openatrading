#!/bin/bash
set -e

REPO="oouxx/openatrading"
INSTALL_DIR="${INSTALL_DIR:-/usr/local/bin}"

# ── Usage ─────────────────────────────────────────────────

usage() {
  echo "Usage:"
  echo "  ./install.sh              安装 openatrading CLI"
  echo "  ./install.sh --desktop    下载桌面客户端 (Tauri)"
  echo "  ./install.sh --uninstall  卸载 openatrading"
  exit 0
}

MODE="cli"
case "${1:-}" in
  --desktop|-d)   MODE="desktop" ;;
  --uninstall|-u) MODE="uninstall" ;;
  --help|-h)      usage ;;
esac

# ── Detect platform ────────────────────────────────────

UNAME_S=$(uname -s)
UNAME_M=$(uname -m)

detect_arch() {
  case "$UNAME_M" in
    x86_64|amd64)  echo "x86_64" ;;
    aarch64|arm64) echo "aarch64" ;;
    *)             echo "unsupported" ;;
  esac
}

ARCH=$(detect_arch)
if [ "$ARCH" = "unsupported" ]; then
  echo "❌ 不支持的架构: $UNAME_M"
  exit 1
fi

# ── Auth header ──────────────────────────────────────────

AUTH=()
if [ -n "${GITHUB_TOKEN:-}" ]; then
  AUTH=(-H "Authorization: Bearer ${GITHUB_TOKEN}")
fi

# ── Fetch release info ───────────────────────────────────

fetch_release() {
  echo "⬇ 正在获取最新版本..."
  RELEASE_URL="https://api.github.com/repos/${REPO}/releases/latest"
  RELEASE_JSON=$(curl -fsSL "${AUTH[@]}" "$RELEASE_URL")
  TAG=$(echo "$RELEASE_JSON" | grep '"tag_name"' | head -1 | cut -d '"' -f 4)
  VERSION="${TAG#v}"
  echo "📦 版本: $TAG"
}

find_asset_url() {
  local name="$1"
  ASSET_URL=$(echo "$RELEASE_JSON" | python3 -c "
import sys, json
data = json.load(sys.stdin)
for a in data.get('assets', []):
    if a['name'] == '$name':
        print(a['url'])
        break
" 2>/dev/null || true)

  if [ -z "$ASSET_URL" ]; then
    ASSET_URL=$(echo "$RELEASE_JSON" | python3 -c "
import sys, json
data = json.load(sys.stdin)
for a in data.get('assets', []):
    if '$name' in a['name']:
        print(a['url'])
        break
" 2>/dev/null || true)
  fi
}

download_asset() {
  local url="$1"
  local out="$2"
  echo "⬇ 正在下载 $out..."
  curl -fsSL "${AUTH[@]}" -H "Accept: application/octet-stream" "$url" -o "$out"
}

# ── Uninstall ─────────────────────────────────────────────

if [ "$MODE" = "uninstall" ]; then
  echo "🗑  正在卸载 openatrading..."

  if command -v systemctl &>/dev/null; then
    echo "  停止 systemd 服务..."
    systemctl --user stop openatrading 2>/dev/null || true
    systemctl --user disable openatrading 2>/dev/null || true
    rm -f "$HOME/.config/systemd/user/openatrading.service"
    systemctl --user daemon-reload 2>/dev/null || true
  fi

  if [ -f "$INSTALL_DIR/openatrading" ]; then
    if [ -w "$INSTALL_DIR" ]; then
      rm -f "$INSTALL_DIR/openatrading"
    else
      sudo rm -f "$INSTALL_DIR/openatrading"
    fi
    echo "  ✓ 已移除 $INSTALL_DIR/openatrading"
  fi

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

# ── Desktop install ───────────────────────────────────────

if [ "$MODE" = "desktop" ]; then
  fetch_release

  TMP_DIR=$(mktemp -d)
  trap 'rm -rf "$TMP_DIR"' EXIT

  if [ "$UNAME_S" = "Darwin" ]; then
    ASSET_NAME="openatrading_${VERSION}_${ARCH}.dmg"
    find_asset_url "$ASSET_NAME"

    if [ -z "$ASSET_URL" ]; then
      echo "❌ 未找到 macOS 桌面客户端: $ASSET_NAME"
      echo "   可用 assets:"
      echo "$RELEASE_JSON" | grep '"name"' | head -20
      exit 1
    fi

    download_asset "$ASSET_URL" "$TMP_DIR/$ASSET_NAME"

    echo "📦 挂载 DMG..."
    hdiutil attach "$TMP_DIR/$ASSET_NAME" -quiet

    echo "📦 安装到 /Applications..."
    cp -R "/Volumes/openatrading/openatrading.app" /Applications/ 2>/dev/null || \
      cp -R /Volumes/openatrading/*.app /Applications/ 2>/dev/null || true

    hdiutil detach "/Volumes/openatrading" -quiet 2>/dev/null || true

    echo ""
    echo "✅ 桌面客户端已安装到 /Applications"
    echo "   首次打开可能需要在「系统设置 → 隐私与安全性」中允许"

  elif [ "$UNAME_S" = "Linux" ]; then
    if [ "$ARCH" = "x86_64" ]; then
      ASSET_NAME="openatrading_${VERSION}_amd64.deb"
      find_asset_url "$ASSET_NAME"

      if [ -z "$ASSET_URL" ]; then
        ASSET_NAME="openatrading_${VERSION}_amd64.AppImage"
        find_asset_url "$ASSET_NAME"
      fi
    else
      ASSET_NAME="openatrading_${VERSION}_${ARCH}.deb"
      find_asset_url "$ASSET_NAME"

      if [ -z "$ASSET_URL" ]; then
        ASSET_NAME="openatrading_${VERSION}_${ARCH}.AppImage"
        find_asset_url "$ASSET_NAME"
      fi
    fi

    if [ -z "$ASSET_URL" ]; then
      echo "❌ 未找到 Linux 桌面客户端"
      echo "   可用 assets:"
      echo "$RELEASE_JSON" | grep '"name"' | head -20
      exit 1
    fi

    download_asset "$ASSET_URL" "$TMP_DIR/$ASSET_NAME"

    if [[ "$ASSET_NAME" == *.deb ]]; then
      echo "📦 安装 .deb..."
      sudo dpkg -i "$TMP_DIR/$ASSET_NAME" || sudo apt-get install -f -y
      echo ""
      echo "✅ 桌面客户端已安装"
    elif [[ "$ASSET_NAME" == *.AppImage ]]; then
      chmod +x "$TMP_DIR/$ASSET_NAME"
      mkdir -p "$HOME/Applications"
      mv "$TMP_DIR/$ASSET_NAME" "$HOME/Applications/"
      echo ""
      echo "✅ 桌面客户端已安装到 ~/Applications/$ASSET_NAME"
    fi

  elif [ "$UNAME_S" = "MINGW"* ] || [ "$UNAME_S" = "MSYS"* ] || [ "$UNAME_S" = "CYGWIN"* ]; then
    ASSET_NAME="openatrading_${VERSION}_x64-setup.nsis.zip"
    find_asset_url "$ASSET_NAME"

    if [ -z "$ASSET_URL" ]; then
      ASSET_NAME="openatrading_${VERSION}_x64_en-US.msi"
      find_asset_url "$ASSET_NAME"
    fi

    if [ -z "$ASSET_URL" ]; then
      echo "❌ 未找到 Windows 桌面客户端"
      exit 1
    fi

    download_asset "$ASSET_URL" "$TMP_DIR/$ASSET_NAME"
    echo ""
    echo "✅ 已下载到 $TMP_DIR/$ASSET_NAME"
    echo "   请手动运行安装程序"
    # Keep tmp dir for Windows
    trap - EXIT
  else
    echo "❌ 不支持的操作系统: $UNAME_S"
    exit 1
  fi

  exit 0
fi

# ── CLI install ───────────────────────────────────────────

fetch_release

if [ "$UNAME_S" = "Linux" ]; then
  CLI_ASSET="openatrading-${ARCH}-unknown-linux-gnu"
elif [ "$UNAME_S" = "Darwin" ]; then
  CLI_ASSET="openatrading-${ARCH}-apple-darwin"
else
  echo "❌ CLI 仅支持 Linux 和 macOS"
  exit 1
fi

find_asset_url "$CLI_ASSET"

if [ -z "$ASSET_URL" ]; then
  echo "❌ 未找到 CLI 二进制: $CLI_ASSET"
  echo "   可用 assets:"
  echo "$RELEASE_JSON" | grep '"name"' | head -20
  exit 1
fi

echo "📦 平台: $CLI_ASSET"

TMP_DIR=$(mktemp -d)
trap 'rm -rf "$TMP_DIR"' EXIT

download_asset "$ASSET_URL" "$TMP_DIR/openatrading"

if [ -w "$INSTALL_DIR" ]; then
  mv "$TMP_DIR/openatrading" "$INSTALL_DIR/openatrading"
else
  sudo mv "$TMP_DIR/openatrading" "$INSTALL_DIR/openatrading"
fi
chmod +x "$INSTALL_DIR/openatrading"

echo ""
echo "✅ openatrading CLI 已安装到 $INSTALL_DIR/openatrading"

# ── Init config ────────────────────────────────────────

echo ""
echo "⚙  正在初始化配置..."
"$INSTALL_DIR/openatrading" init

# ── Install systemd service (Linux only) ──────────────

if [ "$UNAME_S" = "Linux" ] && command -v systemctl &>/dev/null; then
  echo ""
  echo "⚙  正在安装 systemd 服务..."
  "$INSTALL_DIR/openatrading" gateway install

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
else
  echo ""
  echo "✅ 安装完成"
  echo ""
  echo "   openatrading gateway run      # 启动网关服务"
  echo "   卸载: ./install.sh --uninstall"
fi