<template>
  <div>
    <h2 class="text-xl font-bold mb-4">🤖️ AI 三角色观点</h2>

    <div v-if="analyses.length === 0" class="text-gray-400 py-12 text-center">
      <p class="text-lg">未配置 AI 分析</p>
      <p class="text-sm mt-2">设置 OPENROUTER_API_KEY 环境变量后重新构建即可启用</p>
    </div>

    <div v-else class="grid grid-cols-1 md:grid-cols-3 gap-4">
      <div 
        v-for="(analysis, idx) in analyses" 
        :key="idx"
        class="p-5 rounded-xl shadow-lg border-t-4"
        :class="getRoleBorder(idx)"
      >
        <div class="flex items-center gap-2 mb-3">
          <span 
            class="w-3 h-3 rounded-full"
            :class="getRoleDot(idx)"
          ></span>
          <h3 class="text-lg font-bold">{{ analysis.role }}</h3>
        </div>
        
        <p class="text-gray-700 leading-relaxed mb-3">{{ analysis.content }}</p>
        
        <div class="flex justify-between items-center text-sm text-gray-500">
          <span>⏱️ {{ (analysis.elapsed_ms / 1000).toFixed(1) }}s</span>
          <span class="px-2 py-0.5 bg-gray-100 rounded">AI 生成</span>
        </div>
      </div>
    </div>
  </div>
</template>

<script setup lang="ts">
import { computed } from 'vue'
import { useReportStore } from '../../stores/report'

const store = useReportStore()

const analyses = computed(() => store.report?.analyses || [])

const getRoleBorder = (idx: number) => {
  // 0: 游资操盘手 (橙)
  // 1: 量化分析师 (蓝)
  // 2: 风控官 (灰)
  const borders = ['border-orange-500', 'border-blue-500', 'border-gray-500']
  return borders[idx] || 'border-gray-500'
}

const getRoleDot = (idx: number) => {
  const dots = ['bg-orange-500', 'bg-blue-500', 'bg-gray-500']
  return dots[idx] || 'bg-gray-500'
}
</script>
