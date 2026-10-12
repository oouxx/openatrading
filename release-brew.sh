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

# ── gh 认证：优先 keyring，避免 .env 泄漏的 GITHUB_TOKEN 覆盖 ──
# Makefile 顶部 `-include .env` + `export` 会把 .env 里的 GITHUB_TOKEN
# （fine-grained PAT，通常无 release 权限）注入环境；gh 优先读环境变量，
# 导致 "Resource not accessible by personal access token" (HTTP 403)。
# 本地发布应使用 `gh auth login` 的 keyring 凭据；CI 等无 keyring 场景
# 用 RELEASE_GH_TOKEN 显式指定。
if [ -n "${RELEASE_GH_TOKEN:-}" ]; then
  export GH_TOKEN="$RELEASE_GH_TOKEN"
  unset GITHUB_TOKEN
else
  unset GH_TOKEN GITHUB_TOKEN
fi

# 预检：gh 已登录（keyring 或 RELEASE_GH_TOKEN）
if ! gh auth status >/dev/null 2>&1; then
  echo "❌ gh 未登录：请先运行 gh auth login（GitHub.com + HTTPS）"
  echo "   或指定 RELEASE_GH_TOKEN=xxx make release-brew"
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
    if echo "$CREATE_OUT" | rg -q "403|not accessible"; then
      echo ""
      echo "💡 原因：gh 使用了无 release 权限的 token（通常是 .env 的 GITHUB_TOKEN 泄漏覆盖了 keyring）。"
      echo "   本脚本已自动忽略 .env 的 GITHUB_TOKEN，改用 gh auth login 的 keyring 凭据。"
      echo "   若仍失败，请检查："
      echo "     1. gh auth status 确认登录账号为 oouxx 且 scope 含 repo"
      echo "     2. 或显式指定有权限的 token：RELEASE_GH_TOKEN=xxx make release-brew"
    fi
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

# 单架构发布：只填本机架构的 sha，另一架构写 :no_check（多架构请用 CI/release-brew.yml）
if [ "$ARCH" = "x64" ]; then
  export BREW_SHA_X64="$SHA" BREW_SHA_AARCH64=""
else
  export BREW_SHA_X64="" BREW_SHA_AARCH64="$SHA"
fi
export BREW_CASK="$CASK" BREW_VERSION="$V"
ruby scripts/update-cask.rb

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
