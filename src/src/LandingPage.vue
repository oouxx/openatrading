<template>
  <div class="min-h-screen bg-bg">
    <!-- Hero -->
    <section class="pt-24 pb-16 px-6 max-w-4xl mx-auto text-center">
      <h1 class="text-5xl font-bold text-white mb-4 tracking-tight">
        Open<span class="text-ai">ATrading</span>
      </h1>
      <p class="text-xl text-ink-muted mb-8 max-w-2xl mx-auto leading-relaxed">
        AI-native A 股短线交易决策工作台。多角色 AI 投资策略、实时市场采集、Telegram 交互、持仓管理。
      </p>
      <div class="flex gap-4 justify-center">
        <router-link
          to="/trading"
          class="px-6 py-2.5 bg-ai/20 text-ai rounded-lg font-mono font-medium hover:bg-ai/30 transition-colors"
        >
          打开交易终端 →
        </router-link>
        <a
          href="#download"
          class="px-6 py-2.5 bg-white/5 text-white/80 rounded-lg font-mono hover:bg-white/10 transition-colors"
        >
          下载桌面客户端
        </a>
      </div>
    </section>

    <!-- Features -->
    <section class="py-16 px-6 max-w-5xl mx-auto">
      <h2 class="text-2xl font-bold text-white mb-10 text-center">核心能力</h2>
      <div class="grid grid-cols-1 md:grid-cols-3 gap-6">
        <div class="bg-white/5 border border-hairline rounded-card p-6">
          <div class="text-ai text-2xl mb-3">🤖</div>
          <h3 class="text-white font-semibold mb-2">多角色 AI 策略</h3>
          <p class="text-ink-muted text-sm leading-relaxed">
            游资操盘手、量化分析师、风控官、Zarili 量价派 — 每个角色拥有独立的投资策略体系，AI 严格遵守策略进行交易决策分析。
          </p>
        </div>
        <div class="bg-white/5 border border-hairline rounded-card p-6">
          <div class="text-ai text-2xl mb-3">⏱</div>
          <h3 class="text-white font-semibold mb-2">自动采集 & 决策</h3>
          <p class="text-ink-muted text-sm leading-relaxed">
            交易时间每 30 分钟采集市场数据，收盘后自动 AI 决策分析。Cron 调度 + SQLite 历史存储，10 日数据回溯对比。
          </p>
        </div>
        <div class="bg-white/5 border border-hairline rounded-card p-6">
          <div class="text-ai text-2xl mb-3">📡</div>
          <h3 class="text-white font-semibold mb-2">Telegram 交互</h3>
          <p class="text-ink-muted text-sm leading-relaxed">
            采集通知、决策推送、持仓变动实时推送。支持双向对话 — 随时查询持仓、手动触发分析、多轮交易讨论。
          </p>
        </div>
      </div>
    </section>

    <!-- Download -->
    <section id="download" class="py-16 px-6 max-w-5xl mx-auto">
      <h2 class="text-2xl font-bold text-white mb-4 text-center">下载桌面客户端</h2>
      <p class="text-ink-muted text-center mb-10 text-sm">
        由 Tauri 构建的原生桌面应用。下载对应平台版本。
      </p>
      <div class="grid grid-cols-1 md:grid-cols-3 gap-6">
        <!-- macOS Intel -->
        <a
          :href="dl('x86_64-apple-darwin')"
          class="bg-white/5 border border-hairline rounded-card p-6 text-center hover:border-ai/30 hover:bg-white/[0.07] transition-colors group"
        >
          <div class="text-3xl mb-3">🍎</div>
          <h3 class="text-white font-semibold mb-1">macOS Intel</h3>
          <p class="text-ink-muted text-xs font-mono">x86_64-apple-darwin</p>
          <span class="inline-block mt-3 px-4 py-1.5 bg-ai/10 text-ai text-sm rounded-lg group-hover:bg-ai/20 transition-colors">
            下载 .dmg
          </span>
        </a>
        <!-- macOS Apple Silicon -->
        <a
          :href="dl('aarch64-apple-darwin')"
          class="bg-white/5 border border-hairline rounded-card p-6 text-center hover:border-ai/30 hover:bg-white/[0.07] transition-colors group"
        >
          <div class="text-3xl mb-3">💻</div>
          <h3 class="text-white font-semibold mb-1">macOS Apple Silicon</h3>
          <p class="text-ink-muted text-xs font-mono">aarch64-apple-darwin</p>
          <span class="inline-block mt-3 px-4 py-1.5 bg-ai/10 text-ai text-sm rounded-lg group-hover:bg-ai/20 transition-colors">
            下载 .dmg
          </span>
        </a>
        <!-- Linux -->
        <a
          :href="dl('x86_64-unknown-linux-gnu')"
          class="bg-white/5 border border-hairline rounded-card p-6 text-center hover:border-ai/30 hover:bg-white/[0.07] transition-colors group"
        >
          <div class="text-3xl mb-3">🐧</div>
          <h3 class="text-white font-semibold mb-1">Linux</h3>
          <p class="text-ink-muted text-xs font-mono">x86_64 .deb / .AppImage</p>
          <span class="inline-block mt-3 px-4 py-1.5 bg-ai/10 text-ai text-sm rounded-lg group-hover:bg-ai/20 transition-colors">
            下载
          </span>
        </a>
      </div>
    </section>

    <!-- CLI Install -->
    <section class="py-16 px-6 max-w-3xl mx-auto">
      <h2 class="text-2xl font-bold text-white mb-6 text-center">命令行安装</h2>
      <div class="bg-black/50 border border-hairline rounded-card p-6">
        <p class="text-ink-muted text-sm mb-3">一键安装 CLI（仅 Linux x86_64 / aarch64）：</p>
        <pre class="bg-black border border-hairline rounded-lg p-4 text-green/80 font-mono text-sm overflow-x-auto"><code>curl -fsSL {{ getApiBaseUrl() }}/install.sh | bash</code></pre>
      </div>
    </section>

    <!-- Footer -->
    <footer class="py-8 px-6 text-center border-t border-hairline">
      <p class="text-ink-muted text-xs">
        <a href="https://github.com/oouxx/openatrading" class="hover:text-white transition-colors">GitHub</a>
        <span class="mx-3">·</span>
        <span>红涨绿跌 · AI-first · 金融终端风格</span>
      </p>
    </footer>
  </div>
</template>

<script setup lang="ts">
import { getApiBaseUrl } from '@/stores/data'

const DL_BASE = `${getApiBaseUrl()}/downloads`

function dl(target: string): string {
  const ext = target.includes('apple') ? '.dmg' : target.includes('linux') ? '.deb' : '.msi'
  return `${DL_BASE}/OpenATrading_${target}${ext}`
}
</script>
