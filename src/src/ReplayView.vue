<template>
  <div class="pt-11 min-h-screen bg-surface text-white font-sans">
    <!-- Header -->
    <div class="flex items-center justify-between px-4 py-2 border-b border-hairline-dark bg-black">
      <div>
        <span class="text-sm font-bold">{{ data?.date || '--' }}</span>
        <span class="text-xs text-white/30 ml-2">行情回放</span>
      </div>
      <div class="flex items-center gap-2">
        <span
          v-if="isIntraday"
          class="text-[10px] px-2 py-0.5 rounded-full bg-ai/20 text-ai animate-pulse"
        >盘中</span>
        <button
          @click="toggleMute"
          class="w-8 h-8 flex items-center justify-center rounded-button bg-white/10 hover:bg-white/20 text-white text-sm"
        >
          {{ muted ? '🔇' : '🔊' }}
        </button>
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
      <div class="grid grid-cols-3 gap-4">
        <div class="skeleton h-[400px] rounded-lg"></div>
        <div class="skeleton h-[400px] rounded-lg"></div>
        <div class="skeleton h-[400px] rounded-lg"></div>
      </div>
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

      <!-- Main Layout - Three columns -->
      <div class="grid grid-cols-3 gap-0 border-b border-hairline-dark" style="height: calc(100vh - 140px)">
        <!-- Left: Index Trends + Emotion -->
        <div class="border-r border-hairline-dark flex flex-col p-3 overflow-hidden">
          <div class="flex-1 overflow-hidden">
            <IndexTrend :trendData="indexTrendData" />
          </div>
          <!-- Emotion Temperature Bar -->
          <div class="mt-2 pt-2 border-t border-hairline-dark">
            <div class="flex items-center justify-between mb-1">
              <span class="text-xs text-white/30">情绪温度</span>
              <span
                class="font-mono text-sm font-bold"
                :class="emotionColor"
              >{{ currentEmotion }}</span>
            </div>
            <div class="h-2 rounded-full bg-white/5 overflow-hidden relative">
              <div
                class="h-full rounded-full transition-all duration-500"
                :style="{
                  width: `${currentEmotion}%`,
                  background: emotionGradient,
                }"
              ></div>
            </div>
            <div class="flex justify-between mt-0.5">
              <span class="text-[9px] text-down">冷</span>
              <span class="text-[9px] text-yellow-400">中</span>
              <span class="text-[9px] text-up">热</span>
            </div>
          </div>
        </div>

        <!-- Middle: Event Timeline -->
        <div class="border-r border-hairline-dark p-3 overflow-hidden">
          <EventTimeline
            :events="displayEvents"
            :currentTime="currentTs"
            :isPlaying="isPlaying"
            :hasMore="eventsNextCursor !== null"
            :loadingMore="eventsLoadingMore"
            @seekTo="seekTo"
            @loadMore="loadMoreEvents"
          />
        </div>

        <!-- Right: News Feed -->
        <div class="p-3 overflow-hidden">
          <ReplayNewsFeed
            :news="displayNews"
            :currentTime="currentTs"
            :isPlaying="isPlaying"
            :hasMore="hasMoreNews"
            :newsLoading="newsLoadingMore"
            @loadMore="loadMoreNews"
          />
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
import { ref, computed, onMounted, onUnmounted, watch } from 'vue'
import IndexTrend from './components/replay/IndexTrend.vue'
import EventTimeline from './components/replay/EventTimeline.vue'
import ReplayNewsFeed from './components/replay/ReplayNewsFeed.vue'
import AiAnalysis from './components/replay/AiAnalysis.vue'
import { useSpeech } from './composables/useSpeech'

const { muted, speak, stop: stopSpeech, toggleMute } = useSpeech()

interface RelatedStock {
  symbol: string
  name: string
  mtm: number
  pcp: number
}

interface RawEvent {
  id: number
  event_type: number
  event_timestamp: number
  good_or_bad: number
  plate_name?: string
  pcp?: number
  related_stocks?: RelatedStock[]
}

interface ReplayData {
  date: string
  events: RawEvent[]
  market_phase: string
  next_cursor: number | null
}

interface DisplayEvent {
  id: number
  target: string
  good_or_bad: number
  _time_label: string
  _plate_name: string
  _pcp: number
  _highlight: boolean
  _future: boolean
  event_timestamp: number
  _related_stocks: RelatedStock[]
}

interface IndexTick {
  tick_at: number
  close_px: number
}

interface IndexTrendData {
  sh: IndexTick[]
  sz: IndexTick[]
  cyb: IndexTick[]
}

interface NewsItem {
  Id: number
  CreatedAtInSec: number
  UpdatedAtInSec: number
  Title: string
  Content: string
  Summary: string
}

interface NewsPage {
  items: NewsItem[]
  tailmark: number | null
  msgIdMark: string | null
}

interface DisplayNews extends NewsItem {
  _time_label: string
  _future: boolean
}

const data = ref<ReplayData | null>(null)
const dataLoading = ref(true)
const eventsNextCursor = ref<number | null>(null)
const eventsLoadingMore = ref(false)
const indexTrendData = ref<IndexTrendData | null>(null)
const newsData = ref<NewsItem[]>([])
const newsTailmark = ref<number | null>(null)
const newsMsgIdMark = ref<string | null>(null)
const newsLoadingMore = ref(false)

const hasMoreNews = computed(() => newsTailmark.value !== null)

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

const rangeMin = computed(() => {
  const events = data.value?.events
  if (!events?.length) return 0
  return events[0].event_timestamp
})

const rangeMax = computed(() => {
  const events = data.value?.events
  if (!events?.length) return 0
  return events[events.length - 1].event_timestamp
})

function tsToHhmm(ts: number): string {
  const beijingSecs = ((ts % 86400) + 86400 + 8 * 3600) % 86400
  const h = Math.floor(beijingSecs / 3600)
  const m = Math.floor((beijingSecs % 3600) / 60)
  return `${String(h).padStart(2, '0')}:${String(m).padStart(2, '0')}`
}

function tsToHhmmss(ts: number): string {
  const beijingSecs = ((ts % 86400) + 86400 + 8 * 3600) % 86400
  const h = Math.floor(beijingSecs / 3600)
  const m = Math.floor((beijingSecs % 3600) / 60)
  const s = Math.floor(beijingSecs % 60)
  return `${String(h).padStart(2, '0')}:${String(m).padStart(2, '0')}:${String(s).padStart(2, '0')}`
}

const currentLabel = computed(() => tsToHhmm(currentTs.value))

// ── Derived Data ────────────────────────────────────────

const currentEmotion = computed(() => {
  const events = data.value?.events
  if (!events?.length) return 50
  let bull = 0
  let bear = 0
  for (const ev of events) {
    if (ev.event_timestamp > currentTs.value) break
    if (ev.good_or_bad === 1) bull++
    else if (ev.good_or_bad === 2) bear++
  }
  const total = bull + bear
  if (total === 0) return 50
  return Math.round((bull / total) * 100)
})

const emotionColor = computed(() => {
  const e = currentEmotion.value
  if (e > 60) return 'text-up'
  if (e < 40) return 'text-down'
  return 'text-yellow-400'
})

const emotionGradient = computed(() => {
  return 'linear-gradient(90deg, #52C41A, #FAAD14, #F5222D)'
})

const displayEvents = computed<DisplayEvent[]>(() => {
  const events = data.value?.events ?? []
  return events
    .map((e) => ({
      id: e.id,
      target: e.plate_name ?? '',
      good_or_bad: e.good_or_bad,
      _time_label: tsToHhmmss(e.event_timestamp),
      _plate_name: e.plate_name ?? '',
      _pcp: e.pcp ?? 0,
      _highlight: false,
      _future: false,
      event_timestamp: e.event_timestamp,
      _related_stocks: e.related_stocks ?? [],
    }))
})

const displayNews = computed<DisplayNews[]>(() => {
  return newsData.value.map((n) => ({
    ...n,
    _time_label: tsToHhmmss(n.CreatedAtInSec),
    _future: false,
  }))
})

// ── Speech ──────────────────────────────────────────────

let lastSpokenEventId = 0
let lastEmotionAlert = 0

function speakEvent(ev: RawEvent) {
  if (ev.id === lastSpokenEventId) return
  lastSpokenEventId = ev.id
  if (speed.value >= 5) return
  const plateName = ev.plate_name ?? ''
  const pcp = ev.pcp ?? 0
  const pcpStr = `${(Math.abs(pcp) * 100).toFixed(1)}%`
  if (ev.good_or_bad === 1) {
    speak(`${plateName}拉升，涨幅${pcpStr}`)
  } else if (ev.good_or_bad === 2) {
    speak(`${plateName}跳水，跌幅${pcpStr}`)
  }
}

function checkEmotionAlert() {
  const e = currentEmotion.value
  const now = Date.now()
  if (now - lastEmotionAlert < 30000) return
  if (e > 70) {
    speak('市场情绪偏热，注意风险')
    lastEmotionAlert = now
  } else if (e < 30) {
    speak('市场情绪偏冷，观望为主')
    lastEmotionAlert = now
  }
}

// ── Fetch ───────────────────────────────────────────────

async function fetchJson<T>(path: string): Promise<T> {
  const isTauri = '__TAURI_INTERNALS__' in window
  if (isTauri) {
    const { invoke } = await import('@tauri-apps/api/core')
    return invoke('api_fetch', { path })
  }
  const base = localStorage.getItem('trading-agent-api-url') || ''
  const resp = await fetch(`${base}${path}`)
  if (!resp.ok) throw new Error(`${resp.status}`)
  return resp.json()
}

async function fetchData() {
  dataLoading.value = true
  try {
    const [eventsData, indexData, newsResult] = await Promise.allSettled([
      fetchJson<ReplayData>('/api/replay/events'),
      fetchJson<IndexTrendData>('/api/replay/index'),
      fetchJson<NewsPage>('/api/replay/news'),
    ])

    if (eventsData.status === 'fulfilled') {
      data.value = eventsData.value
      eventsNextCursor.value = eventsData.value.next_cursor
      if (data.value?.events.length) {
        currentTs.value = rangeMin.value
      }
    } else {
      console.error('Failed to load replay events:', eventsData.reason)
    }

    if (indexData.status === 'fulfilled') {
      indexTrendData.value = indexData.value
    } else {
      console.error('Failed to load index trends:', indexData.reason)
    }

    if (newsResult.status === 'fulfilled') {
      newsData.value = newsResult.value.items
      newsTailmark.value = newsResult.value.tailmark
      newsMsgIdMark.value = newsResult.value.msgIdMark
    } else {
      console.error('Failed to load news:', newsResult.reason)
    }
  } catch (e: any) {
    console.error('Failed to load replay data:', e)
  } finally {
    dataLoading.value = false
  }
}

async function loadMoreEvents() {
  if (eventsLoadingMore.value || !eventsNextCursor.value || !data.value) return
  eventsLoadingMore.value = true
  try {
    const page = await fetchJson<ReplayData>(`/api/replay/events?cursor=${eventsNextCursor.value}`)
    data.value.events = [...data.value.events, ...page.events]
    // data.value.events.sort((a, b) => a.event_timestamp - b.event_timestamp)
    eventsNextCursor.value = page.next_cursor
  } catch (e: any) {
    console.error('Failed to load more events:', e)
  } finally {
    eventsLoadingMore.value = false
  }
}

async function loadMoreNews() {
  if (newsLoadingMore.value || !newsTailmark.value) return
  newsLoadingMore.value = true
  try {
    const params = new URLSearchParams()
    params.set('tailmark', String(newsTailmark.value))
    if (newsMsgIdMark.value) params.set('msgIdMark', newsMsgIdMark.value)
    const page = await fetchJson<NewsPage>(`/api/replay/news?${params}`)
    const newItems = page.items.filter(
      (n) => !newsData.value.some((existing) => existing.Id === n.Id),
    )
    newsData.value = [...newsData.value, ...newItems]
    newsTailmark.value = page.tailmark
    newsMsgIdMark.value = page.msgIdMark
  } catch (e: any) {
    console.error('Failed to load more news:', e)
  } finally {
    newsLoadingMore.value = false
  }
}

// ── Playback Logic ──────────────────────────────────────

/** Advance currentTs by market-time seconds, scaled by speed.
 *  1× = real-time: 200ms tick × 0.2s step = 1s market / 1s real. */
function advancePlayback() {
  const tickSecs = 0.2 // market seconds per tick at 1×
  const step = tickSecs * speed.value
  currentTs.value = Math.min(currentTs.value + step, rangeMax.value)
  if (currentTs.value >= rangeMax.value) {
    isPlaying.value = false
    isEnded.value = true
    stopSpeech()
    return
  }
  // Check for notable events at this timestamp for speech
  const events = data.value?.events
  if (events) {
    for (const ev of events) {
      if (ev.event_timestamp > currentTs.value - step && ev.event_timestamp <= currentTs.value) {
        speakEvent(ev)
      }
    }
  }
  checkEmotionAlert()
}

function togglePlay() {
  if (isEnded.value) {
    reset()
    isPlaying.value = true
    startTimer()
    return
  }
  isPlaying.value = !isPlaying.value
  isPaused.value = !isPlaying.value
  if (isPlaying.value) {
    startTimer()
  } else {
    stopTimer()
    stopSpeech()
  }
}

function startTimer() {
  stopTimer()
  const tickMs = 200
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
  stopSpeech()
}

function reset() {
  currentTs.value = rangeMin.value
  isPlaying.value = false
  isPaused.value = false
  isEnded.value = false
  lastSpokenEventId = 0
  lastEmotionAlert = 0
  stopTimer()
  stopSpeech()
}

watch(speed, (s) => {
  if (s >= 5) stopSpeech()
})

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

// ── Lifecycle ───────────────────────────────────────────

onMounted(() => {
  fetchData()
})

onUnmounted(() => {
  stopTimer()
  stopSpeech()
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