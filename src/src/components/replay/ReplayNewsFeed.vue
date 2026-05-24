<template>
  <div class="h-full flex flex-col">
    <div class="flex items-center justify-between mb-2">
      <span class="text-xs text-white/30">7×24 资讯</span>
      <span class="text-xs text-white/20 font-mono">{{ visibleNews.length }} 条</span>
    </div>
    <div
      ref="listRef"
      class="flex-1 overflow-y-auto custom-scroll space-y-0 pr-1"
      @scroll="onScroll"
    >
      <div
        v-for="item in visibleNews"
        :key="item.id"
        class="px-2 py-2.5 border-b border-hairline-dark transition-opacity duration-300"
        :class="item._future ? 'opacity-40' : ''"
      >
        <div class="text-[10px] font-mono text-white/30 mb-0.5">{{ item._time_label }}</div>
        <p class="text-xs text-white/80 leading-relaxed line-clamp-2">{{ item.title }}</p>
        <p
          v-if="item.content && item.content !== item.title"
          class="text-[11px] text-white/40 leading-relaxed mt-0.5 line-clamp-2"
        >{{ item.content }}</p>
      </div>
      <div v-if="visibleNews.length === 0" class="text-white/20 text-xs py-8 text-center">
        暂无资讯
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
import { computed, ref, watch, nextTick } from 'vue'

interface NewsItem {
  id: number
  timestamp: number
  title: string
  content: string
  _time_label: string
  _future: boolean
}

const props = defineProps<{
  news: NewsItem[]
  currentTime: number
  isPlaying: boolean
}>()

const listRef = ref<HTMLElement>()
const showBackToLatest = ref(false)
let userScrolled = false

const visibleNews = computed(() => {
  return props.news.map((n) => ({
    ...n,
    _future: props.currentTime > 0 && n.timestamp > props.currentTime,
  }))
})

function onScroll() {
  if (!listRef.value) return
  const el = listRef.value
  const atBottom = el.scrollHeight - el.scrollTop - el.clientHeight < 40
  if (atBottom) {
    userScrolled = false
    showBackToLatest.value = false
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
  () => props.currentTime,
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