<template>
  <div class="h-full flex flex-col relative">
    <div class="flex items-center justify-between mb-2">
      <span class="text-xs text-white/30">板块异动</span>
      <span class="text-xs text-white/20 font-mono">{{ visibleEvents.length }} 条</span>
    </div>
    <div
      ref="listRef"
      class="flex-1 overflow-y-auto custom-scroll space-y-0 pr-1"
      @scroll="onScroll"
    >
      <div
        v-for="ev in visibleEvents"
        :key="ev.id"
        class="px-2 py-2.5 border-b border-hairline-dark transition-all duration-300 cursor-pointer"
        :class="[
          ev._highlight ? 'bg-white/[0.06]' : '',
          ev._future ? 'opacity-40' : '',
        ]"
        @click="$emit('seekTo', ev.event_timestamp)"
      >
        <div class="flex items-center gap-2 mb-1">
          <span class="font-mono text-[10px] text-white/30 shrink-0">{{ ev._time_label }}</span>
          <span
            class="shrink-0 px-1.5 py-0.5 border font-medium text-[11px]"
            :class="ev.good_or_bad === 1
              ? 'border-up/60 text-up'
              : ev.good_or_bad === 2
                ? 'border-down/60 text-down'
                : 'border-white/20 text-white/50'"
          >{{ ev._plate_name }}</span>
          <span
            class="shrink-0 px-1 py-0.5 rounded text-[10px] font-bold"
            :class="ev.good_or_bad === 1 ? 'bg-up/15 text-up' : ev.good_or_bad === 2 ? 'bg-down/15 text-down' : 'bg-white/5 text-white/40'"
          >{{ ev.good_or_bad === 1 ? '利好' : ev.good_or_bad === 2 ? '利空' : '-' }}</span>
          <span
            class="font-mono text-xs ml-auto shrink-0"
            :class="ev._pcp >= 0 ? 'text-up' : 'text-down'"
          >{{ ev._pcp >= 0 ? '+' : '' }}{{ (ev._pcp * 100).toFixed(2) }}%</span>
        </div>
        <div v-if="ev._related_stocks.length > 0" class="mt-1.5 space-y-0.5 pl-2">
          <div
            v-for="(stock, si) in ev._related_stocks.slice(0, 3)"
            :key="stock.symbol"
            class="flex items-center gap-2 text-[11px]"
          >
            <span class="shrink-0 w-4 h-4 rounded-full flex items-center justify-center text-[9px] font-bold bg-white/5 text-white/50">{{ si + 1 }}</span>
            <span class="text-white/70 truncate flex-1">{{ stock.name }}</span>
            <span class="font-mono shrink-0 whitespace-nowrap" :class="stock.mtm >= 0 ? 'text-up' : 'text-down'">
              涨速{{ stock.mtm >= 0 ? '+' : '' }}{{ (stock.mtm * 100).toFixed(2) }}%
            </span>
            <span class="font-mono shrink-0 whitespace-nowrap" :class="stock.pcp >= 0 ? 'text-up' : 'text-down'">
              涨幅{{ stock.pcp >= 0 ? '+' : '' }}{{ (stock.pcp * 100).toFixed(2) }}%
            </span>
          </div>
        </div>
      </div>
      <div v-if="loadingMore" class="text-white/20 text-xs py-4 text-center">
        加载中...
      </div>
      <div v-else-if="!hasMore && visibleEvents.length > 0" class="text-white/10 text-[10px] py-3 text-center">
        — 已加载全部 —
      </div>
      <div v-else-if="visibleEvents.length === 0" class="text-white/20 text-xs py-8 text-center">
        暂无事件
      </div>
    </div>
    <button
      v-if="showBackToLatest"
      class="absolute bottom-2 left-1/2 -translate-x-1/2 px-3 py-1 rounded-full bg-ai/20 text-ai text-[10px] font-bold hover:bg-ai/30 transition-colors z-10"
      @click="scrollToLatest"
    >
      回到最新 ↓
    </button>
  </div>
</template>

<script setup lang="ts">
import { computed, watch, ref, nextTick } from 'vue'

interface RelatedStock {
  symbol: string
  name: string
  mtm: number
  pcp: number
}

interface DisplayEvent {
  id: number
  target: string
  good_or_bad: number
  _time_label: string
  _plate_name: string
  _pcp: number
  _highlight: boolean
  event_timestamp: number
  _future: boolean
  _related_stocks: RelatedStock[]
}

const props = defineProps<{
  events: DisplayEvent[]
  currentTime: number
  isPlaying: boolean
  hasMore: boolean
  loadingMore: boolean
}>()

const emit = defineEmits<{
  seekTo: [ts: number]
  loadMore: []
}>()

const listRef = ref<HTMLElement>()
const showBackToLatest = ref(false)
let userScrolled = false

const visibleEvents = computed(() => {
  let list = props.events.map((e) => ({
    ...e,
    _future: props.currentTime > 0 && e.event_timestamp > props.currentTime,
  }))
  if (props.currentTime > 0) {
    list = list.filter((e) => e.event_timestamp <= props.currentTime || e._future)
  }
  const played = list.filter((e) => !e._future)
  if (played.length > 0) {
    const latest = played[played.length - 1]
    list = list.map((e) => ({
      ...e,
      _highlight: e.id === latest.id,
    }))
  }
  return list.slice(-100)
})

function onScroll() {
  if (!listRef.value) return
  const el = listRef.value
  const atBottom = el.scrollHeight - el.scrollTop - el.clientHeight < 40
  if (atBottom) {
    userScrolled = false
    showBackToLatest.value = false
    if (props.hasMore && !props.loadingMore) emit('loadMore')
  } else if (props.isPlaying) {
    userScrolled = true
    showBackToLatest.value = true
  }
}

function scrollToLatest() {
  userScrolled = false
  showBackToLatest.value = false
  if (listRef.value) {
    listRef.value.scrollTop = listRef.value.scrollHeight
  }
}

watch(
  () => visibleEvents.value.length,
  async () => {
    if (userScrolled) return
    await nextTick()
    if (listRef.value) {
      listRef.value.scrollTop = listRef.value.scrollHeight
    }
  },
)

watch(
  () => props.isPlaying,
  (playing) => {
    if (playing) {
      userScrolled = false
      showBackToLatest.value = false
    }
  },
)
</script>
