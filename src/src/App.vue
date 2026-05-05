<template>
  <div class="min-h-screen bg-gray-100">
    <!-- Top Nav -->
    <nav class="bg-gray-800 text-white shadow-lg fixed top-0 w-full z-50">
      <div class="container mx-auto px-4 py-3 flex justify-between items-center">
        <div class="flex items-center gap-3">
          <span class="text-xl">📊</span>
          <h1 class="text-lg font-bold">A股市场异动追踪</h1>
        </div>
        <div class="flex gap-2">
          <button 
            @click="refreshData" 
            :disabled="loading"
            class="px-4 py-2 bg-blue-600 hover:bg-blue-700 rounded-lg text-sm transition"
          >
            {{ loading ? '刷新中...' : '🔄 刷新数据' }}
          </button>
          <button class="px-4 py-2 bg-gray-600 hover:bg-gray-700 rounded-lg text-sm transition">
            📝 导出报告
          </button>
        </div>
      </div>
    </nav>

    <!-- Main Content -->
    <main class="container mx-auto px-4 pt-24 pb-8">
      <!-- Error State -->
      <div v-if="reportStore.error" class="bg-red-50 border border-red-200 rounded-lg p-4 mb-4 max-w-4xl mx-auto">
        <p class="text-red-700 text-sm">⚠️ 数据获取失败：{{ reportStore.error }}</p>
        <button @click="refreshData" class="mt-2 text-sm text-red-600 underline">重试</button>
      </div>

      <!-- Loading State -->
      <div v-if="reportStore.loading && !reportStore.report" class="text-center py-20 text-gray-400">
        <p class="text-lg animate-pulse">加载中...</p>
      </div>

      <!-- KPI Cards -->
      <KpiCards v-if="reportStore.report" :overview="reportStore.report.overview" />

      <!-- Tab Bar -->
      <TabBar v-model="activeTab" />

      <!-- Tab Content -->
      <div class="mt-6">
        <component :is="currentTabComponent" />
      </div>
    </main>

    <!-- Footer -->
    <footer class="bg-gray-800 text-gray-400 text-center py-4 mt-12">
      <p class="text-sm">trending-agent v0.1.0 | 数据更新: {{ reportStore.report?.date || '暂无' }}</p>
    </footer>
  </div>
</template>

<script setup lang="ts">
import { ref, computed } from 'vue'
import { useReportStore } from './stores/report'
import KpiCards from './components/KpiCards.vue'
import TabBar from './components/TabBar.vue'
import TabLimitUp from './components/tabs/TabLimitUp.vue'
import TabPlate from './components/tabs/TabPlate.vue'
import TabSentiment from './components/tabs/TabSentiment.vue'
import TabAi from './components/tabs/TabAi.vue'
import TabTgb from './components/tabs/TabTgb.vue'

const reportStore = useReportStore()
const activeTab = ref('limit-up')

const tabComponents = {
  'limit-up': TabLimitUp,
  'plate': TabPlate,
  'sentiment': TabSentiment,
  'ai': TabAi,
  'tgb': TabTgb
}

const currentTabComponent = computed(() => tabComponents[activeTab.value])

const refreshData = () => {
  reportStore.fetchReport()
}

// Initial load
reportStore.fetchReport()
</script>
