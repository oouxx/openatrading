#!/bin/bash
set -e

DL_SERVER="${DL_SERVER:-https://trading.96112901.xyz}"
INSTALL_DIR="${INSTALL_DIR:-/usr/local/bin}"

# ── Usage ─────────────────────────────────────────────────

usage() {
  echo "Usage:"
  echo "  ./install.sh              安装 openatrading CLI"
  echo "  ./install.sh --desktop    下载桌面客户端 (Tauri)"
  echo "  ./install.sh --uninstall  卸载 openatrading"
  echo ""
  echo "环境变量:"
  echo "  DL_SERVER   下载服务器地址 (默认: https://trading.96112901.xyz)"
  echo "  INSTALL_DIR CLI 安装目录 (默认: /usr/local/bin)"
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

# ── Helpers ───────────────────────────────────────────────

download_file() {
  local filename="$1"
  local out="$2"
  local url="${DL_SERVER}/downloads/${filename}"
  echo "⬇ 正在下载 ${filename}..."
  local http_code
  http_code=$(curl -fsSL -w "%{http_code}" -o "$out" "$url")
  if [ "$http_code" != "200" ]; then
    echo "❌ 下载失败 (HTTP $http_code): $filename"
    echo "   URL: $url"
    rm -f "$out"
    return 1
  fi
}

try_download() {
  local filename="$1"
  local out="$2"
  if ! download_file "$filename" "$out"; then
    return 1
  fi
  if [ ! -s "$out" ]; then
    echo "❌ 下载文件为空: $filename"
    rm -f "$out"
    return 1
  fi
  return 0
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
  TMP_DIR=$(mktemp -d)
  trap 'rm -rf "$TMP_DIR"' EXIT

  if [ "$UNAME_S" = "Darwin" ]; then
    ASSET_NAME="openatrading_${ARCH}.dmg"

    if ! try_download "$ASSET_NAME" "$TMP_DIR/$ASSET_NAME"; then
      echo ""
      echo "❌ 未找到 macOS 桌面客户端 (${ARCH})"
      echo "   请访问 ${DL_SERVER} 查看可用版本"
      exit 1
    fi

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
      DEB_NAME="openatrading_amd64.deb"
      APPIMG_NAME="openatrading_amd64.AppImage"
    else
      DEB_NAME="openatrading_${ARCH}.deb"
      APPIMG_NAME="openatrading_${ARCH}.AppImage"
    fi

    if try_download "$DEB_NAME" "$TMP_DIR/$DEB_NAME"; then
      echo "📦 安装 .deb..."
      sudo dpkg -i "$TMP_DIR/$DEB_NAME" || sudo apt-get install -f -y
      echo ""
      echo "✅ 桌面客户端已安装"
    elif try_download "$APPIMG_NAME" "$TMP_DIR/$APPIMG_NAME"; then
      chmod +x "$TMP_DIR/$APPIMG_NAME"
      mkdir -p "$HOME/Applications"
      mv "$TMP_DIR/$APPIMG_NAME" "$HOME/Applications/"
      echo ""
      echo "✅ 桌面客户端已安装到 ~/Applications/$APPIMG_NAME"
    else
      echo ""
      echo "❌ 未找到 Linux 桌面客户端 (${ARCH})"
      echo "   请访问 ${DL_SERVER} 查看可用版本"
      exit 1
    fi

  elif [[ "$UNAME_S" == MINGW* ]] || [[ "$UNAME_S" == MSYS* ]] || [[ "$UNAME_S" == CYGWIN* ]]; then
    EXE_NAME="openatrading_x64-setup.exe"
    MSI_NAME="openatrading_x64_en-US.msi"

    if try_download "$EXE_NAME" "$TMP_DIR/$EXE_NAME"; then
      echo ""
      echo "✅ 已下载到 $TMP_DIR/$EXE_NAME"
      echo "   请手动运行安装程序"
      trap - EXIT
    elif try_download "$MSI_NAME" "$TMP_DIR/$MSI_NAME"; then
      echo ""
      echo "✅ 已下载到 $TMP_DIR/$MSI_NAME"
      echo "   请手动运行安装程序"
      trap - EXIT
    else
      echo ""
      echo "❌ 未找到 Windows 桌面客户端"
      echo "   请访问 ${DL_SERVER} 查看可用版本"
      exit 1
    fi
  else
    echo "❌ 不支持的操作系统: $UNAME_S"
    exit 1
  fi

  exit 0
fi

# ── CLI install ───────────────────────────────────────────

if [ "$UNAME_S" = "Linux" ]; then
  CLI_ASSET="openatrading-${ARCH}-unknown-linux-gnu"
elif [ "$UNAME_S" = "Darwin" ]; then
  CLI_ASSET="openatrading-${ARCH}-apple-darwin"
else
  echo "❌ CLI 仅支持 Linux 和 macOS"
  exit 1
fi

TMP_DIR=$(mktemp -d)
trap 'rm -rf "$TMP_DIR"' EXIT

if ! try_download "$CLI_ASSET" "$TMP_DIR/openatrading"; then
  echo ""
  echo "❌ 未找到 CLI 二进制: $CLI_ASSET"
  echo "   请访问 ${DL_SERVER} 查看可用版本"
  exit 1
fi

echo "📦 平台: $CLI_ASSET"

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