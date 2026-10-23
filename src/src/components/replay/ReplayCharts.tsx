import { useEffect, useMemo, useRef, useState } from "react"
import { chartColors, withAlpha } from "@/lib/chartColors"
import {
  BUCKET_COUNT,
  TRADING_MINUTES,
  clamp,
  formatPct,
  formatTradingMinute,
  type IndexPoint,
  type RotationData,
} from "./replayAnalysis"

const AXIS_MINUTES = [0, 60, 120, 180, TRADING_MINUTES]

export interface IndexSeries {
  key: string
  label: string
  color: string
  points: IndexPoint[]
}

function useContainerWidth<T extends HTMLElement>() {
  const ref = useRef<T>(null)
  const [width, setWidth] = useState(0)

  useEffect(() => {
    const element = ref.current
    if (!element) return
    const update = () => setWidth(element.clientWidth)
    update()
    const observer = new ResizeObserver(update)
    observer.observe(element)
    return () => observer.disconnect()
  }, [])

  return [ref, width] as const
}

function pointAt(points: IndexPoint[], minute: number): IndexPoint | null {
  let result: IndexPoint | null = null
  for (const point of points) {
    if (point.minute > minute) break
    result = point
  }
  return result
}

export function IndexMiniChart({
  series,
  cursor,
  height = 112,
}: {
  series: IndexSeries[]
  cursor: number
  height?: number
}) {
  const [ref, width] = useContainerWidth<HTMLDivElement>()
  const canvasRef = useRef<HTMLCanvasElement>(null)

  const activePoints = useMemo(
    () =>
      series.map((item) => ({
        ...item,
        active: pointAt(item.points, cursor),
      })),
    [series, cursor]
  )

  useEffect(() => {
    const canvas = canvasRef.current
    if (!canvas || width <= 0) return
    const dpr = window.devicePixelRatio || 1
    canvas.width = Math.round(width * dpr)
    canvas.height = Math.round(height * dpr)
    canvas.style.width = `${width}px`
    canvas.style.height = `${height}px`
    const ctx = canvas.getContext("2d")
    if (!ctx) return
    ctx.setTransform(dpr, 0, 0, dpr, 0, 0)
    ctx.clearRect(0, 0, width, height)

    const pad = { l: 6, r: 6, t: 12, b: 16 }
    const pw = width - pad.l - pad.r
    const ph = height - pad.t - pad.b
    const values = series.flatMap((item) => item.points.map((point) => point.pct))
    if (values.length === 0) return
    let min = Math.min(0, ...values)
    let max = Math.max(0, ...values)
    const span = Math.max(0.2, max - min)
    min -= span * 0.08
    max += span * 0.08
    const y = (pct: number) => pad.t + ((max - pct) / (max - min)) * ph
    const x = (minute: number) => pad.l + (minute / TRADING_MINUTES) * pw

    // 零轴 + 网格
    ctx.strokeStyle = withAlpha(chartColors.tertiary(), 0.2)
    ctx.lineWidth = 1
    ctx.setLineDash([3, 4])
    for (const pct of [0, max, min]) {
      const py = y(pct)
      ctx.beginPath()
      ctx.moveTo(pad.l, py)
      ctx.lineTo(width - pad.r, py)
      ctx.stroke()
    }
    ctx.setLineDash([])

    for (const item of series) {
      if (item.points.length === 0) continue
      ctx.strokeStyle = item.color
      ctx.lineWidth = 1.5
      ctx.beginPath()
      item.points.forEach((point, index) => {
        const px = x(point.minute)
        const py = y(point.pct)
        if (index === 0) ctx.moveTo(px, py)
        else ctx.lineTo(px, py)
      })
      ctx.stroke()
    }

    // 当前回放时间
    const cursorX = x(cursor)
    ctx.strokeStyle = withAlpha(chartColors.brand(), 0.75)
    ctx.lineWidth = 1
    ctx.setLineDash([5, 4])
    ctx.beginPath()
    ctx.moveTo(cursorX, pad.t)
    ctx.lineTo(cursorX, height - pad.b)
    ctx.stroke()
    ctx.setLineDash([])

    // 时间轴
    ctx.fillStyle = chartColors.tertiary()
    ctx.font = "9px 'Geist Variable', system-ui, sans-serif"
    for (const minute of AXIS_MINUTES) {
      ctx.textAlign = minute === 0 ? "left" : minute === TRADING_MINUTES ? "right" : "center"
      ctx.fillText(formatTradingMinute(minute), x(minute), height - 4)
    }
  }, [series, cursor, width, height])

  return (
    <div ref={ref} className="relative">
      <div className="mb-1 flex flex-wrap items-center gap-x-3 gap-y-1">
        {activePoints.map((item) => (
          <span key={item.key} className="text-caption text-secondary flex items-center gap-1">
            <span
              className="inline-block size-1.5 rounded-full"
              style={{ backgroundColor: item.color }}
            />
            {item.label}
            <span className="font-number text-primary ml-0.5">
              {item.active ? formatPct(item.active.pct / 100) : "—"}
            </span>
          </span>
        ))}
        <span className="text-caption text-tertiary ml-auto">相对开盘</span>
      </div>
      <canvas ref={canvasRef} aria-hidden="true" className="block" />
    </div>
  )
}

const LABEL_W = 70
const HEADER_H = 14
const ROW_H = 17
const FOOTER_H = 16

export function RotationHeatmap({
  rotation,
  cursor,
  selectedId,
  onSelect,
}: {
  rotation: RotationData
  cursor: number
  selectedId: number | null
  onSelect: (eventId: number, tradingMinute: number) => void
}) {
  const [scrollRef, width] = useContainerWidth<HTMLDivElement>()
  const canvasRef = useRef<HTMLCanvasElement>(null)
  const [hover, setHover] = useState<{
    row: number
    col: number
    x: number
    y: number
  } | null>(null)

  const height = HEADER_H + rotation.rows.length * ROW_H + FOOTER_H

  useEffect(() => {
    const canvas = canvasRef.current
    if (!canvas || width <= 0 || rotation.rows.length === 0) return
    const dpr = window.devicePixelRatio || 1
    canvas.width = Math.round(width * dpr)
    canvas.height = Math.round(height * dpr)
    canvas.style.width = `${width}px`
    canvas.style.height = `${height}px`
    const ctx = canvas.getContext("2d")
    if (!ctx) return
    ctx.setTransform(dpr, 0, 0, dpr, 0, 0)
    ctx.clearRect(0, 0, width, height)

    const colWidth = Math.max(2, (width - LABEL_W - 4) / BUCKET_COUNT)
    const gridWidth = width - LABEL_W - 4
    const xForMinute = (minute: number) => LABEL_W + (minute / TRADING_MINUTES) * gridWidth
    const up = chartColors.up()
    const down = chartColors.down()

    ctx.font = "9px 'Geist Variable', system-ui, sans-serif"
    ctx.textBaseline = "middle"

    // 每 30 分钟一条竖向参考线
    ctx.strokeStyle = withAlpha(chartColors.tertiary(), 0.12)
    ctx.lineWidth = 1
    for (const minute of [0, 30, 60, 90, 120, 150, 180, 210, 240]) {
      const x = xForMinute(minute)
      ctx.beginPath()
      ctx.moveTo(x, HEADER_H)
      ctx.lineTo(x, height - FOOTER_H)
      ctx.stroke()
    }

    rotation.rows.forEach((row, rowIndex) => {
      const top = HEADER_H + rowIndex * ROW_H
      ctx.fillStyle = rowIndex % 2 === 0 ? withAlpha(chartColors.tertiary(), 0.02) : "transparent"
      if (rowIndex % 2 === 0) {
        ctx.fillRect(LABEL_W, top, width - LABEL_W - 4, ROW_H)
      }

      ctx.fillStyle = chartColors.tertiary()
      ctx.textAlign = "left"
      ctx.fillText(row.name.slice(0, 6), 2, top + ROW_H / 2)

      row.cells.forEach((cell, bucket) => {
        if (!cell) return
        const abs = Math.abs(cell.pcp)
        const alpha = clamp(0.15 + (abs / rotation.maxAbs) * 0.8, 0.15, 0.95)
        const color = cell.direction === 1 ? up : down
        const left = LABEL_W + bucket * colWidth
        ctx.fillStyle = withAlpha(color, alpha)
        ctx.fillRect(left, top + 1, Math.max(1, colWidth - 1), ROW_H - 2)

        if (cell.eventId === selectedId) {
          ctx.strokeStyle = chartColors.primary()
          ctx.lineWidth = 1
          ctx.strokeRect(left + 0.5, top + 1.5, Math.max(1, colWidth - 2), ROW_H - 3)
        }
      })
    })

    // 当前时间游标
    const cursorX = xForMinute(cursor)
    ctx.strokeStyle = withAlpha(chartColors.brand(), 0.9)
    ctx.lineWidth = 1
    ctx.setLineDash([5, 4])
    ctx.beginPath()
    ctx.moveTo(cursorX, HEADER_H)
    ctx.lineTo(cursorX, height - FOOTER_H)
    ctx.stroke()
    ctx.setLineDash([])

    // 时间轴
    ctx.fillStyle = chartColors.tertiary()
    ctx.textBaseline = "alphabetic"
    for (const minute of AXIS_MINUTES) {
      ctx.textAlign = minute === 0 ? "left" : minute === TRADING_MINUTES ? "right" : "center"
      ctx.fillText(formatTradingMinute(minute), xForMinute(minute), height - 4)
    }
  }, [rotation, cursor, selectedId, width, height])

  if (rotation.rows.length === 0) {
    return (
      <div className="text-secondary text-caption flex h-24 items-center justify-center">
        当前筛选条件下没有板块异动
      </div>
    )
  }

  const hoverCell = hover ? rotation.rows[hover.row]?.cells[hover.col] : null
  const hoverRow = hover ? rotation.rows[hover.row] : null

  return (
    <div
      ref={scrollRef}
      className="relative max-h-72 overflow-y-auto"
      onMouseLeave={() => setHover(null)}
    >
      <canvas
        ref={canvasRef}
        aria-label="板块异动轮动热力图"
        className="block cursor-crosshair"
        onMouseMove={(event) => {
          const rect = event.currentTarget.getBoundingClientRect()
          const x = event.clientX - rect.left
          const y = event.clientY - rect.top
          const colWidth = Math.max(2, (rect.width - LABEL_W - 4) / BUCKET_COUNT)
          const col = Math.floor((x - LABEL_W) / colWidth)
          const row = Math.floor((y - HEADER_H) / ROW_H)
          if (
            x >= LABEL_W &&
            row >= 0 &&
            row < rotation.rows.length &&
            col >= 0 &&
            col < BUCKET_COUNT
          ) {
            setHover({ row, col, x, y })
          } else {
            setHover(null)
          }
        }}
        onClick={(event) => {
          const rect = event.currentTarget.getBoundingClientRect()
          const x = event.clientX - rect.left
          const y = event.clientY - rect.top
          const colWidth = Math.max(2, (rect.width - LABEL_W - 4) / BUCKET_COUNT)
          const col = Math.floor((x - LABEL_W) / colWidth)
          const row = Math.floor((y - HEADER_H) / ROW_H)
          const cell = rotation.rows[row]?.cells[col]
          if (cell) onSelect(cell.eventId, cell.minute)
        }}
      />
      {hover && hoverCell && hoverRow && (
        <div
          className="bg-surface border-hairline text-caption shadow-modal pointer-events-none absolute z-20 rounded-md border px-2 py-1 whitespace-nowrap"
          style={{
            left: clamp(hover.x + 10, 0, Math.max(0, width - 150)),
            top: hover.y + 10,
          }}
        >
          <div className="text-primary font-medium">{hoverRow.name}</div>
          <div className="font-number text-secondary">
            {formatTradingMinute(hoverCell.minute)} · {formatPct(hoverCell.pcp)} · {hoverCell.count}{" "}
            次
          </div>
        </div>
      )}
    </div>
  )
}
