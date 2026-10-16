#!/bin/bash
set -euo pipefail

# 校验 macOS .app 是完整封装签名（ad-hoc 或 Developer ID 都算通过）。
#
# 背景：Tauri bundler 若跳过签名，产物只剩链接器自带的 ad-hoc 签名，表现为
# "Sealed Resources=none" / "Info.plist=not bound"，codesign --verify 直接失败，
# Gatekeeper 也拒绝打开。发版前校验，避免把无法运行的包推给用户。
#
# 用法：verify-macos-signature.sh <path/to/App.app>

if [ "$#" -ne 1 ]; then
  echo "❌ 用法：$(basename "$0") <path/to/App.app>"
  exit 1
fi

APP="$1"

if [ ! -d "$APP" ]; then
  echo "❌ 不是目录或不存在：$APP"
  exit 1
fi

case "$APP" in
  *.app) ;;
  *) echo "❌ 不是 .app bundle：$APP"; exit 1 ;;
esac

# 1. 签名有效性（含嵌套组件与资源封装）
if ! codesign --verify --deep --strict "$APP" 2>&1; then
  echo "❌ 签名校验失败：$APP"
  exit 1
fi

# 2. 必须是封装签名：code directory 需包含资源与 Info.plist 的哈希
SIG_INFO="$(codesign -dv "$APP" 2>&1 || true)"
for bad in "Sealed Resources=none" "Info.plist=not bound"; do
  if printf '%s' "$SIG_INFO" | grep -qF "$bad"; then
    echo "❌ 产物未封装签名（$bad）：$APP"
    echo "   请确认 src-tauri/tauri.conf.json 的 bundle.macOS.signingIdentity 已设置为 \"-\""
    exit 1
  fi
done

echo "✅ 签名校验通过：$APP"
# BSD sed 不支持 \| 交替，摘要过滤用 grep -E
printf '%s\n' "$SIG_INFO" | grep -E '^(Identifier|Signature|TeamIdentifier|Sealed Resources)' | sed 's/^/   /'
