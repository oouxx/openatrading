<template>
  <div class="h-full overflow-y-auto custom-scroll space-y-1 pr-1" ref="listRef">
    <div
      v-for="(ev, i) in visibleEvents"
      :key="ev.id"
      class="flex items-center gap-2 px-2 py-1.5 rounded text-xs transition-all duration-300"
      :class="[
        ev._highlight ? 'bg-white/10' : '',
        i === visibleEvents.length - 1 && currentTime > 0 ? 'animate-pulse-once' : '',
      ]"
    >
      <span class="font-mono text-white/40 w-10 shrink-0">{{ ev._time_label }}</span>
      <span
        class="shrink-0 px-1.5 py-0.5 rounded text-[10px] font-bold"
        :class="ev.good_or_bad === 1 ? 'bg-up/20 text-up' : 'bg-down/20 text-down'"
      >{{ ev.good_or_bad === 1 ? '利好' : '利空' }}</span>
      <span class="text-white truncate flex-1">{{ ev._plate_name || ev.target }}</span>
      <span class="font-mono shrink-0" :class="ev._pcp >= 0 ? 'text-up' : 'text-down'">
        {{ ev._pcp >= 0 ? '+' : '' }}{{ (ev._pcp * 100).toFixed(1) }}%
      </span>
    </div>
    <div v-if="visibleEvents.length === 0" class="text-white/20 text-xs py-8 text-center">
      暂无事件
    </div>
  </div>
</template>

<script setup lang="ts">
import { computed, watch, ref, nextTick } from 'vue'

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

const props = defineProps<{
  events: DisplayEvent[]
  currentTime: number
}>()

const listRef = ref<HTMLElement>()

const visibleEvents = computed(() => {
  let list = props.events
  if (props.currentTime > 0) {
    list = list.filter((e) => e.event_timestamp <= props.currentTime)
  }
  if (list.length > 0) {
    const latest = list[list.length - 1]
    list = list.map((e) => ({ ...e, _highlight: e.id === latest.id }))
  }
  return list.slice(-100)
})

watch(
  () => visibleEvents.value.length,
  async () => {
    await nextTick()
    if (listRef.value) {
      listRef.value.scrollTop = listRef.value.scrollHeight
    }
  },
)
</script>

<style scoped>
@keyframes pulse-once {
  0%,
  100% {
    background: transparent;
  }
  50% {
    background: rgba(255, 255, 255, 0.08);
  }
}
.animate-pulse-once {
  animation: pulse-once 0.6s ease-in-out;
}
</style>