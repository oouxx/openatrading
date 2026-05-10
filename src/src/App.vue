<template>
  <div class="min-h-screen bg-bg">
    <!-- Global Nav -->
    <header class="fixed top-0 w-full z-50 bg-black h-11">
      <div class="max-w-[1600px] mx-auto px-6 h-full flex items-center justify-between">
        <div class="flex items-center gap-2 flex-shrink-0">
          <h1 class="text-nav-link font-semibold text-white tracking-tight">
            TRENDING<span class="text-ai">.AGENT</span>
          </h1>
        </div>

        <div class="flex items-center gap-4">
          <span v-if="overview.data" class="text-nav-link font-mono text-white/60">
            {{ overview.data.date }}
          </span>
          <button
            @click="refreshData"
            :disabled="overview.loading"
            class="px-4 py-1.5 text-nav-link font-medium rounded-lg transition-all active:scale-95"
            :class="overview.loading
              ? 'bg-white/10 text-white/30 cursor-not-allowed'
              : 'bg-ink text-white hover:bg-ink/90 cursor-pointer'"
          >
            {{ overview.loading ? '刷新中...' : '↻ 刷新' }}
          </button>
        </div>
      </div>
    </header>

    <!-- Main Content -->
    <main class="pt-11">
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
        <div class="flex max-w-[1600px] mx-auto">
          <!-- Left Column: Main Content (68%) -->
          <div class="flex-1 min-w-0 lg:max-w-[65%]">

            <!-- ① Market Status + ② KPI -->
            <section class="px-6 py-section-y">
              <KpiCards
                :overview="overview.data"
                :risk-level="overview.data.risk_level"
                :decision-summary="overview.data.decision_summary"
                :sentiment-temp="overview.data.sentiment_temp"
                :promotion-rate="overview.data.promotion_rate"
                :promotion-history-avg="overview.data.promotion_history_avg"
                :net-flow="overview.data.net_flow"
                :flow-sector="overview.data.flow_sector"
              />
            </section>

            <!-- ② Sentiment History Chart -->
            <section class="px-6 pb-8">
              <SentimentHistory :data="sentimentHistory" />
            </section>

            <!-- ③ AI Market Summary -->
            <section class="px-6 pb-8">
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
              <div class="px-6">
                <TabBar v-model="activeTab" />
              </div>
            </div>
            <section class="min-h-[60vh] px-6 py-6">
              <component :is="currentTabComponent" />
            </section>
          </div>

          <!-- Right Column: AI Sidebar (desktop 35%, sticky) -->
          <aside
            v-show="sidebarOpen"
            class="hidden lg:block flex-shrink-0 border-l border-hairline transition-all duration-300 sticky top-11 self-start"
            :class="sidebarOpen ? 'w-[35%]' : 'w-0 overflow-hidden border-l-0'"
            style="height: calc(100vh - 44px)"
          >
            <div class="h-full flex flex-col">
              <div class="flex-shrink-0">
                <NewsFeed />
              </div>
              <div class="flex-1 min-h-0">
                <AiSidebar
                  :overview="overview.data"
                  @close="sidebarOpen = false"
                />
              </div>
            </div>
          </aside>

          <!-- Mobile Sidebar Overlay -->
          <Teleport to="body">
            <div
              v-if="sidebarOpen && mobileSidebarVisible"
              class="fixed inset-0 z-50 lg:hidden"
            >
              <div @click="mobileSidebarVisible = false" class="absolute inset-0 bg-black/50"></div>
              <div class="absolute top-11 right-0 h-[calc(100vh-44px)] w-[85vw] max-w-[400px] shadow-terminal-lg border-l border-hairline bg-bg-surface flex flex-col">
                <div class="flex-shrink-0">
                  <NewsFeed />
                </div>
                <div class="flex-1 min-h-0">
                  <AiSidebar
                    :overview="overview.data"
                    @close="mobileSidebarVisible = false"
                  />
                </div>
              </div>
            </div>
          </Teleport>
        </div>
      </template>
    </main>

    <!-- AI Sidebar Toggle (when sidebar collapsed) -->
    <button
      v-if="overview.data && !sidebarOpen"
      @click="sidebarOpen = true"
      class="fixed right-3 top-1/2 -translate-y-1/2 z-30 px-3 py-2 rounded-button
             bg-ai/15 text-ai border border-ai/25 text-caption font-mono
             hover:bg-ai/25 transition-colors cursor-pointer"
    >
      AI
    </button>

    <!-- Mobile AI Sidebar Toggle (always visible on small screens) -->
    <button
      v-if="overview.data && windowWidth < 1024"
      @click="mobileSidebarVisible = true"
      class="fixed right-3 top-1/2 -translate-y-1/2 z-30 px-3 py-2 rounded-button
             bg-ai/15 text-ai border border-ai/25 text-caption font-mono
             hover:bg-ai/25 transition-colors cursor-pointer lg:hidden"
    >
      AI
    </button>

    <!-- K-line Drawer -->
    <KlineDrawer
      v-if="klineSymbol"
      :symbol="klineSymbol"
      :name="klineName"
      @close="klineSymbol = ''"
    />
  </div>
</template>

<script setup lang="ts">
import { ref, computed, provide, onMounted, onUnmounted } from 'vue'
import { overview, fetchOverview } from '@/stores/data'
import KpiCards from './components/KpiCards.vue'
import KlineDrawer from './components/KlineDrawer.vue'
import SentimentHistory from './components/SentimentHistory.vue'
import TabBar from './components/TabBar.vue'
import AiSidebar from './components/AiSidebar.vue'
import NewsFeed from './components/NewsFeed.vue'
import TabStrongStocks from './components/tabs/TabStrongStocks.vue'
import TabPlate from './components/tabs/TabPlate.vue'
import TabMarketIntel from './components/tabs/TabMarketIntel.vue'
import TabTimeline from './components/tabs/TabTimeline.vue'
import TabPromotion from './components/tabs/TabPromotion.vue'
import TabSentimentChip from './components/tabs/TabSentimentChip.vue'

const activeTab = ref('strong-stocks')
const sidebarOpen = ref(true)
const mobileSidebarVisible = ref(false)
const sentimentHistory = ref<{date: string, value: number}[]>([])
const windowWidth = ref(typeof window !== 'undefined' ? window.innerWidth : 1024)

function onResize() { windowWidth.value = window.innerWidth }
onMounted(() => {
  window.addEventListener('resize', onResize)
})
onUnmounted(() => window.removeEventListener('resize', onResize))

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
}

const currentTabComponent = computed(() => tabComponents[activeTab.value])

const refreshData = () => fetchOverview()

fetchOverview()
</script>
