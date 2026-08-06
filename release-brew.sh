#!/bin/bash
set -euo pipefail

# ── 配置（可用环境变量覆盖） ──
TAP_REPO="${BREW_TAP_REPO:-oouxx/homebrew-openatrading}"
TAP_DIR="${BREW_TAP_DIR:-${HOME}/Desktop/github/homebrew-openatrading}"
PUSH="${RELEASE_PUSH:-1}"

cd "$(dirname "$0")"

if ! which gh >/dev/null 2>&1; then
  echo "❌ 需要 gh CLI（brew install gh）"
  exit 1
fi

case "$(uname -m)" in
  arm64)  ARCH="aarch64" ;;
  x86_64) ARCH="x64" ;;
  *) echo "❌ release-brew 仅支持 macOS（当前: $(uname -m)）"; exit 1 ;;
esac

# ── 版本：以 workspace Cargo.toml 为准（tauri.conf.json 已省略 version，自动继承） ──
V="$(grep '^version' Cargo.toml | head -1 | sed -E 's/.*"([0-9]+\.[0-9]+\.[0-9]+)".*/\1/')"
TAG="v$V"

DMG="target/release/bundle/dmg/openatrading_${V}_${ARCH}.dmg"
[ -f "$DMG" ] || { echo "❌ 未找到 DMG：$DMG（请先运行 make build-tauri）"; exit 1; }
SHA="$(shasum -a 256 "$DMG" | awk '{print $1}')"

echo "🚀 发布 OpenATrading $V ($ARCH) → $TAP_REPO"
echo "   DMG:   $DMG"
echo "   SHA256: $SHA"

# ── 1. gh 上传 DMG 到 tap 仓库的 GitHub Release ──
echo "📦 创建/复用 Release ${TAG}（${TAP_REPO}）"
CREATE_EXIT=0
CREATE_OUT="$(gh release create "$TAG" --repo "$TAP_REPO" --title "$TAG" --notes "OpenATrading $V" 2>&1)" || CREATE_EXIT=$?
if [ "$CREATE_EXIT" -ne 0 ]; then
  if echo "$CREATE_OUT" | rg -q "already exists"; then
    echo "⚠ Release $TAG 已存在，复用"
  else
    echo "❌ 创建 Release 失败："
    echo "$CREATE_OUT"
    exit 1
  fi
fi
echo "📤 上传 $(basename "$DMG")"
gh release upload "$TAG" "$DMG" --repo "$TAP_REPO" --clobber
URL="https://github.com/$TAP_REPO/releases/download/$TAG/$(basename "$DMG")"
echo "✅ 已上传：${URL}"

# ── 2. 更新本地 brew cask ──
[ -d "$TAP_DIR/.git" ] || { echo "❌ tap 仓库不存在：$TAP_DIR"; exit 1; }
CASK="$TAP_DIR/Casks/openatrading.rb"
[ -f "$CASK" ] || { echo "❌ cask 文件不存在：$CASK"; exit 1; }

export BREW_CASK="$CASK" BREW_VERSION="$V" BREW_SHA="$SHA" BREW_ARCH="$ARCH"
ruby <<'RUBY'
cask = ENV.fetch("BREW_CASK")
v    = ENV.fetch("BREW_VERSION")
sha  = ENV.fetch("BREW_SHA")
arch = ENV.fetch("BREW_ARCH")
base = "https://github.com/oouxx/homebrew-openatrading/releases/download"
src  = File.read(cask)

# version
src.sub!(/^  version "[^"]*"/, %Q{  version "#{v}"})
# 顶层 sha256（旧格式）移除
src.sub!(/^  sha256 :no_check\n/, "")
# 兼容旧版（自建服务器 URL）→ GitHub Releases
src.gsub!(%r{https://trading\.example\.com/downloads}, base)
# 架构分支整体重写（幂等）
sha_x64 = arch == "x64" ? %Q{"#{sha}"} : ":no_check"
sha_arm = arch == "aarch64" ? %Q{"#{sha}"} : ":no_check"
block = "  if Hardware::CPU.intel?\n" \
        "    url \"#{base}/v\#{version}/openatrading_\#{version}_x64.dmg\"\n" \
        "    sha256 #{sha_x64}\n" \
        "  else\n" \
        "    url \"#{base}/v\#{version}/openatrading_\#{version}_aarch64.dmg\"\n" \
        "    sha256 #{sha_arm}\n" \
        "  end\n"
unless src.sub!(/  if Hardware::CPU\.intel\?\n.*?  end\n/m, block)
  # 兼容：找不到标准分支时至少替换 URL 基底
  src.gsub!(%r{https://trading\.example\.com/downloads}, base)
end
# livecheck：GitHub Releases 策略
src.sub!(/  livecheck do\n.*?  end\n/m,
  "  livecheck do\n    url :url\n    strategy :github_latest\n  end\n")

File.write(cask, src)
puts "✅ 已更新 #{cask}"
RUBY

# ── 3. 提交 tap 并推送（RELEASE_PUSH=0 可跳过推送） ──
cd "$TAP_DIR"
if ! git diff --quiet -- Casks/openatrading.rb; then
  git add Casks/openatrading.rb
  git commit -m "openatrading $V"
  echo "✅ 已提交 tap：$(git rev-parse --short HEAD)"
  if [ "$PUSH" = "1" ]; then
    git push origin HEAD
    echo "✅ 已推送到 $(git remote get-url origin)"
  else
    echo "ℹ 未推送（RELEASE_PUSH=0 跳过推送）；本地 tap 可直接使用"
  fi
else
  echo "ℹ Cask 无变化（可能已是 ${V}）"
fi

echo ""
echo "🎉 发布完成 openatrading $V"
echo "  下载地址：$URL"
echo "  本地安装："
echo "    brew tap oouxx/openatrading $TAP_DIR"
echo "    brew install --cask openatrading"
echo "  提示：未签名 DMG 首次打开若被 Gatekeeper 拦截，执行："
echo "    xattr -dr com.apple.quarantine /Applications/OpenATrading.app"
