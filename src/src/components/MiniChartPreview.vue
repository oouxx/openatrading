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
        class="rounded-[8px] block"
        alt=""
      />
      <p class="text-caption font-mono text-center text-ink-muted mt-1">
        {{ symbol }} {{ name }}
      </p>
    </div>
  </Teleport>
</template>

<script setup lang="ts">
import { ref, computed } from 'vue'

const props = defineProps<{
  symbol: string
  name: string
}>()

function normalizeSymbol(raw: string): string {
  const dot = raw.indexOf('.')
  if (dot !== -1) {
    const code = raw.substring(0, dot)
    const suffix = raw.substring(dot + 1).toLowerCase()
    return suffix + code
  }
  return (raw.startsWith('6') ? 'sh' : 'sz') + raw
}

const gifUrl = computed(() =>
  `https://image.sinajs.cn/newchart/min/n/${normalizeSymbol(props.symbol)}.gif`
)

const visible = ref(false)
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
