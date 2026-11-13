#!/usr/bin/env bash
# 汇总各平台的 updater 更新包，生成 Tauri 静态更新清单 latest.json。
#
# 用法：generate-latest-json.sh <dist 目录> <版本号> <owner/repo>
#
# dist 目录里应包含各平台的更新包及其 `.sig`：
#   openatrading-darwin-aarch64.app.tar.gz(.sig)
#   openatrading-darwin-x86_64.app.tar.gz(.sig)
#   openatrading_<version>_aarch64.AppImage(.sig)   # Linux 直接使用 AppImage
#   openatrading_<version>_amd64.AppImage(.sig)
#   openatrading-windows-x86_64.nsis.zip(.sig)
#   openatrading-windows-x86_64.msi.zip(.sig)
#
# 只列出实际存在的平台：Tauri 会先校验整份清单，缺失的条目不能留空。
set -euo pipefail

DIST_DIR="${1:?用法: generate-latest-json.sh <dist> <version> <owner/repo>}"
VERSION="${2:?缺少版本号}"
REPO="${3:?缺少仓库（owner/repo）}"

TAG="v${VERSION}"
BASE_URL="https://github.com/${REPO}/releases/download/${TAG}"
platforms='{}'

add_platform() {
    local key="$1"
    local file="$2"
    local path="${DIST_DIR}/${file}"
    local sig_path="${path}.sig"

    if [[ ! -f "$path" || ! -f "$sig_path" ]]; then
        return 0
    fi

    local signature
    signature="$(tr -d '\n' < "$sig_path")"
    if [[ -z "$signature" ]]; then
        echo "::error::${sig_path} 为空，更新包未签名"
        exit 1
    fi

    platforms="$(jq \
        --arg key "$key" \
        --arg url "${BASE_URL}/${file}" \
        --arg signature "$signature" \
        '.[$key] = {url: $url, signature: $signature}' <<<"$platforms")"
    echo "  + ${key}"
}

add_platform darwin-aarch64 openatrading-darwin-aarch64.app.tar.gz
add_platform darwin-x86_64 openatrading-darwin-x86_64.app.tar.gz
# Tauri 的 Linux updater 直接下载/替换 AppImage，没有额外的 tar.gz。
add_platform linux-aarch64 "openatrading_${VERSION}_aarch64.AppImage"
add_platform linux-x86_64 "openatrading_${VERSION}_amd64.AppImage"
# Windows 更新器先找 `{os}-{arch}-{installer}`，找不到才退回 `{os}-{arch}`。
add_platform windows-x86_64-nsis openatrading-windows-x86_64.nsis.zip
add_platform windows-x86_64-msi openatrading-windows-x86_64.msi.zip

# 通用键：无法识别安装器类型时按 NSIS 处理。
if jq -e 'has("windows-x86_64-nsis")' <<<"$platforms" >/dev/null; then
    platforms="$(jq '. + {"windows-x86_64": .["windows-x86_64-nsis"]}' <<<"$platforms")"
    echo "  + windows-x86_64 (fallback → nsis)"
fi

if [[ "$(jq 'length' <<<"$platforms")" -eq 0 ]]; then
    echo "::error::dist 中没有找到任何可发布的更新包"
    exit 1
fi

jq -n \
    --arg version "$VERSION" \
    --arg notes "" \
    --arg pub_date "$(date -u +%Y-%m-%dT%H:%M:%SZ)" \
    --argjson platforms "$platforms" \
    '{version: $version, notes: $notes, pub_date: $pub_date, platforms: $platforms}' \
    > "${DIST_DIR}/latest.json"

echo "已生成 ${DIST_DIR}/latest.json（${VERSION}）"
