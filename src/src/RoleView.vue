<template>
  <div class="pt-11 min-h-screen bg-bg">
    <div class="max-w-[1200px] mx-auto px-6 py-8">
      <h2 class="text-xl font-bold text-white mb-6">角色策略</h2>
      <!-- Loading state -->
      <div v-if="roles.loading" class="flex items-center justify-center py-20">
        <div class="w-3 h-3 rounded-full bg-ai animate-pulse"></div>
        <span class="text-caption text-ink-muted ml-3">加载角色数据...</span>
      </div>
      <!-- Error state -->
      <div v-else-if="roles.error" class="text-center py-20 text-up">
        加载失败: {{ roles.error }}
      </div>
      <!-- Cards -->
      <div v-else class="flex flex-col gap-4">
        <div
          v-for="role in roles.data"
          :key="role.name"
          class="bg-bg-surface border border-hairline rounded-card p-5"
        >
          <!-- Card Header -->
          <div class="flex items-center justify-between mb-4">
            <div class="flex items-center gap-2">
              <span class="w-2 h-2 rounded-full bg-ai"></span>
              <span class="text-white font-semibold">{{ role.display_name }}</span>
              <span class="text-caption text-ink-muted font-mono">({{ role.name }})</span>
            </div>
            <!-- Total P&L -->
            <span
              :class="totalPnls[role.name] >= 0 ? 'text-up' : 'text-down'"
              class="font-mono font-semibold"
              v-if="rolePositions[role.name]?.data?.length"
            >
              {{ totalPnls[role.name] >= 0 ? '+' : '' }}{{ totalPnls[role.name].toFixed(1) }}%
            </span>
            <span v-else class="text-ink-muted text-caption">空仓</span>
          </div>

          <!-- Strategy -->
          <div class="mb-4">
            <button
              @click="toggleStrategy(role.name)"
              class="text-caption text-ai hover:text-ai/80 transition-colors cursor-pointer font-mono"
            >
              {{ expandedStrategies[role.name] ? '收起 ▲' : '展开 ▼' }}
            </button>
            <div
              v-if="expandedStrategies[role.name]"
              class="mt-2 bg-bg rounded-card p-4 max-h-[300px] overflow-y-auto text-caption text-white/70 leading-relaxed whitespace-pre-wrap font-mono"
            >
              {{ role.strategy }}
            </div>
            <p v-else class="mt-1 text-caption text-ink-muted truncate max-w-2xl">
              {{ role.strategy.split('\n').slice(0, 3).join(' ') }}...
            </p>
          </div>

          <!-- Positions + Logs row -->
          <div class="flex gap-4">
            <!-- Positions -->
            <div class="flex-1 min-w-0">
              <div class="text-caption-strong text-ink-muted mb-2 uppercase tracking-wider">持仓</div>
              <div v-if="!rolePositions[role.name]?.loading && !rolePositions[role.name]?.data?.length" class="text-caption text-ink-muted">
                暂无持仓
              </div>
              <table v-else-if="rolePositions[role.name]?.data?.length" class="w-full text-caption">
                <thead>
                  <tr class="text-ink-muted text-left">
                    <th class="pb-2 font-normal">代码</th>
                    <th class="pb-2 font-normal">名称</th>
                    <th class="pb-2 font-normal text-right">盈亏%</th>
                    <th class="pb-2 font-normal text-right">成本/现价</th>
                    <th class="pb-2 font-normal text-right">股数</th>
                    <th class="pb-2 font-normal">理由</th>
                  </tr>
                </thead>
                <tbody>
                  <tr v-for="pos in rolePositions[role.name].data" :key="pos.id" class="border-t border-hairline">
                    <td class="py-2 font-mono text-white/70">{{ pos.symbol }}</td>
                    <td class="py-2 text-white/70">{{ pos.stock_name }}</td>
                    <td class="py-2 text-right font-mono" :class="pnl(pos) >= 0 ? 'text-up' : 'text-down'">
                      {{ pnl(pos) >= 0 ? '+' : '' }}{{ pnl(pos).toFixed(1) }}%
                    </td>
                    <td class="py-2 text-right font-mono text-white/50">
                      {{ pos.cost_price.toFixed(2) }} / {{ (pos.current_price || 0).toFixed(2) }}
                    </td>
                    <td class="py-2 text-right font-mono text-white/70">{{ pos.shares }}</td>
                    <td class="py-2 text-white/50 max-w-[200px] truncate">{{ pos.entry_reason }}</td>
                  </tr>
                </tbody>
              </table>
            </div>

            <!-- Trade Logs -->
            <div class="w-[280px] flex-shrink-0">
              <div class="text-caption-strong text-ink-muted mb-2 uppercase tracking-wider">最近交易</div>
              <div v-if="!roleLogs[role.name]?.loading && !roleLogs[role.name]?.data?.length" class="text-caption text-ink-muted">
                暂无交易记录
              </div>
              <div v-else-if="roleLogs[role.name]?.data?.length" class="space-y-1.5">
                <div
                  v-for="(log, i) in roleLogs[role.name].data.slice(0, 5)"
                  :key="i"
                  class="bg-bg rounded-card px-3 py-1.5 text-caption"
                >
                  <div class="flex items-center justify-between">
                    <span class="text-ink-muted">{{ log.date }}</span>
                    <span :class="log.action === '买入' || log.action === 'add' ? 'text-up' : 'text-down'">{{ log.action }}</span>
                  </div>
                  <div class="flex items-center justify-between mt-0.5">
                    <span class="text-white/70 font-mono">{{ log.symbol }}</span>
                    <span class="text-white/50">¥{{ (log.price || 0).toFixed(2) }}</span>
                  </div>
                  <div v-if="log.reason" class="text-ink-muted text-xs mt-0.5 truncate">{{ log.reason }}</div>
                </div>
              </div>
            </div>
          </div>
        </div>
      </div>
    </div>
  </div>
</template>

<script setup lang="ts">
import { ref, computed, onMounted } from 'vue'
import { roles, fetchRoles, rolePositions, fetchRolePositions, roleLogs, fetchRoleLogs, type RolePosition } from '@/stores/data'

const expandedStrategies = ref<Record<string, boolean>>({})

function toggleStrategy(name: string) {
  expandedStrategies.value[name] = !expandedStrategies.value[name]
}

function pnl(pos: RolePosition): number {
  if (!pos.current_price || !pos.cost_price) return 0
  return ((pos.current_price - pos.cost_price) / pos.cost_price) * 100
}

// Total P&L for each role
const totalPnls = computed(() => {
  const result: Record<string, number> = {}
  for (const role of roles.data) {
    const positions = rolePositions[role.name]?.data || []
    if (positions.length === 0) continue
    let totalCost = 0, totalValue = 0
    for (const p of positions) {
      totalCost += p.cost_price * p.shares
      totalValue += (p.current_price || 0) * p.shares
    }
    result[role.name] = totalCost > 0 ? ((totalValue - totalCost) / totalCost) * 100 : 0
  }
  return result
})

onMounted(async () => {
  await fetchRoles()
  for (const role of roles.data) {
    fetchRolePositions(role.name)
    fetchRoleLogs(role.name)
  }
})
</script>
