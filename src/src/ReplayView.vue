<template>
  <div class="pt-11 min-h-screen bg-surface text-white font-sans">
    <!-- Header -->
    <div class="flex items-center justify-between px-4 py-2 border-b border-hairline-dark bg-black">
      <div>
        <span class="text-sm font-bold">{{ data?.date || '--' }}</span>
        <span class="text-xs text-white/30 ml-2">上证指数 · 行情回放</span>
      </div>
      <div class="flex items-center gap-2">
        <span
          v-if="isIntraday"
          class="text-[10px] px-2 py-0.5 rounded-full bg-ai/20 text-ai animate-pulse"
        >盘中</span>
        <button
          v-if="isEnded || isPaused"
          class="px-4 py-1.5 rounded-button bg-ai text-white text-xs font-bold hover:opacity-90"
          @click="startAiAnalysis"
        >
          AI 分析
        </button>
      </div>
    </div>

    <!-- Intraday Banner -->
    <div
      v-if="isIntraday && data"
      class="px-4 py-1.5 bg-ai/10 border-b border-ai/20 text-ai text-xs flex items-center gap-1.5"
    >
      <span class="inline-block w-1.5 h-1.5 rounded-full bg-ai animate-pulse"></span>
      今日行情尚未收盘，数据持续更新中
    </div>

    <!-- Skeleton Loading -->
    <div v-if="dataLoading" class="p-4 space-y-4">
      <div class="flex items-center gap-3">
        <div class="skeleton w-8 h-8 rounded"></div>
        <div class="skeleton flex-1 h-2 rounded"></div>
        <div class="skeleton w-12 h-4 rounded"></div>
        <div class="skeleton w-14 h-6 rounded"></div>
      </div>
      <div class="grid grid-cols-2 gap-4">
        <div class="skeleton h-[280px] rounded-lg"></div>
        <div class="skeleton h-[280px] rounded-lg"></div>
      </div>
      <div class="skeleton h-[200px] rounded-lg"></div>
      <div class="skeleton h-[120px] rounded-lg"></div>
    </div>

    <template v-else-if="data">
      <!-- Controls -->
      <div class="px-4 py-2 flex items-center gap-3 border-b border-hairline-dark">
        <button
          @click="togglePlay"
          class="w-8 h-8 flex items-center justify-center rounded-button bg-white/10 hover:bg-white/20 text-white text-sm"
        >
          {{ isPlaying ? '⏸' : '▶' }}
        </button>
        <input
          type="range"
          :min="rangeMin"
          :max="rangeMax"
          :value="currentTs"
          @input="seekTo(Number(($event.target as HTMLInputElement).value))"
          class="flex-1 h-1 accent-ai"
        />
        <span class="font-mono text-xs text-white/60 w-12 text-right">{{ currentLabel }}</span>
        <select
          v-model="speed"
          class="bg-surface-dark border border-hairline-dark rounded text-xs text-white/80 px-2 py-1"
        >
          <option :value="1">1×</option>
          <option :value="2">2×</option>
          <option :value="5">5×</option>
          <option :value="10">10×</option>
        </select>
        <button @click="reset" class="text-xs text-white/40 hover:text-white/60 px-2 py-1">重置</button>
      </div>

      <!-- Main Layout -->
      <div class="grid grid-cols-2 gap-0 border-b border-hairline-dark">
        <!-- Left: Emotion Curve -->
        <div class="border-r border-hairline-dark p-3">
          <p class="text-xs text-white/30 mb-2">情绪温度</p>
          <EmotionCurve :snapshots="data.snapshots" :currentTime="currentTs" />
        </div>
        <!-- Right: Event Timeline -->
        <div class="p-3" style="height: 332px">
          <p class="text-xs text-white/30 mb-2">异动事件</p>
          <EventTimeline :events="displayEvents" :currentTime="currentTs" />
        </div>
      </div>

      <!-- Plate Heatmap -->
      <div class="border-b border-hairline-dark p-3">
        <p class="text-xs text-white/30 mb-2">板块热力图</p>
        <PlateHeatmap :topPlates="currentPlates" />
      </div>

      <!-- Bull/Bear Bar -->
      <div class="flex items-center gap-4 p-3">
        <div class="flex-1">
          <p class="text-xs text-white/30 mb-2">多空比</p>
          <BullBearChart :bullCount="currentBull" :bearCount="currentBear" />
        </div>
      </div>
    </template>

    <!-- AI Analysis Panel -->
    <AiAnalysis
      :open="aiPanelOpen"
      :loading="aiLoading"
      :text="aiText"
      @close="aiPanelOpen = false"
    />
  </div>
</template>

<script setup lang="ts">
import { ref, computed, onMounted, onUnmounted } from 'vue'
import EmotionCurve from './components/replay/EmotionCurve.vue'
import PlateHeatmap from './components/replay/PlateHeatmap.vue'
import EventTimeline from './components/replay/EventTimeline.vue'
import BullBearChart from './components/replay/BullBearChart.vue'
import AiAnalysis from './components/replay/AiAnalysis.vue'

interface RelatedStock {
  symbol: string
  name: string
  mtm: number
  pcp: number
}

interface Snapshot {
  timestamp: number
  time_label: string
  emotion_score: number
  bull_count: number
  bear_count: number
  top_plates: Array<{
    plate_name: string
    pcp: number
    good_or_bad: number
    related_stocks: RelatedStock[]
  }>
}

interface RawEvent {
  id: number
  target: string
  event_type: number
  event_timestamp: number
  good_or_bad: number
  plate_abnormal_event_data?: {
    plate_name?: string
    pcp?: number
  }
}

interface ReplayData {
  date: string
  events: RawEvent[]
  snapshots: Snapshot[]
  market_phase: string
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
}

const data = ref<ReplayData | null>(null)
const dataLoading = ref(true)

const isIntraday = computed(() => {
  return data.value?.market_phase === 'morningopen' || data.value?.market_phase === 'afternoonopen'
})

// ── Playback State ──────────────────────────────────────

const isPlaying = ref(false)
const isPaused = ref(false)
const isEnded = ref(false)
const currentTs = ref(0)
const speed = ref(1)
let timer: ReturnType<typeof setInterval> | null = null

const MIN_TS = 9 * 3600 + 30 * 60
const MAX_TS = 15 * 3600

const rangeMin = computed(() => {
  if (!data.value?.snapshots.length) return MIN_TS
  const ts = data.value.snapshots[0].timestamp
  return ts - (ts % 86400) + MIN_TS
})

const rangeMax = computed(() => {
  if (!data.value?.events.length) return MAX_TS
  const last = data.value.events[data.value.events.length - 1].event_timestamp
  return last - (last % 86400) + Math.max(last % 86400, MAX_TS)
})

const currentLabel = computed(() => {
  const secs = currentTs.value % 86400
  const h = Math.floor(secs / 3600)
  const m = Math.floor((secs % 3600) / 60)
  return `${String(h).padStart(2, '0')}:${String(m).padStart(2, '0')}`
})

// ── Derived Data ────────────────────────────────────────

const currentSnapshot = computed(() => {
  const snap = data.value?.snapshots
  if (!snap?.length) return null
  for (let i = snap.length - 1; i >= 0; i--) {
    if (snap[i].timestamp <= currentTs.value) return snap[i]
  }
  return null
})

const currentBull = computed(() => currentSnapshot.value?.bull_count ?? 0)
const currentBear = computed(() => currentSnapshot.value?.bear_count ?? 0)
const currentPlates = computed(() => currentSnapshot.value?.top_plates ?? [])

const displayEvents = computed<DisplayEvent[]>(() => {
  const events = data.value?.events ?? []
  return events
    .filter((e) => e.event_type === 11001)
    .map((e) => ({
      id: e.id,
      target: e.target,
      good_or_bad: e.good_or_bad,
      _time_label: unixToHhmm(e.event_timestamp),
      _plate_name: e.plate_abnormal_event_data?.plate_name ?? e.target,
      _pcp: e.plate_abnormal_event_data?.pcp ?? 0,
      _highlight: false,
      event_timestamp: e.event_timestamp,
    }))
})

// ── Fetch ───────────────────────────────────────────────

async function fetchData() {
  dataLoading.value = true
  try {
    const isTauri = '__TAURI_INTERNALS__' in window
    if (isTauri) {
      const { invoke } = await import('@tauri-apps/api/core')
      data.value = await invoke('api_fetch', { path: '/api/replay/events' })
    } else {
      const base = localStorage.getItem('trading-agent-api-url') || ''
      const resp = await fetch(`${base}/api/replay/events`)
      if (!resp.ok) throw new Error(`${resp.status}`)
      data.value = await resp.json()
    }
    if (data.value?.events.length) {
      currentTs.value = rangeMin.value
    }
  } catch (e: any) {
    console.error('Failed to load replay data:', e)
  } finally {
    dataLoading.value = false
  }
}

// ── Playback Logic ──────────────────────────────────────

function findNextEventTime(): number | null {
  const events = data.value?.events
  if (!events?.length) return null
  for (const ev of events) {
    if (ev.event_timestamp > currentTs.value) return ev.event_timestamp
  }
  return null
}

function advancePlayback() {
  const step = speed.value
  for (let i = 0; i < step; i++) {
    const t = findNextEventTime()
    if (t) {
      currentTs.value = t
    } else {
      isPlaying.value = false
      isEnded.value = true
      return
    }
  }
}

function togglePlay() {
  if (isEnded.value) {
    reset()
    return
  }
  isPlaying.value = !isPlaying.value
  isPaused.value = !isPlaying.value
  if (isPlaying.value) {
    startTimer()
  } else {
    stopTimer()
  }
}

function startTimer() {
  stopTimer()
  const tickMs = 300
  timer = setInterval(advancePlayback, tickMs)
}

function stopTimer() {
  if (timer) {
    clearInterval(timer)
    timer = null
  }
}

function seekTo(ts: number) {
  currentTs.value = Number(ts)
  isPaused.value = true
  isPlaying.value = false
  isEnded.value = false
  stopTimer()
}

function reset() {
  currentTs.value = rangeMin.value
  isPlaying.value = false
  isPaused.value = false
  isEnded.value = false
  stopTimer()
}

// ── AI Analysis ─────────────────────────────────────────

const aiPanelOpen = ref(false)
const aiLoading = ref(false)
const aiText = ref('')

async function startAiAnalysis() {
  aiPanelOpen.value = true
  aiLoading.value = true
  aiText.value = ''

  const isTauri = '__TAURI_INTERNALS__' in window
  if (isTauri) {
    const { invoke, Channel } = await import('@tauri-apps/api/core')
    const channel = new Channel<string>()
    channel.onmessage = (text) => {
      aiLoading.value = false
      if (text.startsWith('[ERROR]')) {
        aiText.value += `\n\n> ⚠️ ${text}`
        return
      }
      aiText.value += text
    }
    try {
      await invoke('api_replay_analyze', { channel })
    } catch (e: any) {
      aiText.value += `\n\n> ❌ ${e}`
      aiLoading.value = false
    }
    return
  }

  const base = localStorage.getItem('trading-agent-api-url') || ''
  try {
    const resp = await fetch(`${base}/api/replay/analyze`, { method: 'POST' })
    if (!resp.ok) throw new Error(`${resp.status}`)
    const reader = resp.body?.getReader()
    const decoder = new TextDecoder()
    let buffer = ''
    while (reader) {
      const { done, value } = await reader.read()
      if (done) break
      buffer += decoder.decode(value, { stream: true })
      const lines = buffer.split('\n')
      buffer = lines.pop() || ''
      for (const line of lines) {
        if (line.startsWith('data: ')) {
          const d = line.slice(6)
          if (d === '[DONE]') {
            aiLoading.value = false
            return
          }
          if (d.startsWith('[ERROR]')) {
            aiText.value += `\n\n> ⚠️ ${d}`
            continue
          }
          aiLoading.value = false
          aiText.value += d
        }
      }
    }
    aiLoading.value = false
  } catch (e: any) {
    aiText.value += `\n\n> ❌ ${e}`
    aiLoading.value = false
  }
}

// ── Helpers ─────────────────────────────────────────────

function unixToHhmm(ts: number): string {
  const secs = ((ts % 86400) + 86400) % 86400
  const h = Math.floor(secs / 3600)
  const m = Math.floor((secs % 3600) / 60)
  return `${String(h).padStart(2, '0')}:${String(m).padStart(2, '0')}`
}

// ── Lifecycle ───────────────────────────────────────────

onMounted(() => {
  fetchData()
})

onUnmounted(() => {
  stopTimer()
})
</script>

<style scoped>
.skeleton {
  background: linear-gradient(90deg, rgba(255, 255, 255, 0.04) 25%, rgba(255, 255, 255, 0.08) 50%, rgba(255, 255, 255, 0.04) 75%);
  background-size: 200% 100%;
  animation: skeleton-shimmer 1.5s ease-in-out infinite;
}

@keyframes skeleton-shimmer {
  0% {
    background-position: 200% 0;
  }
  100% {
    background-position: -200% 0;
  }
}
</style>