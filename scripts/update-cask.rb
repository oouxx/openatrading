#!/usr/bin/env ruby
# frozen_string_literal: true

# 更新 Homebrew cask 的 version / 下载 URL / 双架构 sha256 / livecheck。
#
# 本地单架构发布（release-brew.sh）与 CI 双架构发布（release-brew.yml）共用本脚本，
# 保证两条路径改写结果一致。
#
# 环境变量：
#   BREW_CASK          cask 文件路径（必填）
#   BREW_VERSION       版本号，不含前导 v（必填）
#   BREW_SHA_X64       Intel DMG 的 sha256；留空写 :no_check
#   BREW_SHA_AARCH64   Apple Silicon DMG 的 sha256；留空写 :no_check
#   BREW_BASE_URL      Release 下载地址前缀，默认 GitHub Releases

cask    = ENV.fetch("BREW_CASK")
version = ENV.fetch("BREW_VERSION")
sha_x64 = ENV.fetch("BREW_SHA_X64", "")
sha_arm = ENV.fetch("BREW_SHA_AARCH64", "")
base    = ENV.fetch("BREW_BASE_URL", "https://github.com/oouxx/homebrew-openatrading/releases/download")

src = File.read(cask)

# version
src.sub!(/^  version "[^"]*"/, %(  version "#{version}"))

# 兼容旧格式：移除顶层 sha256 :no_check
src.sub!(/^  sha256 :no_check\n/, "")

# 兼容旧版：自建服务器 URL → GitHub Releases
src.gsub!(%r{https://trading\.96112901\.xyz/downloads}, base)

sha_value = ->(sha) { sha.empty? ? ":no_check" : %("#{sha}") }

# 双架构分支整体重写（幂等）；\#{version} 保持为 cask 内的变量插值
block = "  if Hardware::CPU.intel?\n" \
        "    url \"#{base}/v\#{version}/openatrading_\#{version}_x64.dmg\"\n" \
        "    sha256 #{sha_value.call(sha_x64)}\n" \
        "  else\n" \
        "    url \"#{base}/v\#{version}/openatrading_\#{version}_aarch64.dmg\"\n" \
        "    sha256 #{sha_value.call(sha_arm)}\n" \
        "  end\n"
unless src.sub!(/  if Hardware::CPU\.intel\?\n.*?  end\n/m, block)
  # 找不到标准架构分支时至少兜底替换 URL 基底
  src.gsub!(%r{https://trading\.96112901\.xyz/downloads}, base)
end

# livecheck：GitHub Releases 策略
src.sub!(/  livecheck do\n.*?  end\n/m,
         "  livecheck do\n    url :url\n    strategy :github_latest\n  end\n")

File.write(cask, src)
puts "✅ 已更新 #{cask} → #{version}"
