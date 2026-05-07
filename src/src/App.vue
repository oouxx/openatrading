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
        <div class="max-w-[1200px] mx-auto px-6">
          <div class="bg-data-red/5 border border-data-red/20 rounded-card px-5 py-3 flex items-center justify-between">
            <div class="flex items-center gap-3">
              <span class="text-data-red text-sm">&#9888;</span>
              <span class="text-caption text-data-red">{{ overview.error }}</span>
            </div>
            <button @click="refreshData" class="text-caption text-data-red underline cursor-pointer">重试</button>
          </div>
        </div>
      </div>

      <!-- Loading -->
      <div v-if="overview.loading && !overview.data" class="flex flex-col items-center justify-center py-32 gap-4">
        <div class="w-3 h-3 rounded-full bg-primary animate-pulse"></div>
        <span class="text-caption font-mono text-ink-muted">加载市场数据...</span>
      </div>

      <!-- Content -->
      <template v-if="overview.data">
        <!-- KPI Tile: white canvas -->
        <section class="bg-canvas py-section-y">
          <div class="max-w-[980px] mx-auto px-6">
            <KpiCards
              :overview="overview.data"
              :risk-level="overview.data.risk_level"
              :decision-summary="overview.data.decision_summary"
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
import { overview, fetchOverview } from '@/stores/data'
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

const activeTab = ref('limit-up')
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

const refreshData = () => fetchOverview()

fetchOverview()
</script>
