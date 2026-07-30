import { useState, useEffect, useCallback, useRef, useMemo } from "react"
import { useReplayEvents } from "@/hooks/query/useReplayEvents"

// ── Constants ──────────────────────────────────────────

const AM = [9 * 60 + 30, 11 * 60 + 30]
const PM = [13 * 60, 15 * 60]
const TRADING_MINUTES: number[] = []
for (let m = AM[0]; m <= AM[1]; m++) TRADING_MINUTES.push(m)
for (let m = PM[0]; m <= PM[1]; m++) TRADING_MINUTES.push(m)

function minToTime(m: number) {
  return `${String(Math.floor(m / 60)).padStart(2, "0")}:${String(m % 60).padStart(2, "0")}`
}

// ── Types ──────────────────────────────────────────────

interface RelatedStock {
  symbol: string
  name: string
  mtm: number
  pcp: number
}

interface ReplayEvent {
  id: number
  event_type: number
  event_timestamp: number
  good_or_bad: number
  plate_name?: string
  pcp?: number
  related_stocks?: RelatedStock[]
}

interface ReplayPage {
  date: string
  market_phase: string
  events: ReplayEvent[]
  next_cursor: number | null
}

interface DisplayEvent {
  id: number
  event_type: number
  minute: number
  plateName: string
  pcp: number
  goodOrBad: 1 | 2
  relatedStocks: RelatedStock[]
}

// ── Main Component ────────────────────────────────────

export default function ReplayView() {
  const [minPct, setMinPct] = useState(2)
  const { data: rawPage, isLoading, error } = useReplayEvents(minPct)
  const [currentMinute, setCurrentMinute] = useState(AM[0])
  const [filter, setFilter] = useState<"all" | "good" | "bad">("all")
  const [eventTypeFilter, setEventTypeFilter] = useState<number | null>(null)
  const [selectedId, setSelectedId] = useState<number | null>(null)
  const [tooltip, setTooltip] = useState<{ x: number; y: number; html: string } | null>(null)
  const [hoveredId, setHoveredId] = useState<number | null>(null)
  const chartRef = useRef<HTMLDivElement>(null)

  // Derive events from rawPage
  const allEvents = useMemo<DisplayEvent[]>(() => {
    if (!rawPage) return []
    const page = rawPage as ReplayPage
    const evs = page.events ?? []
    if (evs.length === 0) return []
    // Compute midnight Beijing from the events' timestamps themselves,
    // avoiding date mismatch when the trading day differs from `page.date`.
    const firstTs = evs[0].event_timestamp
    const beijingOffset = 8 * 3600
    const base = firstTs - ((firstTs + beijingOffset) % 86400)
    return evs
      .filter((e: ReplayEvent) => e.plate_name)
      .map((e: ReplayEvent) => ({
        id: e.id,
        event_type: e.event_type,
        minute: Math.round((e.event_timestamp - base) / 60),
        plateName: e.plate_name!,
        pcp: e.pcp || 0,
        goodOrBad: (e.good_or_bad === 1 ? 1 : 2) as 1 | 2,
        relatedStocks: e.related_stocks || [],
      }))
      .sort((a, b) => a.minute - b.minute)
  }, [rawPage])

  const filtered = useMemo(
    () =>
      allEvents.filter((e) => {
        if (Math.abs(e.pcp) * 100 < minPct) return false
        if (eventTypeFilter != null && e.event_type !== eventTypeFilter) return false
        if (filter === "all") return true
        return (filter === "good" && e.goodOrBad === 1) || (filter === "bad" && e.goodOrBad === 2)
      }),
    [allEvents, minPct, filter, eventTypeFilter]
  )

  const eventTypes = useMemo(() => {
    const types = new Set(allEvents.map((e) => e.event_type))
    return Array.from(types).sort()
  }, [allEvents])

  const step = (d: number) => {
    setCurrentMinute((prev) => {
      const idx = TRADING_MINUTES.indexOf(prev) + d
      return idx >= 0 && idx < TRADING_MINUTES.length ? TRADING_MINUTES[idx] : prev
    })
  }
  const jumpToEvent = (id: number) => {
    const ev = allEvents.find((e) => e.id === id)
    if (ev) {
      setCurrentMinute(ev.minute)
      setSelectedId(id)
    }
  }

  // Keyboard
  useEffect(() => {
    const onKey = (e: KeyboardEvent) => {
      if (e.key === "ArrowRight") {
        e.preventDefault()
        step(1)
      }
      if (e.key === "ArrowLeft") {
        e.preventDefault()
        step(-1)
      }
      if (e.key === "Escape") {
        setSelectedId(null)
      }
    }
    document.addEventListener("keydown", onKey)
    return () => document.removeEventListener("keydown", onKey)
  }, [])

  if (isLoading) {
    return (
      <div
        className="bg-bg flex min-h-screen items-center justify-center pt-11"
        style={{ height: "calc(100vh - 44px)" }}
      >
        <div className="text-ink-muted flex items-center gap-3 text-xs">
          <span className="bg-ai size-2 animate-pulse rounded-full" />
          加载回放数据...
        </div>
      </div>
    )
  }
  if (error)
    return (
      <div
        className="bg-bg flex min-h-screen items-center justify-center pt-11"
        style={{ height: "calc(100vh - 44px)" }}
      >
        <p className="text-ink-muted text-xs">{error.message}</p>
      </div>
    )

  const nowTs = currentMinute

  return (
    <div className="flex h-full flex-col">
      {/* Control Bar */}
      <div className="bg-bg-surface border-hairline sticky top-0 z-15 flex shrink-0 flex-wrap items-center gap-2.5 border-b px-5 py-2">
        {/* 方向 */}
        {[
          { f: "all" as const, label: "全部" },
          { f: "good" as const, label: "利好 ↑" },
          { f: "bad" as const, label: "利空 ↓" },
        ].map(({ f, label }) => (
          <button
            key={f}
            onClick={() => {
              setFilter(f)
              setSelectedId(null)
            }}
            className={`cursor-pointer rounded-2xl border px-3.5 py-1 text-[11px] font-medium transition-colors ${
              filter === f
                ? f === "good"
                  ? "border-up bg-up text-white"
                  : f === "bad"
                    ? "border-down bg-down text-white"
                    : "border-ai bg-ai text-white"
                : "border-hairline text-ink-muted hover:text-white"
            }`}
          >
            {label}
          </button>
        ))}
        <span className="bg-hairline mx-1 h-5 w-px" />
        {/* 类型 */}
        {eventTypes.map((t) => {
          const label = t === 11000 ? "板块异动" : t === 11001 ? "个股异动" : String(t)
          return (
            <button
              key={t}
              onClick={() => setEventTypeFilter(eventTypeFilter === t ? null : t)}
              className={`cursor-pointer rounded-2xl border px-3 py-1 text-[11px] font-medium transition-colors ${
                eventTypeFilter === t
                  ? "border-ai bg-ai text-white"
                  : "border-hairline text-ink-muted hover:text-white"
              }`}
            >
              {label}
            </button>
          )
        })}
        <span className="bg-hairline mx-1 h-5 w-px" />
        {/* 振幅 */}
        <span className="text-ink-muted text-[10px]">振幅≥</span>
        <div className="flex gap-0.5">
          {[0, 1, 2, 3, 5].map((p) => (
            <button
              key={p}
              onClick={() => setMinPct(p)}
              className={`cursor-pointer rounded px-2 py-0.5 font-mono text-[10px] transition-colors ${minPct === p ? "bg-ai text-white" : "border-hairline text-ink-muted border hover:text-white"}`}
            >
              {p}%
            </button>
          ))}
        </div>
        <span className="bg-hairline mx-1 h-5 w-px" />
        <span className="min-w-[46px] text-center font-mono text-sm font-semibold text-white">
          {minToTime(currentMinute)}
        </span>
        <span className="bg-hairline mx-1 h-5 w-px" />
        <span className="text-ink-muted ml-auto text-[10px]">{filtered.length} 个事件</span>
      </div>

      {/* Chart Area */}
      <div
        ref={chartRef}
        className="bg-bg-surface border-hairline relative shrink-0 cursor-crosshair border-b"
        style={{ height: 260 }}
      >
        <PulseChart
          events={filtered}
          curMin={nowTs}
          hoveredId={hoveredId}
          onHover={(id, x, y) => {
            setHoveredId(id)
            if (id) {
              const ev = allEvents.find((e) => e.id === id)
              if (ev) {
                const isGood = ev.goodOrBad === 1
                setTooltip({
                  x,
                  y,
                  html: `<div style="font-weight:600">${ev.plateName} ${isGood ? "利好" : "利空"}</div><div style="font-family:var(--font-mono)">${minToTime(ev.minute)} | ${ev.pcp >= 0 ? "+" : "-"}${(Math.abs(ev.pcp) * 100).toFixed(2)}% | ${ev.relatedStocks.length}只</div>`,
                })
              }
            } else {
              setTooltip(null)
            }
          }}
          onClick={(id) => {
            if (id) {
              setSelectedId(id)
              jumpToEvent(id)
            } else {
              setSelectedId(null)
            }
          }}
        />
        {tooltip && (
          <div
            className="bg-bg-surface border-hairline pointer-events-none absolute z-50 rounded-lg border px-3 py-2 text-[11px] whitespace-nowrap text-white shadow-lg"
            style={{ left: Math.min(tooltip.x + 16, window.innerWidth - 200), top: tooltip.y - 60 }}
          >
            <span dangerouslySetInnerHTML={{ __html: tooltip.html }} />
          </div>
        )}
      </div>

      {/* Card List */}
      <div className="flex-1 overflow-y-auto px-5 py-2.5">
        {filtered.map((e) => {
          const past = e.minute <= nowTs
          const now = e.minute === nowTs
          const isGood = e.goodOrBad === 1
          return (
            <div
              key={e.id}
              className={`bg-bg-surface mb-1 flex cursor-pointer items-center gap-2.5 rounded-lg border px-3 py-2 transition-all ${
                past ? "opacity-100" : "opacity-40"
              } ${now || selectedId === e.id ? "!border-ai !opacity-100 shadow-[0_0_0_2px_var(--color-accent)]" : "border-hairline"}`}
              onClick={() => {
                setSelectedId(e.id)
                jumpToEvent(e.id)
              }}
            >
              <span className={`size-2 shrink-0 rounded-full ${isGood ? "bg-up" : "bg-down"}`} />
              <span className="text-ink-muted min-w-[42px] font-mono text-[11px]">
                {minToTime(e.minute)}
              </span>
              <span className="w-[72px] truncate text-xs font-semibold text-white">
                {e.plateName}
              </span>
              <span
                className={`shrink-0 rounded px-1.5 py-0.5 text-[10px] font-semibold ${isGood ? "bg-up/10 text-up" : "bg-down/10 text-down"}`}
              >
                {isGood ? "利好" : "利空"}
              </span>
              <span
                className={`w-[64px] font-mono text-[11px] font-medium ${isGood ? "text-up" : "text-down"}`}
              >
                {e.pcp >= 0 ? "+" : "-"}
                {(Math.abs(e.pcp) * 100).toFixed(2)}%
              </span>
              <span className="text-ink-muted flex-1 truncate text-right text-[10px]">
                {e.relatedStocks.map((s) => (
                  <span key={s.symbol}>
                    {s.name}
                    <span className={`ml-0.5 font-mono ${s.pcp >= 0 ? "text-up" : "text-down"}`}>
                      {s.pcp >= 0 ? "+" : "-"}
                      {(Math.abs(s.pcp) * 100).toFixed(2)}%
                    </span>{" "}
                  </span>
                ))}
              </span>
            </div>
          )
        })}
      </div>
    </div>
  )
}

// ── Canvas Pulse Chart ──────────────────────────────────

function PulseChart({
  events,
  curMin,
  hoveredId,
  onHover,
  onClick,
}: {
  events: DisplayEvent[]
  curMin: number
  hoveredId: number | null
  onHover: (id: number | null, x: number, y: number) => void
  onClick: (id: number | null) => void
}) {
  const canvasRef = useRef<HTMLCanvasElement>(null)
  const containerRef = useRef<HTMLDivElement>(null)
  const hitMap = useRef<
    Map<string, { id: number; x: number; y1: number; y2: number; isGood: boolean }>
  >(new Map())
  const resizeRef = useRef<number>(0)

  useEffect(() => {
    const canvas = canvasRef.current
    const container = containerRef.current
    if (!canvas || !container) return

    const dpr = window.devicePixelRatio || 1
    const rect = container.getBoundingClientRect()
    const W = rect.width,
      H = rect.height
    canvas.width = W * dpr
    canvas.height = H * dpr
    canvas.style.width = W + "px"
    canvas.style.height = H + "px"

    const ctx = canvas.getContext("2d")!
    ctx.scale(dpr, dpr)

    const pad = { l: 50, r: 24, t: 30, b: 40 }
    const pw = W - pad.l - pad.r,
      ph = H - pad.t - pad.b
    const midY = pad.t + ph / 2
    const maxAbs = Math.max(0.005, ...events.map((e) => Math.abs(e.pcp)))
    const scale = (ph / 2 - 12) / maxAbs
    const xFromMin = (m: number) => pad.l + ((m - AM[0]) / (PM[1] - AM[0])) * pw

    hitMap.current.clear()

    // ── Background ──
    ctx.clearRect(0, 0, W, H)

    // Grid lines
    ctx.strokeStyle = "rgb(var(--color-fg-rgb) / 0.04)"
    ctx.lineWidth = 1
    for (let i = 0; i <= 4; i++) {
      const y = pad.t + (ph * i) / 4
      ctx.beginPath()
      ctx.setLineDash([3, 4])
      ctx.moveTo(pad.l, y)
      ctx.lineTo(W - pad.r, y)
      ctx.stroke()
      ctx.setLineDash([])
    }

    // Center axis (zero line)
    ctx.strokeStyle = "rgb(var(--color-fg-rgb) / 0.1)"
    ctx.lineWidth = 1.5
    ctx.beginPath()
    ctx.moveTo(pad.l, midY)
    ctx.lineTo(W - pad.r, midY)
    ctx.stroke()

    // ── Bars ──
    const drawBar = (e: DisplayEvent, isGlow: boolean) => {
      const cx = xFromMin(e.minute)
      const h = Math.abs(e.pcp) * scale
      const isGood = e.goodOrBad === 1
      const y1 = isGood ? midY - h : midY
      const y2 = isGood ? midY : midY + h
      const past = e.minute <= curMin
      const col = isGood ? upColor : downColor

      ctx.globalAlpha = past ? 1 : 0.2

      if (isGlow) {
        ctx.save()
        ctx.shadowColor = col
        ctx.shadowBlur = 12
      }

      ctx.strokeStyle = col
      ctx.lineWidth = isGlow ? 4 : 2.5
      ctx.lineCap = "round"
      ctx.beginPath()
      ctx.moveTo(cx, y1)
      ctx.lineTo(cx, y2)
      ctx.stroke()

      if (isGlow) ctx.restore()

      // Circle at tip for past
      if (past) {
        ctx.fillStyle = col
        ctx.beginPath()
        ctx.arc(cx, isGood ? y1 : y2, isGlow ? 5 : 3, 0, Math.PI * 2)
        ctx.fill()
        ctx.strokeStyle = "#fff"
        ctx.lineWidth = 1.5
        ctx.setLineDash([])
        ctx.stroke()
      }

      ctx.globalAlpha = 1

      // Hit area: wider than visual for easier clicking
      hitMap.current.set(String(e.id), { id: e.id, x: cx, y1, y2, isGood })
    }

    const upColor =
      getComputedStyle(document.documentElement).getPropertyValue("--color-up").trim() || "#FF5C5C"
    const downColor =
      getComputedStyle(document.documentElement).getPropertyValue("--color-down").trim() ||
      "#1D9E75"

    // Draw all bars first (non-glow)
    events.forEach((e) => {
      const isNow = e.minute === curMin || e.id === hoveredId
      if (!isNow) drawBar(e, false)
    })

    // Draw glow bars on top
    events.forEach((e) => {
      const isNow = e.minute === curMin || e.id === hoveredId
      if (isNow) drawBar(e, true)
    })

    // ── Current time cursor ──
    const curX = xFromMin(curMin)
    ctx.strokeStyle =
      getComputedStyle(document.documentElement).getPropertyValue("--color-accent").trim() ||
      "#7F77DD"
    ctx.lineWidth = 1.5
    ctx.setLineDash([6, 4])
    ctx.globalAlpha = 0.7
    ctx.beginPath()
    ctx.moveTo(curX, pad.t)
    ctx.lineTo(curX, H - pad.b)
    ctx.stroke()
    ctx.setLineDash([])
    ctx.globalAlpha = 1

    // Cursor circle
    ctx.fillStyle = "#fff"
    ctx.beginPath()
    ctx.arc(curX, midY, 7, 0, Math.PI * 2)
    ctx.fill()
    ctx.strokeStyle =
      getComputedStyle(document.documentElement).getPropertyValue("--color-accent").trim() ||
      "#7F77DD"
    ctx.lineWidth = 2.5
    ctx.stroke()

    // ── Labels ──
    ctx.fillStyle = "rgb(var(--color-muted-rgb))"
    ctx.font = "9px Inter, system-ui, sans-serif"

    // X labels
    const xLabels = [
      "09:30",
      "10:00",
      "10:30",
      "11:00",
      "11:30",
      "13:00",
      "13:30",
      "14:00",
      "14:30",
      "15:00",
    ]
    xLabels.forEach((t) => {
      const [h, mn] = t.split(":").map(Number)
      const m = h * 60 + mn
      const x = xFromMin(m)
      ctx.textAlign = "center"
      ctx.fillText(t, x, H - 8)
    })

    // Y labels
    ctx.font = "8px Inter, system-ui, sans-serif"
    ctx.textAlign = "end"
    ctx.fillStyle = upColor
    ctx.fillText(`+${(maxAbs * 100).toFixed(1)}%`, pad.l - 8, pad.t + 12)
    ctx.fillStyle = downColor
    ctx.fillText(`-${(maxAbs * 100).toFixed(1)}%`, pad.l - 8, H - pad.b + 4)
  }, [events, curMin, hoveredId])

  // Resize handler
  useEffect(() => {
    const onResize = () => {
      resizeRef.current++
    }
    window.addEventListener("resize", onResize)
    return () => window.removeEventListener("resize", onResize)
  }, [])

  return (
    <div ref={containerRef} style={{ width: "100%", height: "100%", position: "relative" }}>
      <canvas
        ref={canvasRef}
        style={{ display: "block", width: "100%", height: "100%" }}
        onMouseMove={(e) => {
          const rect = canvasRef.current?.getBoundingClientRect()
          if (!rect) return
          const mx = e.clientX - rect.left
          const my = e.clientY - rect.top
          // Find nearest event
          let nearest: { id: number; dist: number } | null = null
          hitMap.current.forEach((v) => {
            if (
              mx >= v.x - 12 &&
              mx <= v.x + 12 &&
              my >= Math.min(v.y1, v.y2) - 4 &&
              my <= Math.max(v.y1, v.y2) + 4
            ) {
              const dist = Math.abs(mx - v.x)
              if (!nearest || dist < nearest.dist) nearest = { id: v.id, dist }
            }
          })
          onHover(nearest?.id || null, e.clientX, e.clientY)
        }}
        onMouseLeave={() => onHover(null, 0, 0)}
        onClick={(e) => {
          const rect = canvasRef.current?.getBoundingClientRect()
          if (!rect) return
          const mx = e.clientX - rect.left
          const my = e.clientY - rect.top
          let clicked: number | null = null
          let minDist = Infinity
          hitMap.current.forEach((v) => {
            if (
              mx >= v.x - 16 &&
              mx <= v.x + 16 &&
              my >= Math.min(v.y1, v.y2) - 6 &&
              my <= Math.max(v.y1, v.y2) + 6
            ) {
              const dist = Math.abs(mx - v.x)
              if (dist < minDist) {
                minDist = dist
                clicked = v.id
              }
            }
          })
          onClick(clicked)
        }}
      />
    </div>
  )
}
