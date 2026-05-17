<template>
  <canvas ref="canvasRef" :width="width" :height="height" class="inline-block"></canvas>
</template>

<script setup lang="ts">
import { ref, onMounted, watch } from 'vue'
import type { PnlPoint } from '@/stores/data'

const props = defineProps<{
  points: PnlPoint[]
  width?: number
  height?: number
}>()

const canvasRef = ref<HTMLCanvasElement>()
const w = props.width ?? 200
const h = props.height ?? 32

function draw() {
  const canvas = canvasRef.value
  if (!canvas || !props.points.length) return
  const ctx = canvas.getContext('2d')
  if (!ctx) return

  canvas.width = w * devicePixelRatio
  canvas.height = h * devicePixelRatio
  canvas.style.width = w + 'px'
  canvas.style.height = h + 'px'
  ctx.scale(devicePixelRatio, devicePixelRatio)

  ctx.clearRect(0, 0, w, h)

  const values = props.points.map(p => p.return_pct)
  const min = Math.min(...values, 0)
  const max = Math.max(...values, 0)
  const range = max - min || 1

  const padX = 2
  const padY = 2
  const plotW = w - padX * 2
  const plotH = h - padY * 2

  const getX = (i: number) => padX + (i / Math.max(values.length - 1, 1)) * plotW
  const getY = (v: number) => padY + plotH - ((v - min) / range) * plotH

  // Draw line
  ctx.beginPath()
  ctx.strokeStyle = values[values.length - 1] >= 0 ? '#FF5C5C' : '#1D9E75'
  ctx.lineWidth = 1.5
  ctx.lineJoin = 'round'

  const firstX = getX(0)
  const firstY = getY(values[0])
  ctx.moveTo(firstX, firstY)

  for (let i = 1; i < values.length; i++) {
    const x = getX(i)
    const y = getY(values[i])
    // Simple quadratic bezier for smoothness
    const prevX = getX(i - 1)
    const prevY = getY(values[i - 1])
    const cx = (prevX + x) / 2
    ctx.quadraticCurveTo(prevX, prevY, cx, (prevY + y) / 2)
    ctx.lineTo(x, y)
  }
  ctx.stroke()
}

onMounted(draw)
watch(() => props.points, draw, { deep: true })
</script>
