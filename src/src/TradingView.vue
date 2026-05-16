<template>
  <div class="pt-11 min-h-screen bg-bg">
    <!-- Error -->
      <div v-if="overview.error" class="py-4">
        <div class="max-w-[1600px] mx-auto px-6">
          <div class="bg-up/5 border border-up/20 rounded-card px-5 py-3 flex items-center justify-between">
            <div class="flex items-center gap-3">
              <span class="text-up text-sm">&#9888;</span>
              <span class="text-caption text-up">{{ overview.error }}</span>
            </div>
            <button @click="refreshData" class="text-caption text-up underline cursor-pointer">重试</button>
          </div>
        </div>
      </div>

      <!-- Loading -->
      <div v-if="overview.loading && !overview.data" class="flex flex-col items-center justify-center py-32 gap-4">
        <div class="w-3 h-3 rounded-full bg-ai animate-pulse"></div>
        <span class="text-caption font-mono text-ink-muted">加载市场数据...</span>
      </div>

      <!-- Content -->
      <template v-if="overview.data">
        <!-- ① Market Status + ② KPI -->
        <section class="max-w-[1600px] mx-auto px-6 py-section-y">
          <KpiCards
            :overview="overview.data"
            :risk-level="overview.data.risk_level"
            :decision-summary="overview.data.decision_summary"
          />
        </section>

        <!-- ② Sentiment History Chart -->
        <section class="max-w-[1600px] mx-auto px-6 pb-8">
          <SentimentHistory :data="sentimentChartData" />
        </section>

        <!-- ③ AI Market Summary -->
        <section class="max-w-[1600px] mx-auto px-6 pb-8">
          <div class="bg-bg-surface rounded-card border border-hairline p-card-inner">
            <div class="flex items-center gap-2 mb-2">
              <span class="w-1.5 h-1.5 rounded-full bg-ai"></span>
              <span class="text-caption-strong text-ink-muted font-mono uppercase tracking-wider">AI 市场摘要</span>
            </div>
            <p class="text-body text-white/80 leading-relaxed">{{ overview.data.decision_summary || '等待数据...' }}</p>
          </div>
        </section>

        <!-- ④ Tab Bar + Content -->
        <div class="sticky z-40 top-11 bg-bg border-b border-hairline">
          <div class="max-w-[1600px] mx-auto px-6">
            <TabBar v-model="activeTab" />
          </div>
        </div>
        <section class="max-w-[1600px] mx-auto min-h-[60vh] px-6 py-6">
          <component :is="currentTabComponent" />
        </section>
      </template>
    

    <!-- Floating AI Robot Button -->
    <motion.button
      v-if="overview.data"
      @click="showAiChat = true"
      class="fixed right-5 bottom-8 z-40 w-14 h-14 rounded-full
             bg-ai shadow-terminal-lg border-2 border-ai/30
             flex items-center justify-center text-2xl cursor-pointer"
      :initial="{ opacity: 0, scale: 0 }"
      :animate="{ opacity: 1, scale: 1 }"
      :whileHover="{ scale: 1.15 }"
      :whilePress="{ scale: 0.9 }"
      :transition="{ type: 'spring', stiffness: 400, damping: 17 }"
      title="AI 交易副驾驶"
    >
      🤖
    </motion.button>

    <!-- AI Chat Dialog -->
    <Teleport to="body">
      <AnimatePresence>
        <div
          v-if="showAiChat"
          class="fixed inset-0 z-[80] flex items-center justify-center"
          @click.self="showAiChat = false"
        >
          <motion.div
            class="bg-bg-surface rounded-card border border-hairline shadow-terminal-lg flex flex-col"
            style="width: 480px; max-width: 90vw; height: 600px; max-height: 80vh"
            :initial="{ opacity: 0, scale: 0.92, y: 30 }"
            :animate="{ opacity: 1, scale: 1, y: 0 }"
            :exit="{ opacity: 0, scale: 0.92, y: 30 }"
            :transition="{ duration: 0.25, ease: 'easeOut' }"
          >
            <!-- Dialog Header -->
            <div class="flex items-center justify-between px-4 py-3 border-b border-hairline flex-shrink-0">
              <div class="flex items-center gap-2">
                <span class="text-xl">🤖</span>
                <span class="text-caption-strong text-white font-mono">AI 交易副驾驶</span>
              </div>
              <button
                @click="showAiChat = false"
                class="text-ink-muted hover:text-white text-sm cursor-pointer transition-colors"
              >
                ✕
              </button>
            </div>

            <!-- Chat Body -->
            <div class="flex-1 min-h-0">
              <AiChatPanel :overview="overview.data" />
            </div>
          </motion.div>
        </div>
      </AnimatePresence>
    </Teleport>

    <!-- Settings Dialog -->
    <Teleport to="body">
      <AnimatePresence>
        <div
          v-if="showSettings"
          class="fixed inset-0 z-[90] flex items-center justify-center"
          @click.self="showSettings = false"
        >
          <motion.div
            class="bg-bg-surface rounded-card border border-hairline shadow-terminal-lg p-6"
            style="width: 420px; max-width: 90vw"
            :initial="{ opacity: 0, scale: 0.92, y: 20 }"
            :animate="{ opacity: 1, scale: 1, y: 0 }"
            :exit="{ opacity: 0, scale: 0.92, y: 20 }"
            :transition="{ duration: 0.2, ease: 'easeOut' }"
          >
          <div class="flex items-center justify-between mb-4">
            <span class="text-caption-strong text-white font-mono">⚙ 设置</span>
            <button @click="showSettings = false" class="text-ink-muted hover:text-white cursor-pointer">✕</button>
          </div>

          <label class="block mb-2">
            <span class="text-caption text-ink-muted">后端 API 地址</span>
            <input
              v-model="apiUrlInput"
              class="w-full mt-1 bg-bg border border-hairline rounded-input px-3 py-2 text-caption text-white font-mono outline-none focus:border-ai/50 transition-colors"
              placeholder="http://localhost:8080"
              @keydown.enter="saveSettings"
            />
          </label>

          <p class="text-caption text-ink-muted mb-1">
            状态：
            <span :class="settingsStatus === 'ok' ? 'text-down' : settingsStatus === 'error' ? 'text-up' : 'text-ink-muted'">
              {{ settingsStatusText }}
            </span>
          </p>

          <div class="flex gap-2 mt-4">
            <button
              @click="testConnection"
              class="flex-1 px-3 py-2 bg-bg-hover text-caption text-ink-muted hover:text-white rounded-card transition-colors cursor-pointer font-mono"
            >
              测试连接
            </button>
            <button
              @click="saveSettings"
              class="flex-1 px-3 py-2 bg-ai/20 text-ai rounded-card text-caption font-mono hover:bg-ai/30 transition-colors cursor-pointer"
            >
              保存
            </button>
          </div>

          <p class="text-caption text-ink-muted mt-3 text-xs">
            桌面端默认 <code class="text-ai/80">http://localhost:8080</code>。<br>如果后端部署在远程服务器，请输入完整地址，如 <code class="text-ai/80">http://your-server:8080</code>。
          </p>
          </motion.div>
        </div>
      </AnimatePresence>
    </Teleport>

    <!-- K-line Drawer -->
    <AnimatePresence>
      <KlineDrawer
        v-if="klineSymbol"
        :symbol="klineSymbol"
        :name="klineName"
        @close="klineSymbol = ''"
      />
    </AnimatePresence>
  </div>
</template>

<script setup lang="ts">
import { ref, computed, provide } from 'vue'
import { motion, AnimatePresence } from 'motion-v'
import { overview, fetchOverview, sentimentHistory, fetchSentimentHistory } from '@/stores/data'
import KpiCards from './components/KpiCards.vue'
import KlineDrawer from './components/KlineDrawer.vue'
import SentimentHistory from './components/SentimentHistory.vue'
import TabBar from './components/TabBar.vue'
import AiChatPanel from './components/AiSidebar.vue'

import TabStrongStocks from './components/tabs/TabStrongStocks.vue'
import TabPlate from './components/tabs/TabPlate.vue'
import TabMarketIntel from './components/tabs/TabMarketIntel.vue'
import TabTimeline from './components/tabs/TabTimeline.vue'
import TabPromotion from './components/tabs/TabPromotion.vue'
import TabSentimentChip from './components/tabs/TabSentimentChip.vue'
import TabNews from './components/tabs/TabNews.vue'

const activeTab = ref('strong-stocks')
const showAiChat = ref(false)
const showSettings = ref(false)

function onGlobalKeydown(e: KeyboardEvent) {
  if (e.key === 'Escape') {
    if (showAiChat.value) showAiChat.value = false
    else if (showSettings.value) showSettings.value = false
  }
}

if (typeof window !== 'undefined') {
  window.addEventListener('keydown', onGlobalKeydown)
}

// ── Settings ──────────────────────────────────────────

const apiUrlInput = ref(localStorage.getItem('trading-agent-api-url') || '')
const settingsStatus = ref<'idle' | 'ok' | 'error'>('idle')
const settingsStatusText = computed(() => {
  if (settingsStatus.value === 'ok') return '连接成功'
  if (settingsStatus.value === 'error') return '连接失败'
  return '未测试'
})

async function testConnection() {
  const url = apiUrlInput.value.trim() || 'http://localhost:8080'
  try {
    const resp = await fetch(`${url}/api/overview`)
    settingsStatus.value = resp.ok ? 'ok' : 'error'
  } catch {
    settingsStatus.value = 'error'
  }
}

function saveSettings() {
  const url = apiUrlInput.value.trim()
  if (url) {
    localStorage.setItem('trading-agent-api-url', url)
  } else {
    localStorage.removeItem('trading-agent-api-url')
  }
  showSettings.value = false
  fetchOverview() // re-fetch with new URL
}

// K-line drawer state
const klineSymbol = ref('')
const klineName = ref('')
function openKline(symbol: string, name: string) {
  klineSymbol.value = symbol
  klineName.value = name
}
provide('openKline', openKline)

const tabComponents: Record<string, any> = {
  'strong-stocks': TabStrongStocks,
  'plate': TabPlate,
  'promotion': TabPromotion,
  'timeline': TabTimeline,
  'market-intel': TabMarketIntel,
  'sentiment-chip': TabSentimentChip,
  'news': TabNews,
}

const currentTabComponent = computed(() => tabComponents[activeTab.value])

const sentimentChartData = computed(() =>
  sentimentHistory.data.map(d => ({ date: d.date, value: d.sentiment }))
)

const refreshData = () => { fetchOverview(); fetchSentimentHistory() }

fetchOverview()
fetchSentimentHistory()
</script>