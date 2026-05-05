<template>
  <div>
    <h2 class="text-xl font-bold mb-4">📝 淘股吧</h2>

    <div v-if="users.length === 0" class="text-gray-400 py-12 text-center">
      <p class="text-lg">暂无淘股吧数据</p>
      <p class="text-sm mt-2">数据将通过定时任务生成静态文件后加载</p>
    </div>

    <div v-else class="space-y-3">
      <div 
        v-for="(user, idx) in users" 
        :key="idx"
        class="p-4 bg-white rounded-lg shadow-sm hover:shadow-md transition"
      >
        <div class="flex items-center justify-between mb-3">
          <div class="flex items-center gap-3">
            <span class="w-10 h-10 rounded-full bg-blue-500 flex items-center justify-center text-white font-bold">
              {{ user.nickname.slice(0, 2) }}
            </span>
            <div>
              <h3 class="font-bold">{{ user.nickname }}</h3>
              <p class="text-sm text-gray-500">{{ user.post_count }} 篇帖子</p>
            </div>
          </div>
          <span class="text-sm text-gray-400">{{ user.latest_update }}</span>
        <p class="text-sm text-gray-600 mt-2">置顶：{{ user.top_post_title }}</p>
        </div>
        
        <details class="mt-2">
          <summary class="cursor-pointer text-sm text-blue-600 hover:text-blue-800">
            查看详情
          </summary>
          <div class="mt-2 p-3 bg-gray-50 rounded text-sm text-gray-700">
            <p>短线情绪分析、市场判断等历史观点摘要...</p>
            <p class="text-xs text-gray-500 mt-2">点击「刷新数据」可更新最新方法论</p>
          </div>
        </details>
      </div>
    </div>
  </div>
</template>

<script setup lang="ts">
import { computed } from 'vue'
import { useReportStore } from '../../../stores/report'

const store = useReportStore()

const users = computed(() => store.report?.taoguba_users || [])
</script>
