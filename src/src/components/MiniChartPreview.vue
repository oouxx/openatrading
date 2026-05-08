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
      class="fixed z-[60] bg-canvas rounded-card border border-hairline p-2"
      :style="{ top: pos.y + 'px', left: pos.x + 'px' }"
      @mouseenter="clearTimer"
      @mouseleave="onLeave"
    >
      <img
        :src="gifUrl"
        width="240"
        height="180"
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
        class="bg-canvas rounded-card border border-hairline p-4"
        @click.stop
        @wheel.prevent="onWheel"
      >
        <img
          :src="gifUrl"
          :style="{ width: 240 * scale + 'px', height: 180 * scale + 'px' }"
          class="rounded-[8px] block"
          alt=""
        />
        <p class="text-caption font-mono text-center text-ink-muted mt-2">
          {{ symbol }} {{ name }}  ·  {{ scale }}x  滚轮缩放
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
let timer: ReturnType<typeof setTimeout> | null = null

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
  scale.value += e.deltaY < 0 ? 0.25 : -0.25
  scale.value = Math.max(1, Math.min(4, scale.value))
}

watch(enlarged, (val) => {
  if (val) {
    scale.value = 1
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
