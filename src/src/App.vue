<template>
  <div class="min-h-screen bg-canvas">
    <!-- Global Nav -->
    <header class="fixed top-0 w-full z-50 bg-black h-11">
      <div class="max-w-[1200px] mx-auto px-6 h-full flex items-center justify-between">
        <div class="flex items-center gap-2 flex-shrink-0">
          <h1 class="text-nav-link font-semibold text-white tracking-tight">
            TRENDING<span class="text-primary">.AGENT</span>
          </h1>
        </div>

        <div class="flex items-center gap-4">
          <span class="text-nav-link font-mono px-2 py-0.5 rounded-badge"
            :class="dataSource === 'api'
              ? 'bg-data-green/10 text-data-green'
              : 'bg-primary/10 text-primary'">
            {{ dataSource.toUpperCase() }}
          </span>
          <span v-if="reportStore.report" class="text-nav-link font-mono text-white/60">
            {{ reportStore.report.date }}
          </span>
          <button
            @click="refreshData"
            :disabled="reportStore.loading"
            class="px-4 py-1.5 text-nav-link font-medium rounded-lg transition-all active:scale-95"
            :class="reportStore.loading
              ? 'bg-white/10 text-white/30 cursor-not-allowed'
              : 'bg-ink text-white hover:bg-ink/90 cursor-pointer'"
          >
            {{ reportStore.loading ? '刷新中...' : '↻ 刷新' }}
          </button>
        </div>
      </div>
    </header>

    <!-- Main Content -->
    <main class="pt-11">
      <!-- Error -->
      <div v-if="reportStore.error" class="py-4">
        <div class="max-w-[1200px] mx-auto px-6">
          <div class="bg-data-red/5 border border-data-red/20 rounded-card px-5 py-3 flex items-center justify-between">
            <div class="flex items-center gap-3">
              <span class="text-data-red text-sm">&#9888;</span>
              <span class="text-caption text-data-red">{{ reportStore.error }}</span>
            </div>
            <button @click="refreshData" class="text-caption text-data-red underline cursor-pointer">重试</button>
          </div>
        </div>
      </div>

      <!-- Loading -->
      <div v-if="reportStore.loading && !reportStore.report" class="flex flex-col items-center justify-center py-32 gap-4">
        <div class="w-3 h-3 rounded-full bg-primary animate-pulse"></div>
        <span class="text-caption font-mono text-ink-muted">加载市场数据...</span>
      </div>

      <!-- Content -->
      <template v-if="reportStore.report">
        <!-- KPI Tile: white canvas -->
        <section class="bg-canvas py-section-y">
          <div class="max-w-[980px] mx-auto px-6">
            <KpiCards
              :overview="reportStore.report.overview"
              :risk-level="reportStore.report.risk_level"
              :decision-summary="reportStore.report.decision_summary"
            />
          </div>
        </section>

        <!-- Tab Tile: parchment + sticky -->
        <section class="bg-canvas-parchment sticky z-40 top-11 p-0">
          <div class="max-w-[1200px] mx-auto px-6">
            <TabBar v-model="activeTab" />
          </div>
        </section>

        <!-- Data Tile: dark -->
        <section class="bg-surface-dark py-section-y min-h-[60vh]">
          <div class="max-w-[1200px] mx-auto px-6">
            <component :is="currentTabComponent" />
          </div>
        </section>
      </template>
    </main>

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
import { ref, computed, provide } from 'vue'
import { useReportStore, fetchReport } from '@/stores/report'
import KpiCards from './components/KpiCards.vue'
import KlineDrawer from './components/KlineDrawer.vue'
import TabBar from './components/TabBar.vue'
import TabLimitUp from './components/tabs/TabLimitUp.vue'
import TabPlate from './components/tabs/TabPlate.vue'
import TabSentiment from './components/tabs/TabSentiment.vue'
import TabTimeline from './components/tabs/TabTimeline.vue'
import TabAi from './components/tabs/TabAi.vue'
import TabTgb from './components/tabs/TabTgb.vue'
import TabPromotion from './components/tabs/TabPromotion.vue'

const reportStore = useReportStore()
const activeTab = ref('limit-up')
const dataSource = computed(() => reportStore.source)
// K-line drawer state
const klineSymbol = ref('')
const klineName = ref('')
function openKline(symbol: string, name: string) {
  klineSymbol.value = symbol
  klineName.value = name
}
provide('openKline', openKline)

const tabComponents: Record<string, any> = {
  'limit-up': TabLimitUp,
  'plate': TabPlate,
  'promotion': TabPromotion,
  'timeline': TabTimeline,
  'sentiment': TabSentiment,
  'ai': TabAi,
  'tgb': TabTgb,
}

const currentTabComponent = computed(() => tabComponents[activeTab.value])

const refreshData = () => fetchReport()

fetchReport()
</script>
