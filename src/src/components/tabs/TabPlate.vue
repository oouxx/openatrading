<template>
  <div>
    <div class="flex items-center gap-3 mb-6 text-sm">
      <span style="color: var(--text-muted)">
        <b style="color: var(--text-primary)">{{ plateEvents.length }}</b> 板块异动
      </span>
      <span style="color: var(--border-subtle)">|</span>
      <span style="color: var(--text-muted)">
        <b style="color: var(--text-primary)">{{ stockEvents.length }}</b> 个股异动
      </span>
    </div>

    <!-- Event Timeline: Plate -->
    <div class="mb-6" v-if="plateEvents.length">
      <h3 class="text-xs font-medium mb-3" style="color: var(--text-muted); letter-spacing: 0.5px">板块事件</h3>
      <div class="space-y-2">
        <div v-for="event in plateEvents" :key="event.id" class="glow-card pl-4 py-3 pr-4 flex gap-4 items-start">
          <div class="w-1 self-stretch rounded-full flex-shrink-0"
            :style="{ background: event.pcp >= 0 ? 'var(--red)' : 'var(--green)' }"></div>
          <div class="flex-1 min-w-0">
            <div class="flex items-center gap-2 mb-1 flex-wrap">
              <span class="text-sm font-medium" style="color: var(--text-primary)">{{ event.plate_name }}</span>
              <span class="text-xs font-mono" :style="{ color: event.pcp >= 0 ? 'var(--red)' : 'var(--green)' }">{{ formatPcp(event.pcp) }}</span>
            </div>
            <div class="flex gap-1 flex-wrap mt-2" v-if="event.stocks.length">
              <span v-for="stock in event.stocks" :key="stock.symbol"
                class="px-2 py-0.5 rounded text-xs font-mono"
                style="background: var(--border-subtle); color: var(--text-secondary)">
                {{ stock.symbol }} {{ stock.name }}
              </span>
            </div>
          </div>
          <span class="text-xs flex-shrink-0 font-mono" style="color: var(--text-muted)">{{ formatTime(event.event_timestamp) }}</span>
        </div>
      </div>
    </div>

    <!-- Event Timeline: Stock -->
    <div v-if="stockEvents.length">
      <h3 class="text-xs font-medium mb-3" style="color: var(--text-muted); letter-spacing: 0.5px">个股事件</h3>
      <div class="glow-card overflow-hidden">
        <table class="data-table">
          <thead>
            <tr>
              <th>时间</th>
              <th>代码</th>
              <th>名称</th>
              <th style="text-align:right">涨跌幅</th>
              <th style="text-align:right">动量</th>
              <th>方向</th>
            </tr>
          </thead>
          <tbody>
            <tr v-for="event in stockEvents" :key="event.id">
              <td class="font-mono" style="color: var(--text-muted)">{{ formatTime(event.event_timestamp) }}</td>
              <td class="font-mono cursor-pointer hover:underline" style="color: var(--blue)" @click="openKline(event.symbol, event.name)">{{ event.symbol }}</td>
              <td style="color: var(--text-primary); font-weight: 500">{{ event.name }}</td>
              <td class="font-mono text-right" :style="{ color: event.pcp >= 0 ? 'var(--red)' : 'var(--green)' }">{{ formatPcp(event.pcp) }}</td>
              <td class="font-mono text-right" style="color: var(--text-secondary)">{{ event.mtm?.toFixed(2) || '-' }}</td>
              <td>
                <span class="px-2 py-0.5 rounded text-xs font-mono"
                  :style="{
                    background: event.good_or_bad === 1 ? 'rgba(0,230,118,0.1)' : 'rgba(255,23,68,0.1)',
                    color: event.good_or_bad === 1 ? 'var(--green)' : 'var(--red)'
                  }">
                  {{ event.good_or_bad === 1 ? '利好' : '利空' }}
                </span>
              </td>
            </tr>
          </tbody>
        </table>
      </div>
    </div>

    <div v-if="plateEvents.length === 0 && stockEvents.length === 0" class="text-center py-12 text-sm" style="color: var(--text-muted)">暂无异动数据</div>
  </div>
</template>

<script setup lang="ts">
import { computed, inject } from 'vue'
import { useReportStore } from '@/stores/report'

const store = useReportStore()
const openKline = inject<(symbol: string, name: string) => void>('openKline', () => {})

interface PlateFlatEvent {
  id: number
  plate_name: string
  pcp: number
  event_timestamp: number
  stocks: { symbol: string; name: string }[]
}

interface StockFlatEvent {
  id: number
  symbol: string
  name: string
  pcp: number
  mtm: number | null
  event_timestamp: number
  good_or_bad: number
}

const plateEvents = computed<PlateFlatEvent[]>(() => {
  const events: PlateFlatEvent[] = []
  store.report?.plate_abnormal.forEach((e: any) => {
    const d = e.plate_abnormal_event_data
    if (!d || !d.plate_name) return
    events.push({
      id: e.id,
      plate_name: d.plate_name,
      pcp: d.pcp ?? 0,
      event_timestamp: e.event_timestamp,
      stocks: (d.related_stocks || []).map((s: any) => ({ symbol: s.symbol, name: s.name })),
    })
  })
  return events.sort((a, b) => b.event_timestamp - a.event_timestamp)
})

const stockEvents = computed<StockFlatEvent[]>(() => {
  const events: StockFlatEvent[] = []
  store.report?.plate_abnormal.forEach((e: any) => {
    const d = e.stock_abnormal_event_data
    if (!d || !d.name) return
    events.push({
      id: e.id,
      symbol: d.symbol || '-',
      name: d.name,
      pcp: d.pcp ?? 0,
      mtm: d.mtm ?? null,
      event_timestamp: e.event_timestamp,
      good_or_bad: e.good_or_bad ?? 0,
    })
  })
  return events.sort((a, b) => b.event_timestamp - a.event_timestamp)
})

function formatTime(ts: number): string {
  return new Date(ts * 1000).toLocaleTimeString('zh-CN', { hour: '2-digit', minute: '2-digit' })
}
function formatPcp(pcp: number): string {
  return pcp === 0 ? '0%' : (pcp > 0 ? '+' : '') + (pcp * 100).toFixed(1) + '%'
}
</script>
