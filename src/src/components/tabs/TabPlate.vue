<template>
  <div>
    <div class="flex items-center gap-3 mb-8 text-caption">
      <span class="text-white/60">
        <b class="text-white">{{ plateEvents.length }}</b> 板块异动
      </span>
      <span class="text-white/20">|</span>
      <span class="text-white/60">
        <b class="text-white">{{ stockEvents.length }}</b> 个股异动
      </span>
    </div>

    <!-- Plate Events -->
    <div class="mb-10" v-if="plateEvents.length">
      <h3 class="text-caption-strong text-white/40 mb-4 tracking-wider uppercase">板块事件</h3>
      <div class="space-y-3">
        <div v-for="event in plateEvents" :key="event.id"
          class="bg-surface-dark rounded-card border border-hairline-dark pl-4 py-3 pr-4 flex gap-4 items-start">
          <div class="w-1 self-stretch rounded-full flex-shrink-0"
            :class="event.pcp >= 0 ? 'bg-data-red' : 'bg-data-green'"></div>
          <div class="flex-1 min-w-0">
            <div class="flex items-center gap-2 mb-2 flex-wrap">
              <span class="text-body-strong text-white">{{ event.plate_name }}</span>
              <span class="text-caption-strong font-mono" :class="event.pcp >= 0 ? 'text-data-red' : 'text-data-green'">
                {{ formatPcp(event.pcp) }}
              </span>
            </div>
            <div class="flex gap-1 flex-wrap mt-2" v-if="event.stocks.length">
              <span v-for="stock in event.stocks" :key="stock.symbol"
                class="px-2 py-0.5 rounded-badge text-caption font-mono bg-white/10 text-white/60">
                {{ stock.symbol }} {{ stock.name }}
              </span>
            </div>
          </div>
          <span class="text-caption font-mono flex-shrink-0 text-white/40">{{ formatTime(event.event_timestamp) }}</span>
        </div>
      </div>
    </div>

    <!-- Stock Events -->
    <div v-if="stockEvents.length">
      <h3 class="text-caption-strong text-white/40 mb-4 tracking-wider uppercase">个股事件</h3>
      <div class="bg-surface-dark rounded-card border border-hairline-dark overflow-hidden">
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
              <td class="font-mono text-white/40">{{ formatTime(event.event_timestamp) }}</td>
              <td class="font-mono cursor-pointer hover:underline text-primary" @click="openKline(event.symbol, event.name)">{{ event.symbol }}</td>
              <td class="font-medium text-white">{{ event.name }}</td>
              <td class="font-mono text-right" :class="event.pcp >= 0 ? 'text-data-red' : 'text-data-green'">{{ formatPcp(event.pcp) }}</td>
              <td class="font-mono text-right text-white/60">{{ event.mtm?.toFixed(2) || '-' }}</td>
              <td>
                <span class="px-2 py-0.5 rounded-badge text-caption font-mono"
                  :class="event.good_or_bad === 1
                    ? 'bg-data-green/15 text-data-green'
                    : 'bg-data-red/15 text-data-red'">
                  {{ event.good_or_bad === 1 ? '利好' : '利空' }}
                </span>
              </td>
            </tr>
          </tbody>
        </table>
      </div>
    </div>

    <div v-if="plateEvents.length === 0 && stockEvents.length === 0" class="text-center py-12 text-caption text-white/40">
      暂无异动数据
    </div>
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
      id: e.id, plate_name: d.plate_name, pcp: d.pcp ?? 0,
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
      id: e.id, symbol: d.symbol || '-', name: d.name, pcp: d.pcp ?? 0,
      mtm: d.mtm ?? null, event_timestamp: e.event_timestamp, good_or_bad: e.good_or_bad ?? 0,
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
