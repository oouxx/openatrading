import { useState, useEffect, useCallback, useRef } from 'react'
import { apiFetch } from '@/stores/data'

// ── Constants ──────────────────────────────────────────

const AM = [9 * 60 + 30, 11 * 60 + 30]
const PM = [13 * 60, 15 * 60]
const TRADING_MINUTES: number[] = []
for (let m = AM[0]; m <= AM[1]; m++) TRADING_MINUTES.push(m)
for (let m = PM[0]; m <= PM[1]; m++) TRADING_MINUTES.push(m)

function minToTime(m: number) {
  return `${String(Math.floor(m / 60)).padStart(2, '0')}:${String(m % 60).padStart(2, '0')}`
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
  minute: number
  plateName: string
  pcp: number
  goodOrBad: 1 | 2
  relatedStocks: RelatedStock[]
}

// ── Main Component ────────────────────────────────────

export default function ReplayView() {
  const [loading, setLoading] = useState(true)
  const [error, setError] = useState('')
  const [allEvents, setAllEvents] = useState<DisplayEvent[]>([])
  const [currentMinute, setCurrentMinute] = useState(AM[0])
  const [isPlaying, setIsPlaying] = useState(false)
  const [playSpeed, setPlaySpeed] = useState(4)
  const [filter, setFilter] = useState<'all' | 'good' | 'bad'>('all')
  const [selectedId, setSelectedId] = useState<number | null>(null)
  const [tooltip, setTooltip] = useState<{ x: number; y: number; html: string } | null>(null)
  const [voiceOn, setVoiceOn] = useState(false)
  const [hoveredId, setHoveredId] = useState<number | null>(null)
  const timerRef = useRef<ReturnType<typeof setInterval> | null>(null)
  const chartRef = useRef<HTMLDivElement>(null)
  const lastAnnouncedTs = useRef<number | null>(null)

  useEffect(() => {
    async function fetch() {
      setLoading(true)
      setError('')
      try {
        const resp = await apiFetch('/api/replay/events')
        if (!resp.ok) throw new Error(`HTTP ${resp.status}`)
        const page: ReplayPage = await resp.json()
        const base = new Date(`${page.date}T00:00:00+08:00`).getTime() / 1000
        const events: DisplayEvent[] = page.events
          .filter(e => e.plate_name)
          .map(e => ({
            id: e.id,
            minute: Math.round((e.event_timestamp - base) / 60),
            plateName: e.plate_name!,
            pcp: e.pcp || 0,
            goodOrBad: (e.good_or_bad === 1 ? 1 : 2) as 1 | 2,
            relatedStocks: e.related_stocks || [],
          }))
          .sort((a, b) => a.minute - b.minute)
        setAllEvents(events)
      } catch (e: unknown) {
        setError((e as Error).message || '加载失败')
      } finally { setLoading(false) }
    }
    fetch()
  }, [])

  useEffect(() => () => { if (timerRef.current) clearInterval(timerRef.current) }, [])

  const stop = useCallback(() => {
    setIsPlaying(false)
    if (timerRef.current) { clearInterval(timerRef.current); timerRef.current = null }
  }, [])

  const start = useCallback(() => {
    setIsPlaying(true)
    timerRef.current = setInterval(() => {
      setCurrentMinute(prev => {
        const idx = TRADING_MINUTES.indexOf(prev) + playSpeed
        if (idx >= TRADING_MINUTES.length) stop()
        return idx >= TRADING_MINUTES.length ? prev : TRADING_MINUTES[idx]
      })
    }, 150)
  }, [playSpeed, stop])

  const toggle = () => { isPlaying ? stop() : start() }
  const step = (d: number) => {
    stop()
    setCurrentMinute(prev => {
      const idx = TRADING_MINUTES.indexOf(prev) + d
      return idx >= 0 && idx < TRADING_MINUTES.length ? TRADING_MINUTES[idx] : prev
    })
  }
  const setSpeed = (sp: number) => {
    setPlaySpeed(sp)
    if (isPlaying) { stop(); setTimeout(() => start(), 0) }
  }
  const jumpToEvent = (id: number) => {
    const ev = allEvents.find(e => e.id === id)
    if (ev) { stop(); setCurrentMinute(ev.minute); setSelectedId(id) }
  }

  // Voice
  useEffect(() => {
    if (!voiceOn || !window.speechSynthesis) return
    const nowEv = allEvents.find(e => e.minute === currentMinute)
    if (nowEv && nowEv.id !== lastAnnouncedTs.current) {
      lastAnnouncedTs.current = nowEv.id
      const isGood = nowEv.goodOrBad === 1
      const pct = (Math.abs(nowEv.pcp) * 100).toFixed(1)
      const text = `${minToTime(currentMinute)}，${nowEv.plateName}${isGood ? '利好' : '利空'}，${isGood ? '上涨' : '下跌'}百分之${pct}`
      const u = new SpeechSynthesisUtterance(text)
      u.lang = 'zh-CN'; u.rate = 1.1; u.pitch = 1
      window.speechSynthesis.cancel()
      window.speechSynthesis.speak(u)
    }
  }, [currentMinute, voiceOn])

  // Keyboard
  useEffect(() => {
    const onKey = (e: KeyboardEvent) => {
      if (e.key === 'ArrowRight') { e.preventDefault(); step(1) }
      if (e.key === 'ArrowLeft') { e.preventDefault(); step(-1) }
      if (e.key === ' ') { e.preventDefault(); toggle() }
      if (e.key === 'Escape') { setSelectedId(null) }
    }
    document.addEventListener('keydown', onKey)
    return () => document.removeEventListener('keydown', onKey)
  }, [isPlaying, playSpeed])

  if (loading) {
    return (
      <div className="min-h-screen bg-bg pt-11 flex items-center justify-center" style={{ height: 'calc(100vh - 44px)' }}>
        <div className="flex items-center gap-3 text-xs text-ink-muted">
          <span className="w-2 h-2 rounded-full bg-ai animate-pulse" />加载回放数据...
        </div>
      </div>
    )
  }
  if (error) return <div className="min-h-screen bg-bg pt-11 flex items-center justify-center" style={{ height: 'calc(100vh - 44px)' }}><p className="text-xs text-ink-muted">{error}</p></div>

  const filtered = allEvents.filter(e => filter === 'all' || (filter === 'good' && e.goodOrBad === 1) || (filter === 'bad' && e.goodOrBad === 2))
  const nowTs = currentMinute

  return (
    <div className="min-h-screen bg-bg pt-11 flex flex-col" style={{ height: 'calc(100vh - 44px)' }}>
      {/* Control Bar */}
      <div className="flex items-center gap-2.5 px-5 py-2 bg-bg-surface border-b border-hairline flex-shrink-0 flex-wrap sticky top-0 z-15">
        {[
          { f: 'all' as const, label: '全部' },
          { f: 'good' as const, label: '利好 ↑' },
          { f: 'bad' as const, label: '利空 ↓' },
        ].map(({ f, label }) => (
          <button
            key={f}
            onClick={() => { setFilter(f); setSelectedId(null) }}
            className={`px-3.5 py-1 rounded-2xl border text-[11px] cursor-pointer font-medium transition-colors ${
              filter === f
                ? f === 'good' ? 'bg-up border-up text-white'
                  : f === 'bad' ? 'bg-down border-down text-white'
                  : 'bg-ai border-ai text-white'
                : 'border-hairline text-ink-muted hover:text-white'
            }`}
          >{label}</button>
        ))}
        <span className="w-px h-5 bg-hairline mx-1" />
        <button onClick={() => step(-1)} className="w-[26px] h-[26px] rounded-md border border-hairline bg-transparent text-ink-muted cursor-pointer flex items-center justify-center text-[11px] hover:border-ai hover:text-white transition-colors">⏮</button>
        <button onClick={toggle} className="w-[34px] h-[34px] rounded-full bg-ai text-white cursor-pointer flex items-center justify-center text-sm hover:opacity-90 transition-opacity flex-shrink-0">{isPlaying ? '⏸' : '▶'}</button>
        <button onClick={() => step(1)} className="w-[26px] h-[26px] rounded-md border border-hairline bg-transparent text-ink-muted cursor-pointer flex items-center justify-center text-[11px] hover:border-ai hover:text-white transition-colors">⏭</button>
        <div className="flex gap-1">
          {[1, 4, 8, 16].map(sp => (
            <button key={sp} onClick={() => setSpeed(sp)}
              className={`px-2 py-0.5 rounded text-[10px] font-mono cursor-pointer transition-colors ${playSpeed === sp ? 'bg-ai text-white' : 'border border-hairline text-ink-muted hover:text-white'}`}>{sp}×</button>
          ))}
        </div>
        <span className="font-mono text-sm font-semibold text-white min-w-[46px] text-center">{minToTime(currentMinute)}</span>
        <span className="w-px h-5 bg-hairline mx-1" />
        <button onClick={() => { setVoiceOn(v => !v); if (voiceOn) { window.speechSynthesis?.cancel(); lastAnnouncedTs.current = null } }}
          className={`flex items-center gap-1 px-3 py-1 rounded-2xl border text-[11px] cursor-pointer transition-colors whitespace-nowrap ${voiceOn ? 'bg-[oklch(55%_0.14_240_/_0.12)] border-[oklch(55%_0.14_240)] text-[oklch(55%_0.14_240)]' : 'border-hairline text-ink-muted hover:text-white'}`}>
          <span className={`w-1.5 h-1.5 rounded-full ${voiceOn ? 'bg-[oklch(55%_0.14_240)] animate-pulse' : 'bg-ink-muted'}`} /> 语音播报
        </button>
        <span className="text-[10px] text-ink-muted ml-auto">{filtered.length} 个事件</span>
      </div>

      {/* Chart Area */}
      <div ref={chartRef} className="flex-shrink-0 bg-bg-surface border-b border-hairline relative cursor-crosshair" style={{ height: 260 }}>
        <PulseChart
          events={filtered}
          curMin={nowTs}
          hoveredId={hoveredId}
          onHover={(id, x, y) => {
            setHoveredId(id)
            if (id) {
              const ev = allEvents.find(e => e.id === id)
              if (ev) {
                const isGood = ev.goodOrBad === 1
                setTooltip({ x, y, html: `<div style="font-weight:600">${ev.plateName} ${isGood ? '利好' : '利空'}</div><div style="font-family:var(--font-mono)">${minToTime(ev.minute)} | ${ev.pcp >= 0 ? '+' : '-'}${(Math.abs(ev.pcp) * 100).toFixed(2)}% | ${ev.relatedStocks.length}只</div>` })
              }
            } else { setTooltip(null) }
          }}
          onClick={(id) => { if (id) { setSelectedId(id); jumpToEvent(id) } else { setSelectedId(null) } }}
        />
        {tooltip && (
          <div className="absolute pointer-events-none px-3 py-2 rounded-lg bg-bg-surface border border-hairline text-white text-[11px] z-50 whitespace-nowrap shadow-lg" style={{ left: Math.min(tooltip.x + 16, window.innerWidth - 200), top: tooltip.y - 60 }}>
            <span dangerouslySetInnerHTML={{ __html: tooltip.html }} />
          </div>
        )}
      </div>

      {/* Card List */}
      <div className="flex-1 overflow-y-auto px-5 py-2.5">
        {filtered.map(e => {
          const past = e.minute <= nowTs
          const now = e.minute === nowTs
          const isGood = e.goodOrBad === 1
          return (
            <div
              key={e.id}
              className={`flex items-center gap-2.5 px-3 py-2 rounded-lg mb-1 bg-bg-surface border cursor-pointer transition-all ${
                past ? 'opacity-100' : 'opacity-40'
              } ${now || selectedId === e.id ? '!border-ai shadow-[0_0_0_2px_var(--color-accent)] !opacity-100' : 'border-hairline'}`}
              onClick={() => { setSelectedId(e.id); jumpToEvent(e.id) }}
            >
              <span className={`w-2 h-2 rounded-full flex-shrink-0 ${isGood ? 'bg-up' : 'bg-down'}`} />
              <span className="font-mono text-[11px] text-ink-muted min-w-[42px]">{minToTime(e.minute)}</span>
              <span className="font-semibold text-xs text-white w-[72px] truncate">{e.plateName}</span>
              <span className={`text-[10px] font-semibold px-1.5 py-0.5 rounded flex-shrink-0 ${isGood ? 'bg-up/10 text-up' : 'bg-down/10 text-down'}`}>{isGood ? '利好' : '利空'}</span>
              <span className={`font-mono text-[11px] font-medium w-[64px] ${isGood ? 'text-up' : 'text-down'}`}>
                {(e.pcp >= 0 ? '+' : '-')}{(Math.abs(e.pcp) * 100).toFixed(2)}%
              </span>
              <span className="text-[10px] text-ink-muted flex-1 text-right truncate">
                {e.relatedStocks.map(s => (
                  <span key={s.symbol}>
                    {s.name}
                    <span className={`font-mono ml-0.5 ${s.pcp >= 0 ? 'text-up' : 'text-down'}`}>
                      {s.pcp >= 0 ? '+' : '-'}{(Math.abs(s.pcp) * 100).toFixed(2)}%
                    </span>
                    {' '}
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

function PulseChart({ events, curMin, hoveredId, onHover, onClick }: {
  events: DisplayEvent[]
  curMin: number
  hoveredId: number | null
  onHover: (id: number | null, x: number, y: number) => void
  onClick: (id: number | null) => void
}) {
  const canvasRef = useRef<HTMLCanvasElement>(null)
  const containerRef = useRef<HTMLDivElement>(null)
  const hitMap = useRef<Map<string, { id: number; x: number; y1: number; y2: number; isGood: boolean }>>(new Map())
  const resizeRef = useRef<number>(0)

  useEffect(() => {
    const canvas = canvasRef.current
    const container = containerRef.current
    if (!canvas || !container) return

    const dpr = window.devicePixelRatio || 1
    const rect = container.getBoundingClientRect()
    const W = rect.width, H = rect.height
    canvas.width = W * dpr; canvas.height = H * dpr
    canvas.style.width = W + 'px'; canvas.style.height = H + 'px'

    const ctx = canvas.getContext('2d')!
    ctx.scale(dpr, dpr)

    const pad = { l: 50, r: 24, t: 30, b: 40 }
    const pw = W - pad.l - pad.r, ph = H - pad.t - pad.b
    const midY = pad.t + ph / 2
    const maxAbs = Math.max(0.005, ...events.map(e => Math.abs(e.pcp)))
    const scale = (ph / 2 - 12) / maxAbs
    const xFromMin = (m: number) => pad.l + ((m - AM[0]) / (PM[1] - AM[0])) * pw

    hitMap.current.clear()

    // ── Background ──
    ctx.clearRect(0, 0, W, H)

    // Grid lines
    ctx.strokeStyle = 'rgb(var(--color-fg-rgb) / 0.04)'
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
    ctx.strokeStyle = 'rgb(var(--color-fg-rgb) / 0.1)'
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
      ctx.lineCap = 'round'
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
        ctx.strokeStyle = '#fff'
        ctx.lineWidth = 1.5
        ctx.setLineDash([])
        ctx.stroke()
      }

      ctx.globalAlpha = 1

      // Hit area: wider than visual for easier clicking
      hitMap.current.set(String(e.id), { id: e.id, x: cx, y1, y2, isGood })
    }

    const upColor = getComputedStyle(document.documentElement).getPropertyValue('--color-up').trim() || '#FF5C5C'
    const downColor = getComputedStyle(document.documentElement).getPropertyValue('--color-down').trim() || '#1D9E75'

    // Draw all bars first (non-glow)
    events.forEach(e => {
      const isNow = e.minute === curMin || e.id === hoveredId
      if (!isNow) drawBar(e, false)
    })

    // Draw glow bars on top
    events.forEach(e => {
      const isNow = e.minute === curMin || e.id === hoveredId
      if (isNow) drawBar(e, true)
    })

    // ── Current time cursor ──
    const curX = xFromMin(curMin)
    ctx.strokeStyle = getComputedStyle(document.documentElement).getPropertyValue('--color-accent').trim() || '#7F77DD'
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
    ctx.fillStyle = '#fff'
    ctx.beginPath()
    ctx.arc(curX, midY, 7, 0, Math.PI * 2)
    ctx.fill()
    ctx.strokeStyle = getComputedStyle(document.documentElement).getPropertyValue('--color-accent').trim() || '#7F77DD'
    ctx.lineWidth = 2.5
    ctx.stroke()

    // ── Labels ──
    ctx.fillStyle = 'rgb(var(--color-muted-rgb))'
    ctx.font = '9px Inter, system-ui, sans-serif'

    // X labels
    const xLabels = ['09:30', '10:00', '10:30', '11:00', '11:30', '13:00', '13:30', '14:00', '14:30', '15:00']
    xLabels.forEach(t => {
      const [h, mn] = t.split(':').map(Number)
      const m = h * 60 + mn
      const x = xFromMin(m)
      ctx.textAlign = 'center'
      ctx.fillText(t, x, H - 8)
    })

    // Y labels
    ctx.font = '8px Inter, system-ui, sans-serif'
    ctx.textAlign = 'end'
    ctx.fillStyle = upColor
    ctx.fillText(`+${(maxAbs * 100).toFixed(1)}%`, pad.l - 8, pad.t + 12)
    ctx.fillStyle = downColor
    ctx.fillText(`-${(maxAbs * 100).toFixed(1)}%`, pad.l - 8, H - pad.b + 4)
  }, [events, curMin, hoveredId])

  // Resize handler
  useEffect(() => {
    const onResize = () => { resizeRef.current++ }
    window.addEventListener('resize', onResize)
    return () => window.removeEventListener('resize', onResize)
  }, [])

  return (
    <div ref={containerRef} style={{ width: '100%', height: '100%', position: 'relative' }}>
      <canvas
        ref={canvasRef}
        style={{ display: 'block', width: '100%', height: '100%' }}
        onMouseMove={(e) => {
          const rect = canvasRef.current?.getBoundingClientRect()
          if (!rect) return
          const mx = e.clientX - rect.left
          const my = e.clientY - rect.top
          // Find nearest event
          let nearest: { id: number; dist: number } | null = null
          hitMap.current.forEach((v) => {
            if (mx >= v.x - 12 && mx <= v.x + 12 && my >= Math.min(v.y1, v.y2) - 4 && my <= Math.max(v.y1, v.y2) + 4) {
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
            if (mx >= v.x - 16 && mx <= v.x + 16 && my >= Math.min(v.y1, v.y2) - 6 && my <= Math.max(v.y1, v.y2) + 6) {
              const dist = Math.abs(mx - v.x)
              if (dist < minDist) { minDist = dist; clicked = v.id }
            }
          })
          onClick(clicked)
        }}
      />
    </div>
  )
}
