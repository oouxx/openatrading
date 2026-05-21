<template>
  <div class="pt-11 min-h-screen bg-bg">
    <div class="flex h-[calc(100vh-44px)]">
      <!-- Left Panel: Role List (320px) -->
      <div class="w-[320px] flex-shrink-0 border-r border-hairline overflow-y-auto px-4 py-6 flex flex-col gap-3">
        <h2 class="text-lg font-bold text-white px-1 mb-1">角色策略</h2>

        <div v-if="roles.loading" class="flex items-center gap-2 px-1 py-4">
          <div class="w-2 h-2 rounded-full bg-ai animate-pulse"></div>
          <span class="text-caption text-ink-muted">加载中...</span>
        </div>

        <div v-else-if="roles.error" class="text-up text-caption px-1 py-4">{{ roles.error }}</div>

        <template v-else>
          <div
            v-for="role in roles.data"
            :key="role.name"
            @click="selectRole(role.name)"
            class="rounded-card p-3 cursor-pointer transition-colors border"
            :class="selectedRole === role.name
              ? 'bg-ai/10 border-ai/30'
              : 'bg-bg-surface border-hairline hover:bg-white/[0.03]'"
          >
            <div class="flex items-center justify-between mb-1">
              <div class="flex items-center gap-2">
                <span class="w-2 h-2 rounded-full bg-ai flex-shrink-0"></span>
                <span class="text-white font-semibold text-sm truncate">{{ role.display_name }}</span>
              </div>
            </div>
            <div class="flex items-center justify-between text-caption">
              <span v-if="rolePnls[role.name] !== undefined" :class="rolePnls[role.name] >= 0 ? 'text-up' : 'text-down'" class="font-mono">
                {{ rolePnls[role.name] >= 0 ? '+' : '' }}{{ rolePnls[role.name].toFixed(1) }}%
              </span>
              <span v-else class="text-ink-muted">空仓</span>
              <span class="text-ink-muted">{{ rolePositionCounts[role.name] || 0 }}支</span>
            </div>
            <div class="mt-2 flex justify-center">
              <Sparkline
                v-if="rolePnlHistories[role.name]?.length"
                :points="rolePnlHistories[role.name]"
                :width="220"
                :height="28"
              />
              <span v-else class="text-ink-muted text-xs">暂无数据</span>
            </div>
          </div>

          <button
            @click="showCreateForm = true"
            class="rounded-card p-3 border border-dashed border-hairline text-ink-muted hover:text-ai hover:border-ai/30 transition-colors text-sm text-center cursor-pointer"
          >
            + 新增角色
          </button>

          <div v-if="showCreateForm" class="bg-bg-surface border border-hairline rounded-card p-4 space-y-3">
            <input v-model="createForm.name" placeholder="角色名 (英文)" class="w-full bg-bg border border-hairline rounded px-3 py-2 text-white text-sm font-mono" />
            <input v-model="createForm.displayName" placeholder="显示名 (中文)" class="w-full bg-bg border border-hairline rounded px-3 py-2 text-white text-sm" />
            <textarea v-model="createForm.strategy" placeholder="初始策略 (Markdown)" rows="4" class="w-full bg-bg border border-hairline rounded px-3 py-2 text-white text-sm font-mono resize-none"></textarea>
            <div class="flex gap-2">
              <button @click="doCreateRole" class="px-4 py-1.5 bg-ai/20 text-ai rounded text-sm hover:bg-ai/30 transition-colors">创建</button>
              <button @click="showCreateForm = false" class="px-4 py-1.5 text-ink-muted text-sm hover:text-white transition-colors">取消</button>
            </div>
          </div>
        </template>
      </div>

      <!-- Right Panel: Role Detail -->
      <div class="flex-1 overflow-y-auto px-8 py-6">
        <div v-if="!selectedRole" class="flex items-center justify-center h-full text-ink-muted text-sm">
          请选择一个角色
        </div>

        <template v-else>
          <!-- Action Bar -->
          <div class="flex items-center justify-between mb-6">
            <div>
              <h2 class="text-xl font-bold text-white">{{ currentRole?.display_name }}</h2>
              <div class="flex items-center gap-4 mt-1 text-caption text-ink-muted">
                <span>累计 <span :class="(rolePnls[selectedRole] || 0) >= 0 ? 'text-up' : 'text-down'" class="font-mono">
                  {{ (rolePnls[selectedRole] || 0) >= 0 ? '+' : '' }}{{ (rolePnls[selectedRole] || 0).toFixed(1) }}%
                </span></span>
                <span>持仓 {{ rolePositionCounts[selectedRole] || 0 }} 支</span>
                <span>初始资金 ¥1,000,000</span>
              </div>
            </div>
            <div class="flex gap-2">
              <button
                @click="editingStrategy = !editingStrategy"
                class="px-3 py-1.5 border border-hairline rounded text-sm text-ink-muted hover:text-white hover:border-white/20 transition-colors"
              >
                {{ editingStrategy ? '取消编辑' : '编辑策略' }}
              </button>
              <button
                @click="doResetRole"
                class="px-3 py-1.5 border border-hairline rounded text-sm text-ink-muted hover:text-up hover:border-up/30 transition-colors"
              >
                重置账户
              </button>
              <button
                @click="doDeleteRole"
                class="px-3 py-1.5 border border-hairline rounded text-sm text-ink-muted hover:text-up hover:border-up/30 transition-colors"
              >
                删除角色
              </button>
            </div>
          </div>

          <!-- Profit Curve -->
          <div class="bg-bg-surface border border-hairline rounded-card p-5 mb-6">
            <div class="flex items-center justify-between mb-3">
              <h3 class="text-white font-semibold text-sm">盈利曲线</h3>
              <div class="flex gap-1">
                <button v-for="d in [7, 30, 0]" :key="d" @click="pnlDays = d"
                  class="px-2 py-0.5 rounded text-xs transition-colors"
                  :class="pnlDays === d ? 'bg-ai/20 text-ai' : 'text-ink-muted hover:text-white'"
                >
                  {{ d === 0 ? '全部' : d + '日' }}
                </button>
              </div>
            </div>
            <div v-if="rolePnlHistory[selectedRole]?.loading" class="text-center py-10 text-ink-muted text-sm">加载中...</div>
            <div v-else-if="!rolePnlHistory[selectedRole]?.data?.length" class="text-center py-10 text-ink-muted text-sm">暂无交易数据</div>
            <div v-else ref="chartContainerRef" class="w-full h-[300px]"></div>
          </div>

          <!-- Positions Table -->
          <div class="bg-bg-surface border border-hairline rounded-card p-5 mb-6">
            <h3 class="text-white font-semibold text-sm mb-3">持仓明细</h3>
            <div v-if="!rolePositions[selectedRole]?.loading && !rolePositions[selectedRole]?.data?.length" class="text-caption text-ink-muted py-4 text-center">
              暂无持仓
            </div>
            <table v-else-if="rolePositions[selectedRole]?.data?.length" class="w-full text-caption">
              <thead>
                <tr class="text-ink-muted text-left">
                  <th class="pb-2 font-normal">代码</th>
                  <th class="pb-2 font-normal">名称</th>
                  <th class="pb-2 font-normal text-right">盈亏%</th>
                  <th class="pb-2 font-normal text-right">成本/现价</th>
                  <th class="pb-2 font-normal text-right">股数</th>
                  <th class="pb-2 font-normal text-right">持仓天数</th>
                  <th class="pb-2 font-normal">理由</th>
                </tr>
              </thead>
              <tbody>
                <tr v-for="pos in rolePositions[selectedRole].data" :key="pos.id" class="border-t border-hairline">
                  <td class="py-2 font-mono text-white/70">{{ pos.symbol }}</td>
                  <td class="py-2 text-white/70">{{ pos.stock_name }}</td>
                  <td class="py-2 text-right font-mono" :class="pnl(pos) >= 0 ? 'text-up' : 'text-down'">
                    {{ pnl(pos) >= 0 ? '+' : '' }}{{ pnl(pos).toFixed(1) }}%
                  </td>
                  <td class="py-2 text-right font-mono text-white/50">
                    {{ pos.cost_price.toFixed(2) }} / {{ (pos.current_price || 0).toFixed(2) }}
                  </td>
                  <td class="py-2 text-right font-mono text-white/70">{{ pos.shares }}</td>
                  <td class="py-2 text-right font-mono text-white/50">{{ daysSince(pos.entry_date) }}天</td>
                  <td class="py-2 text-white/50 max-w-[200px] truncate">{{ pos.entry_reason }}</td>
                </tr>
              </tbody>
            </table>
          </div>

          <!-- Strategy Section -->
          <div class="bg-bg-surface border border-hairline rounded-card p-5">
            <h3 class="text-white font-semibold text-sm mb-3">策略</h3>

            <div v-if="!editingStrategy">
              <button @click="toggleStrategy" class="text-caption text-ai hover:text-ai/80 transition-colors font-mono">
                {{ strategyExpanded ? '收起 ▲' : '展开 ▼' }}
              </button>
              <div v-if="strategyExpanded" class="mt-2 bg-bg rounded-card p-4 max-h-[400px] overflow-y-auto text-caption text-white/70 leading-relaxed max-w-none">
                <div v-html="renderMarkdown(currentRole?.strategy || '')"></div>
              </div>
              <p v-else class="mt-1 text-caption text-ink-muted truncate max-w-2xl">
                {{ (currentRole?.strategy || '').split('\n').slice(0, 3).join(' ') }}...
              </p>
            </div>

            <div v-else>
              <textarea
                v-model="strategyDraft"
                rows="20"
                class="w-full bg-bg border border-hairline rounded-card p-4 text-caption text-white/70 font-mono resize-none focus:border-ai/30 focus:outline-none"
              ></textarea>
              <div class="flex gap-2 mt-3">
                <button @click="doSaveStrategy" class="px-4 py-1.5 bg-ai/20 text-ai rounded text-sm hover:bg-ai/30 transition-colors">保存</button>
                <button @click="editingStrategy = false" class="px-4 py-1.5 text-ink-muted text-sm hover:text-white transition-colors">取消</button>
              </div>
            </div>
          </div>
        </template>
      </div>
    </div>

    <!-- Confirm Dialog -->
    <div v-if="confirmAction" class="fixed inset-0 z-50 flex items-center justify-center bg-black/60" @click.self="confirmAction = null">
      <div class="bg-bg-surface border border-hairline rounded-card p-6 max-w-sm w-full">
        <p class="text-white text-sm mb-4">{{ confirmMessage }}</p>
        <div class="flex gap-2 justify-end">
          <button @click="confirmAction = null" class="px-4 py-1.5 border border-hairline rounded text-sm text-ink-muted hover:text-white transition-colors">取消</button>
          <button @click="executeConfirm" class="px-4 py-1.5 bg-up/20 text-up rounded text-sm hover:bg-up/30 transition-colors">确认</button>
        </div>
      </div>
    </div>
  </div>
</template>

<script setup lang="ts">
import { ref, computed, onMounted, watch, nextTick } from 'vue'
import {
  roles, fetchRoles,
  rolePositions, fetchRolePositions,
  fetchRoleLogs,
  rolePnlHistory, fetchRolePnlHistory,
  createRole, updateRoleStrategy, deleteRole, resetRole,
  type RolePosition, type PnlPoint
} from '@/stores/data'
import Sparkline from '@/components/Sparkline.vue'
import { createChart, ColorType } from 'lightweight-charts'

// ── State ──────────────────────────────────────────────

const selectedRole = ref<string | null>(null)
const strategyExpanded = ref(false)
const editingStrategy = ref(false)
const strategyDraft = ref('')
const showCreateForm = ref(false)
const pnlDays = ref(30)

const createForm = ref({ name: '', displayName: '', strategy: '# 新角色\n\n## 策略\n\n' })

const confirmAction = ref<'reset' | 'delete' | null>(null)
const confirmMessage = computed(() =>
  confirmAction.value === 'reset'
    ? `确认重置 ${currentRole.value?.display_name} 账户？将清空所有持仓和交易记录，恢复初始资金 ¥1,000,000。此操作不可撤销。`
    : `确认删除 ${currentRole.value?.display_name}？将删除该角色及所有相关持仓和交易记录。此操作不可撤销。`
)

const chartContainerRef = ref<HTMLDivElement>()
let chart: any = null

// ── Computed ────────────────────────────────────────────

const currentRole = computed(() => roles.data.find(r => r.name === selectedRole.value))

const rolePnls = computed(() => {
  const result: Record<string, number> = {}
  for (const role of roles.data) {
    const positions = rolePositions[role.name]?.data || []
    if (!positions.length) continue
    let totalCost = 0, totalValue = 0
    for (const p of positions) {
      totalCost += p.cost_price * p.shares
      totalValue += (p.current_price || 0) * p.shares
    }
    result[role.name] = totalCost > 0 ? ((totalValue - totalCost) / totalCost) * 100 : 0
  }
  return result
})

const rolePositionCounts = computed(() => {
  const result: Record<string, number> = {}
  for (const role of roles.data) {
    result[role.name] = (rolePositions[role.name]?.data || []).length
  }
  return result
})

const rolePnlHistories = computed(() => {
  const result: Record<string, PnlPoint[]> = {}
  for (const role of roles.data) {
    result[role.name] = rolePnlHistory[role.name]?.data || []
  }
  return result
})

// ── Methods ─────────────────────────────────────────────

function pnl(pos: RolePosition): number {
  if (!pos.current_price || !pos.cost_price) return 0
  return ((pos.current_price - pos.cost_price) / pos.cost_price) * 100
}

function daysSince(dateStr: string): number {
  if (!dateStr) return 0
  const d = new Date(dateStr)
  const now = new Date()
  return Math.floor((now.getTime() - d.getTime()) / 86400000)
}

function renderMarkdown(md: string): string {
  return md
    .replace(/^### (.+)$/gm, '<h4 class="text-white font-semibold mt-4 mb-1">$1</h4>')
    .replace(/^## (.+)$/gm, '<h3 class="text-ai font-semibold mt-4 mb-2">$1</h3>')
    .replace(/^# (.+)$/gm, '<h3 class="text-ai font-bold mt-3 mb-2 text-sm">$1</h3>')
    .replace(/\*\*(.+?)\*\*/g, '<strong class="text-white/90">$1</strong>')
    .replace(/^\- (.+)$/gm, '<li class="ml-4 text-white/60">$1</li>')
    .replace(/\n\n/g, '<br/><br/>')
    .replace(/\n/g, '<br/>')
}

function toggleStrategy() {
  strategyExpanded.value = !strategyExpanded.value
}

async function selectRole(name: string) {
  selectedRole.value = name
  strategyExpanded.value = false
  editingStrategy.value = false
  await refreshRole(name)
  await nextTick()
  renderChart()
}

async function refreshRole(name: string) {
  await fetchRolePnlHistory(name, pnlDays.value === 0 ? 365 : pnlDays.value)
  await fetchRolePositions(name)
  fetchRoleLogs(name)
}

// ── CRUD ────────────────────────────────────────────────

async function doCreateRole() {
  if (!createForm.value.name.trim()) return
  try {
    const name = createForm.value.name.trim()
    await createRole(name, createForm.value.displayName.trim() || createForm.value.name.trim(), createForm.value.strategy)
    showCreateForm.value = false
    createForm.value = { name: '', displayName: '', strategy: '# 新角色\n\n## 策略\n\n' }
    await fetchRoles()
    await selectRole(name)
  } catch (e: any) {
    alert('创建失败: ' + (e.message || e))
  }
}

async function doSaveStrategy() {
  if (!selectedRole.value) return
  try {
    await updateRoleStrategy(selectedRole.value, strategyDraft.value)
    editingStrategy.value = false
    await fetchRoles()
  } catch (e: any) {
    alert('保存失败: ' + (e.message || e))
  }
}

function doResetRole() {
  confirmAction.value = 'reset'
}

function doDeleteRole() {
  confirmAction.value = 'delete'
}

async function executeConfirm() {
  if (!selectedRole.value) return
  try {
    if (confirmAction.value === 'reset') {
      await resetRole(selectedRole.value)
      delete rolePositions[selectedRole.value]
      delete rolePnlHistory[selectedRole.value]
      await refreshRole(selectedRole.value)
      await nextTick()
      renderChart()
    } else if (confirmAction.value === 'delete') {
      const name = selectedRole.value
      await deleteRole(name)
      delete rolePositions[name]
      delete rolePnlHistory[name]
      selectedRole.value = null
      await fetchRoles()
    }
  } catch (e: any) {
    alert('操作失败: ' + (e.message || e))
  } finally {
    confirmAction.value = null
  }
}

watch(editingStrategy, (editing) => {
  if (editing && currentRole.value) {
    strategyDraft.value = currentRole.value.strategy
  }
})

watch(pnlDays, async (days) => {
  if (selectedRole.value) {
    await fetchRolePnlHistory(selectedRole.value, days === 0 ? 365 : days)
    await nextTick()
    renderChart()
  }
})

// ── Chart ───────────────────────────────────────────────

function renderChart() {
  if (chart) {
    chart.remove()
    chart = null
  }

  const container = chartContainerRef.value
  if (!container || !selectedRole.value) return

  const points = rolePnlHistory[selectedRole.value]?.data || []
  if (!points.length) return

  chart = createChart(container, {
    width: container.clientWidth,
    height: 300,
    layout: {
      background: { type: ColorType.Solid, color: 'transparent' },
      textColor: '#888780',
    },
    grid: {
      vertLines: { color: 'rgba(255,255,255,0.04)' },
      horzLines: { color: 'rgba(255,255,255,0.04)' },
    },
    rightPriceScale: { borderColor: 'rgba(255,255,255,0.08)' },
    timeScale: { borderColor: 'rgba(255,255,255,0.08)', timeVisible: false },
  })

  const returnData = points.map(p => ({
    time: p.date,
    value: p.return_pct,
  }))

  const returnSeries = chart.addAreaSeries({
    lineColor: points.length && points[points.length - 1].return_pct >= 0 ? '#FF5C5C' : '#1D9E75',
    topColor: points.length && points[points.length - 1].return_pct >= 0 ? 'rgba(255,92,92,0.15)' : 'rgba(29,158,117,0.15)',
    bottomColor: 'rgba(0,0,0,0)',
    lineWidth: 2,
    priceFormat: { type: 'custom', formatter: (v: number) => v.toFixed(1) + '%' },
  })
  returnSeries.setData(returnData)

  const mvData = points.map(p => ({
    time: p.date,
    value: p.market_value,
  }))

  chart.addLineSeries({
    color: '#7F77DD',
    lineWidth: 1,
    priceScaleId: 'right',
    priceFormat: { type: 'custom', formatter: (v: number) => (v / 10000).toFixed(1) + '万' },
  }).setData(mvData)

  const cashData = points.map(p => ({
    time: p.date,
    value: p.cash,
  }))

  chart.addLineSeries({
    color: '#888780',
    lineWidth: 1,
    lineStyle: 2,
    priceScaleId: 'right',
    priceFormat: { type: 'custom', formatter: (v: number) => (v / 10000).toFixed(1) + '万' },
  }).setData(cashData)

  chart.timeScale().fitContent()
}

// ── Init ────────────────────────────────────────────────

onMounted(async () => {
  await fetchRoles()
  for (const role of roles.data) {
    fetchRolePositions(role.name)
    fetchRolePnlHistory(role.name, 30)
  }
  if (roles.data.length && !selectedRole.value) {
    selectRole(roles.data[0].name)
  }
})
</script>