<template>
  <div class="min-h-screen" style="background: var(--bg-primary)">
    <!-- Top Bar -->
    <header class="fixed top-0 w-full z-50 border-b" style="background: var(--bg-secondary); border-color: var(--border-subtle)">
      <div class="max-w-7xl mx-auto px-6 h-14 flex items-center justify-between gap-4">
        <div class="flex items-center gap-3 flex-shrink-0">
          <span class="pulse-dot"></span>
          <h1 class="text-sm font-bold tracking-wider" style="color: var(--text-primary); font-family: var(--font-mono)">
            TRENDING<span style="color: var(--blue)">.AGENT</span>
          </h1>
        </div>

        <!-- API Address Input (only in api mode) -->
        <div v-if="dataSource === 'api'" class="flex items-center gap-2 flex-1 max-w-sm">
          <input
            :value="apiUrl"
            @change="onApiUrlChange"
            placeholder="http://localhost:8080"
            class="flex-1 px-3 py-1 text-xs rounded border"
            style="background: var(--bg-primary); color: var(--text-primary); border-color: var(--border-subtle); font-family: var(--font-mono); outline: none"
          />
        </div>

        <div class="flex items-center gap-4 flex-shrink-0">
          <!-- Source indicator -->
          <span class="text-xs px-2 py-0.5 rounded font-mono"
            :style="{ background: sourceBg, color: sourceColor }">
            {{ dataSource.toUpperCase() }}
          </span>
          <span v-if="reportStore.report" class="text-xs" style="color: var(--text-muted); font-family: var(--font-mono)">
            {{ reportStore.report.date }}
          </span>
          <button
            @click="refreshData"
            :disabled="reportStore.loading"
            class="px-4 py-1.5 text-xs font-medium rounded border transition-all"
            :style="{
              background: reportStore.loading ? 'transparent' : 'rgba(68,138,255,0.1)',
              color: reportStore.loading ? 'var(--text-muted)' : 'var(--blue)',
              borderColor: reportStore.loading ? 'var(--border-subtle)' : 'rgba(68,138,255,0.3)',
              cursor: reportStore.loading ? 'not-allowed' : 'pointer'
            }"
          >
            {{ reportStore.loading ? '刷新中...' : '↻ 刷新' }}
          </button>
        </div>
      </div>
    </header>

    <!-- Main -->
    <main class="max-w-7xl mx-auto px-6 pt-20 pb-12">
      <!-- Error -->
      <div v-if="reportStore.error" class="mb-6 p-4 rounded-lg flex items-center justify-between" style="background: rgba(255,23,68,0.08); border: 1px solid rgba(255,23,68,0.2)">
        <div class="flex items-center gap-3">
          <span style="color: var(--red); font-size: 16px">&#9888;</span>
          <span class="text-sm" style="color: var(--red)">{{ reportStore.error }}</span>
        </div>
        <button @click="refreshData" class="text-xs underline" style="color: var(--red)">重试</button>
      </div>

      <!-- Loading -->
      <div v-if="reportStore.loading && !reportStore.report" class="flex flex-col items-center justify-center py-32 gap-4">
        <div class="pulse-dot" style="width: 12px; height: 12px"></div>
        <span class="text-sm" style="color: var(--text-muted); font-family: var(--font-mono)">加载市场数据...</span>
      </div>

      <!-- Content -->
      <template v-if="reportStore.report">
        <KpiCards
          :overview="reportStore.report.overview"
          :risk-level="reportStore.report.risk_level"
          :decision-summary="reportStore.report.decision_summary"
        />

        <TabBar v-model="activeTab" />

        <div class="mt-6">
          <component :is="currentTabComponent" />
        </div>
      </template>
    </main>
  </div>
</template>

<script setup lang="ts">
import { ref, computed } from 'vue'
import { useReportStore, fetchReport, getApiBaseUrlForDisplay, setApiBaseUrl } from '@/stores/report'
import KpiCards from './components/KpiCards.vue'
import TabBar from './components/TabBar.vue'
import TabLimitUp from './components/tabs/TabLimitUp.vue'
import TabPlate from './components/tabs/TabPlate.vue'
import TabSentiment from './components/tabs/TabSentiment.vue'
import TabAi from './components/tabs/TabAi.vue'
import TabTgb from './components/tabs/TabTgb.vue'

const reportStore = useReportStore()
const activeTab = ref('limit-up')
const dataSource = computed(() => reportStore.source)
const apiUrl = ref(getApiBaseUrlForDisplay())

const tabComponents: Record<string, any> = {
  'limit-up': TabLimitUp,
  'plate': TabPlate,
  'sentiment': TabSentiment,
  'ai': TabAi,
  'tgb': TabTgb,
}

const currentTabComponent = computed(() => tabComponents[activeTab.value])

const sourceBg = computed(() => {
  switch (dataSource.value) {
    case 'api': return 'rgba(0,230,118,0.1)'
    case 'wasm': return 'rgba(68,138,255,0.1)'
    default: return 'rgba(255,171,0,0.1)'
  }
})

const sourceColor = computed(() => {
  switch (dataSource.value) {
    case 'api': return 'var(--green)'
    case 'wasm': return 'var(--blue)'
    default: return 'var(--amber)'
  }
})

function onApiUrlChange(e: Event) {
  const val = (e.target as HTMLInputElement).value.trim()
  if (val) {
    setApiBaseUrl(val)
    apiUrl.value = val
  }
}

const refreshData = () => fetchReport()

fetchReport()
</script>
