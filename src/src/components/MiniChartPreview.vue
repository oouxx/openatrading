<template>
  <span
    ref="trigger"
    class="inline-block"
    @mouseenter="onEnter"
    @mouseleave="onLeave"
  >
    <slot />
  </span>

  <Teleport to="body">
    <div
      v-if="visible"
      ref="card"
      class="fixed z-[60] bg-bg-surface rounded-card border border-hairline p-2"
      :style="{ top: pos.y + 'px', left: pos.x + 'px' }"
      @mouseenter="clearTimer"
      @mouseleave="onLeave"
    >
      <img
        :src="gifUrl"
        :width="baseWidth"
        :height="baseHeight"
        class="rounded-[8px] block cursor-pointer"
        alt=""
        @click.stop="enlarged = true"
      />
      <p class="text-caption font-mono text-center text-ink-muted mt-1">
        {{ symbol }} {{ name }}
      </p>
    </div>

    <!-- Fullscreen overlay -->
    <div
      v-if="enlarged"
      class="fixed inset-0 z-[70] flex items-center justify-center"
      @click="enlarged = false"
    >
      <div
        class="bg-bg-surface rounded-card border border-hairline p-4"
        @click.stop
        @wheel.prevent="onWheel"
      >
        <img
          :src="gifUrl"
          :style="{ width: baseWidth * scale + 'px', height: baseHeight * scale + 'px' }"
          class="rounded-[8px] block"
          alt=""
        />
        <p class="text-caption font-mono text-center text-ink-muted mt-2">
          {{ symbol }} {{ name }}  ·  {{ scale.toFixed(2) }}x  滚轮缩放
        </p>
      </div>
    </div>
  </Teleport>
</template>

<script setup lang="ts">
import { ref, computed, watch, onUnmounted } from 'vue'

const props = defineProps<{
  symbol: string
  name: string
}>()

function normalizeSymbol(raw: string): string {
  const dot = raw.indexOf('.')
  if (dot !== -1) {
    const code = raw.substring(0, dot)
    let suffix = raw.substring(dot + 1).toLowerCase()
    if (suffix === 'ss') suffix = 'sh'
    return suffix + code
  }
  return (raw.startsWith('6') ? 'sh' : 'sz') + raw
}

const gifUrl = computed(() =>
  `https://image.sinajs.cn/newchart/min/n/${normalizeSymbol(props.symbol)}.gif`
)

const visible = ref(false)
const enlarged = ref(false)
const scale = ref(1)
const pos = ref({ x: 0, y: 0 })
const trigger = ref<HTMLElement>()
const baseWidth = 240
const baseHeight = 180
let timer: ReturnType<typeof setTimeout> | null = null

function calcDefaultScale(): number {
  // 1/3 of screen for both dimensions, pick the smaller
  const w = (window.innerWidth / 3) / baseWidth
  const h = (window.innerHeight / 3) / baseHeight
  return Math.min(w, h)
}

function onEnter() {
  clearTimer()
  updatePosition()
  visible.value = true
}

function onLeave() {
  timer = setTimeout(() => {
    visible.value = false
  }, 200)
}

function clearTimer() {
  if (timer) { clearTimeout(timer); timer = null }
}

function onKeydown(e: KeyboardEvent) {
  if (e.key === 'Escape') enlarged.value = false
}

function onWheel(e: WheelEvent) {
  const step = 0.5
  scale.value += e.deltaY < 0 ? step : -step
  scale.value = Math.max(0.5, Math.min(8, scale.value))
}

watch(enlarged, (val) => {
  if (val) {
    scale.value = calcDefaultScale()
    document.addEventListener('keydown', onKeydown)
  } else {
    document.removeEventListener('keydown', onKeydown)
  }
})

onUnmounted(() => document.removeEventListener('keydown', onKeydown))

function updatePosition() {
  if (!trigger.value) return
  const rect = trigger.value.getBoundingClientRect()
  const cardW = 256  // 240 + padding
  const cardH = 228  // 180 + text + padding
  let x = rect.right + 12
  let y = rect.top - 24

  // Flip left if would overflow right edge
  if (x + cardW > window.innerWidth) {
    x = rect.left - cardW - 12
  }
  // Clamp to viewport top/bottom
  if (y + cardH > window.innerHeight) {
    y = window.innerHeight - cardH - 8
  }
  if (y < 8) y = 8

  pos.value = { x, y }
}
</script>
