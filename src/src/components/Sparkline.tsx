import { useRef, useEffect } from "react"
import type { PnlPoint } from "@/stores/data"

interface SparklineProps {
  points: PnlPoint[]
  width?: number
  height?: number
}

export default function Sparkline({ points, width = 200, height = 32 }: SparklineProps) {
  const canvasRef = useRef<HTMLCanvasElement>(null)

  useEffect(() => {
    const canvas = canvasRef.current
    if (!canvas || !points.length) return
    const ctx = canvas.getContext("2d")
    if (!ctx) return

    const w = width
    const h = height
    canvas.width = w * devicePixelRatio
    canvas.height = h * devicePixelRatio
    canvas.style.width = w + "px"
    canvas.style.height = h + "px"
    ctx.scale(devicePixelRatio, devicePixelRatio)
    ctx.clearRect(0, 0, w, h)

    const values = points.map((p) => p.return_pct)
    const min = Math.min(...values, 0)
    const max = Math.max(...values, 0)
    const range = max - min || 1

    const padX = 2
    const padY = 2
    const plotW = w - padX * 2
    const plotH = h - padY * 2

    const getX = (i: number) => padX + (i / Math.max(values.length - 1, 1)) * plotW
    const getY = (v: number) => padY + plotH - ((v - min) / range) * plotH

    ctx.beginPath()
    ctx.strokeStyle = values[values.length - 1] >= 0 ? "#FF5C5C" : "#1D9E75"
    ctx.lineWidth = 1.5
    ctx.lineJoin = "round"

    ctx.moveTo(getX(0), getY(values[0]))
    for (let i = 1; i < values.length; i++) {
      const x = getX(i)
      const y = getY(values[i])
      const prevX = getX(i - 1)
      const prevY = getY(values[i - 1])
      const cx = (prevX + x) / 2
      ctx.quadraticCurveTo(prevX, prevY, cx, (prevY + y) / 2)
      ctx.lineTo(x, y)
    }
    ctx.stroke()
  }, [points, width, height])

  return <canvas ref={canvasRef} width={width} height={height} className="inline-block" />
}
