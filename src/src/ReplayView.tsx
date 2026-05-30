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

function tsToMinute(ts: number, date: string) {
  const base = new Date(`${date}T00:00:00+08:00`).getTime() / 1000
  return Math.round((ts - base) / 60)
}

// ── Types ──────────────────────────────────────────────

interface ReplayEvent {
  id: number
  event_type: number
  event_timestamp: number
  good_or_bad: number
  plate_name?: string
  pcp?: number
  related_stocks?: RelatedStock[]
}

interface RelatedStock {
  symbol: string
  name: string
  mtm: number
  pcp: number
}

interface ReplayPage {
  date: string
  market_phase: string
  events: ReplayEvent[]
  next_cursor: number | null
}

interface PlateEvent {
  id: number
  minute: number
  plateName: string
  mtm: number
  goodOrBad: number
}

interface StockState { name: string; symbol: string; mtm: number; pcp: number }

// ── SVG Mini Chart ─────────────────────────────────────

function MiniChart({ points, curMin }: { points: { minute: number; change: number; hasEvent: boolean; gob: number }[]; curMin: number }) {
  const W = 300; const H = 50; const padL = 30; const padR = 10; const padT = 5; const padB = 8
  const pw = W - padL - padR; const ph = H - padT - padB
  const changes = points.map(p => p.change)
  const minC = Math.min(...changes, -4); const maxC = Math.max(...changes, 4)
  const range = maxC - minC || 1
  const x = (i: number) => padL + (i / (points.length - 1)) * pw
  const y = (v: number) => padT + (1 - (v - minC) / range) * ph
  const zeroY = y(0)
  const lastUp = points[points.length - 1].change >= points[0].change
  const lineColor = lastUp ? 'var(--color-up)' : 'var(--color-down)'

  let pathD = ''
  points.forEach((p, i) => { pathD += `${i === 0 ? 'M' : 'L'}${x(i).toFixed(1)},${y(p.change).toFixed(1)} ` })

  return (
    <svg viewBox={`0 0 ${W} ${H}`} width="100%" height="50">
      <line x1={padL} y1={zeroY} x2={W - padR} y2={zeroY} stroke="#888780" strokeWidth="0.5" strokeDasharray="4,3" />
      <path d={pathD} fill="none" stroke={lineColor} strokeWidth="1.8" strokeLinecap="round" strokeLinejoin="round" />
      {points.filter(p => p.hasEvent).map((p) => {
        const idx = points.indexOf(p)
        return <circle key={`e${idx}`} cx={x(idx).toFixed(1)} cy={y(p.change).toFixed(1)} r="3" fill={p.gob === 1 ? 'var(--color-up)' : 'var(--color-down)'} stroke="#fff" strokeWidth="1" />
      })}
      {(() => {
        const ci = TRADING_MINUTES.indexOf(curMin)
        if (ci < 0) return null
        return (
          <>
            <line x1={x(ci)} y1={padT} x2={x(ci)} y2={H - padB} stroke="var(--color-accent)" strokeWidth="1.5" strokeDasharray="4,2" opacity="0.7" />
            <circle cx={x(ci)} cy={y(points[ci]?.change || 0)} r="4" fill="#fff" stroke="var(--color-accent)" strokeWidth="2" />
          </>
        )
      })()}
    </svg>
  )
}

// ── Process API data ───────────────────────────────────

function processEvents(events: ReplayEvent[], date: string): {
  plateEvents: PlateEvent[]
  allStocks: StockState[]
  sectorSet: string[]
  latestStocks: Map<string, StockState[]>
} {
  const plateEvents: PlateEvent[] = []
  const allStockStates = new Map<string, StockState>()
  const sectorStocks = new Map<string, StockState[]>()

  events.forEach((e) => {
    const minute = tsToMinute(e.event_timestamp, date)

    if (e.plate_name) {
      plateEvents.push({
        id: e.id,
        minute,
        plateName: e.plate_name,
        mtm: e.pcp || 0,
        goodOrBad: e.good_or_bad,
      })
    }

    if (e.related_stocks) {
      e.related_stocks.forEach((s) => {
        const key = s.symbol
        allStockStates.set(key, { name: s.name, symbol: s.symbol, mtm: s.mtm, pcp: s.pcp })

        // Group stocks by their first seen plate association
        const plateName = e.plate_name || '其他'
        if (!sectorStocks.has(plateName)) sectorStocks.set(plateName, [])
        const existing = sectorStocks.get(plateName)!
        if (!existing.find(x => x.symbol === s.symbol)) {
          existing.push({ name: s.name, symbol: s.symbol, mtm: s.mtm, pcp: s.pcp })
        }
      })
    }
  })

  plateEvents.sort((a, b) => a.minute - b.minute)

  const sectorSet = [...new Set(plateEvents.map(e => e.plateName))]

  // Build sector→stocks map with latest state
  const latestStocks = new Map<string, StockState[]>()
  events.forEach((e) => {
    if (e.related_stocks && e.plate_name) {
      const list = latestStocks.get(e.plate_name) || []
      e.related_stocks.forEach((s) => {
        const existing = list.find(x => x.symbol === s.symbol)
        if (existing) {
          existing.mtm = s.mtm
          existing.pcp = s.pcp
        } else {
          list.push({ name: s.name, symbol: s.symbol, mtm: s.mtm, pcp: s.pcp })
        }
      })
      latestStocks.set(e.plate_name, list)
    }
  })

  return {
    plateEvents,
    allStocks: [...allStockStates.values()],
    sectorSet,
    latestStocks,
  }
}

// ── Main Component ────────────────────────────────────

export default function ReplayView() {
  const [date, setDate] = useState('')
  const [loading, setLoading] = useState(true)
  const [error, setError] = useState('')
  const [plateEvents, setPlateEvents] = useState<PlateEvent[]>([])
  const [allStocks, setAllStocks] = useState<StockState[]>([])
  const [sectorSet, setSectorSet] = useState<string[]>([])
  const [latestStocks, setLatestStocks] = useState<Map<string, StockState[]>>(new Map())
  const [selectedSector, setSelectedSector] = useState('')
  const [currentMinute, setCurrentMinute] = useState(AM[0])
  const [isPlaying, setIsPlaying] = useState(false)
  const [playSpeed, setPlaySpeed] = useState(5)
  const [rankMode, setRankMode] = useState<'speed' | 'change'>('speed')
  const timerRef = useRef<ReturnType<typeof setInterval> | null>(null)
  // 一次性拉取当天所有事件
  useEffect(() => {
    async function fetchAllEvents() {
      setLoading(true)
      setError('')
      try {
        const resp = await apiFetch('/api/replay/events')
        if (!resp.ok) throw new Error(`HTTP ${resp.status}`)
        const page: ReplayPage = await resp.json()
        setDate(page.date)

        const processed = processEvents(page.events, page.date)
        setPlateEvents(processed.plateEvents)
        setAllStocks(processed.allStocks)
        setSectorSet(processed.sectorSet)
        setLatestStocks(processed.latestStocks)
        if (processed.sectorSet.length > 0) {
          setSelectedSector(processed.sectorSet[0])
        }
      } catch (e: unknown) {
        setError((e as Error).message || '加载失败')
      } finally {
        setLoading(false)
      }
    }
    fetchAllEvents()
  }, [])

  useEffect(() => () => { if (timerRef.current) clearInterval(timerRef.current) }, [])

  const stopPlay = useCallback(() => {
    setIsPlaying(false)
    if (timerRef.current) { clearInterval(timerRef.current); timerRef.current = null }
  }, [])

  const startPlay = useCallback(() => {
    setIsPlaying(true)
    timerRef.current = setInterval(() => {
      setCurrentMinute((prev) => {
        const idx = TRADING_MINUTES.indexOf(prev) + playSpeed
        return idx >= TRADING_MINUTES.length ? TRADING_MINUTES[0] : TRADING_MINUTES[idx]
      })
    }, 120)
  }, [playSpeed])

  const togglePlay = () => { isPlaying ? stopPlay() : startPlay() }

  const stepTime = (dir: number) => {
    stopPlay()
    setCurrentMinute((prev) => {
      const idx = TRADING_MINUTES.indexOf(prev) + dir
      return idx >= 0 && idx < TRADING_MINUTES.length ? TRADING_MINUTES[idx] : prev
    })
  }

  const setSpeed = (sp: number) => {
    setPlaySpeed(sp)
    if (isPlaying) { stopPlay(); setTimeout(() => startPlay(), 0) }
  }

  const goToTime = (min: number) => {
    stopPlay()
    setCurrentMinute(Math.max(AM[0], Math.min(PM[1], min)))
  }

  // Loading state — show partial data while prefetching
  if (loading && plateEvents.length === 0) {
    return (
      <div className="min-h-screen bg-bg pt-11 flex items-center justify-center" style={{ height: 'calc(100vh - 44px)' }}>
        <div className="flex flex-col items-center gap-3">
          <div className="flex items-center gap-3 text-xs text-ink-muted">
            <span className="w-2 h-2 rounded-full bg-ai animate-pulse" />
            加载回放数据...
          </div>
          {date && (
            <span className="text-[11px] text-ink-muted font-mono">交易日：{date}</span>
          )}
        </div>
      </div>
    )
  }

  if (error) {
    return (
      <div className="min-h-screen bg-bg pt-11 flex items-center justify-center" style={{ height: 'calc(100vh - 44px)' }}>
        <div className="text-center">
          <p className="text-up text-sm mb-2">数据加载失败</p>
          <p className="text-xs text-ink-muted">{error}</p>
        </div>
      </div>
    )
  }

  if (plateEvents.length === 0) {
    return (
      <div className="min-h-screen bg-bg pt-11 flex items-center justify-center" style={{ height: 'calc(100vh - 44px)' }}>
        <p className="text-xs text-ink-muted">当日无回放数据</p>
      </div>
    )
  }

  // ── Derived data for current minute ──────────────────
  const plateUpToNow = plateEvents.filter(e => e.minute <= currentMinute)
  const sectorChanges: Record<string, number> = {}
  sectorSet.forEach((s) => {
    const evts = plateUpToNow.filter(e => e.plateName === s)
    sectorChanges[s] = evts.length ? evts[evts.length - 1].mtm : 0
  })
  const sectorNowEvents: Record<string, PlateEvent | undefined> = {}
  plateEvents.filter(e => e.minute === currentMinute).forEach(e => { sectorNowEvents[e.plateName] = e })

  // Mini chart points for selected sector
  const miniChartPoints = TRADING_MINUTES.map((m) => {
    const evts = plateEvents.filter(e => e.plateName === selectedSector && e.minute <= m)
    const chg = evts.length ? evts[evts.length - 1].mtm : 0
    const hit = plateEvents.some(e => e.plateName === selectedSector && e.minute === m)
    const hitEvt = plateEvents.find(e => e.plateName === selectedSector && e.minute === m)
    return { minute: m, change: chg, hasEvent: hit, gob: hitEvt?.goodOrBad || 0 }
  })

  // Ranking: use latestStocks for selected sector, or allStocks
  const rankStocks = selectedSector && latestStocks.has(selectedSector)
    ? (latestStocks.get(selectedSector) || allStocks)
    : allStocks

  const sortedBySpeed = [...rankStocks].sort((a, b) => b.mtm - a.mtm)
  const sortedByChange = [...rankStocks].sort((a, b) => b.pcp - a.pcp)
  const ranked = rankMode === 'speed' ? sortedBySpeed : sortedByChange
  const maxSpeed = Math.max(1, ...ranked.map(s => Math.abs(s.mtm)))

  const scrubberPct = (TRADING_MINUTES.indexOf(currentMinute) / (TRADING_MINUTES.length - 1)) * 100
  const rankingTitle = rankMode === 'speed' ? '涨速排名' : '涨幅排名'

  return (
    <div className="min-h-screen bg-bg pt-11 flex flex-col" style={{ height: 'calc(100vh - 44px)' }}>
      {/* Date Badge */}
      <div className="flex items-center justify-between px-4 py-2 bg-bg-surface border-b border-hairline flex-shrink-0 max-md:hidden">
        <span className="text-[13px] font-medium text-white">行情回放</span>
        {date && (
          <span className="text-xs text-ink-muted font-mono">{date}</span>
        )}
      </div>

      <div className="flex flex-1 overflow-hidden">
        {/* Left Panel */}
        <aside className="w-[340px] min-w-[340px] bg-bg-surface border-r border-hairline flex flex-col overflow-y-auto max-md:hidden">
          <div className="px-4 py-3.5 border-b border-hairline">
            <h4 className="text-[13px] font-medium text-white mb-2.5 flex items-center justify-between">
              板块涨跌 <span className="text-[11px] text-ink-muted font-normal">{minToTime(currentMinute)}</span>
            </h4>
            <div className="grid grid-cols-2 gap-1.5">
              {sectorSet.map((s) => {
                const chg = sectorChanges[s]
                const up = chg >= 0
                const sel = s === selectedSector
                const sig = sectorNowEvents[s]
                return (
                  <button key={s} onClick={() => setSelectedSector(s)}
                    className={`relative flex items-center justify-between px-2.5 py-2 rounded-lg border text-xs cursor-pointer transition-colors ${
                      sel ? 'border-ai bg-ai/5' : 'border-hairline hover:border-ai'
                    }`}>
                    {sig && (
                      <span className={`absolute -top-1 -right-1 text-[9px] px-1.5 py-0.5 rounded-lg font-bold text-white ${sig.goodOrBad === 1 ? 'bg-up' : 'bg-down'}`}>
                        {sig.goodOrBad === 1 ? '利好' : '利空'}
                      </span>
                    )}
                    <span className="font-medium text-white truncate max-w-[90px]">{s}</span>
                    <span className={`font-mono font-semibold text-xs px-1.5 py-0.5 rounded flex-shrink-0 ${up ? 'bg-up/20 text-up' : 'bg-down/20 text-down'}`}>
                      {up ? '+' : ''}{chg.toFixed(2)}%
                    </span>
                  </button>
                )
              })}
            </div>
          </div>

          <div className="px-4 py-3.5 border-b border-hairline">
            <h4 className="text-[13px] font-medium text-white mb-2.5">
              {selectedSector || '板块'} 日内走势
            </h4>
            <MiniChart points={miniChartPoints} curMin={currentMinute} />
            <div className="text-[10px] text-ink-muted mt-1">
              <span style={{ marginRight: 8 }}>— 昨收基准线</span>
              <span style={{ color: 'var(--color-up)', marginRight: 8 }}>● 利好</span>
              <span style={{ color: 'var(--color-down)' }}>● 利空</span>
            </div>
          </div>

          <div className="flex-1 overflow-y-auto px-4 py-3.5">
            <h4 className="text-[13px] font-medium text-white mb-2.5">板块信号时间线</h4>
            {plateEvents.map((e) => {
              const isPast = e.minute <= currentMinute
              const isNow = e.minute === currentMinute
              return (
                <div key={e.id}
                  className={`flex items-start gap-2 py-1.5 px-1.5 rounded text-[11px] cursor-pointer transition-colors border-b border-hairline last:border-b-0 hover:bg-bg-hover ${isNow ? 'bg-ai/10' : ''}`}
                  style={{ opacity: isPast ? 1 : 0.35 }}
                  onClick={() => goToTime(e.minute)}>
                  <span className={`w-[7px] h-[7px] rounded-full mt-0.5 flex-shrink-0 ${e.goodOrBad === 1 ? 'bg-up' : 'bg-down'}`} />
                  <span className="font-mono text-[11px] text-ink-muted min-w-[42px]">{minToTime(e.minute)}</span>
                  <span>
                    <span className="font-semibold text-white">{e.plateName}</span>{' '}
                    {e.goodOrBad === 1 ? '利好' : '利空'}{' '}
                    <span className="text-ink-muted">{e.goodOrBad === 1 ? '+' : ''}{e.mtm.toFixed(2)}%</span>
                  </span>
                </div>
              )
            })}
          </div>
        </aside>

        {/* Center — Ranking */}
        <div className="flex-1 flex flex-col min-w-0 overflow-hidden">
          <div className="flex-1 overflow-y-auto px-4 py-3">
            <div className="flex items-center justify-between py-1 pb-2.5 mb-0 sticky top-0 bg-bg z-10">
              <h3 className="text-[15px] font-medium text-white">{rankingTitle}</h3>
              <div className="flex gap-1">
                <button onClick={() => setRankMode('speed')}
                  className={`px-3 py-1 rounded-md text-[11px] cursor-pointer transition-colors ${rankMode === 'speed' ? 'bg-ai text-white' : 'border border-hairline text-ink-muted hover:text-white'}`}>涨速</button>
                <button onClick={() => setRankMode('change')}
                  className={`px-3 py-1 rounded-md text-[11px] cursor-pointer transition-colors ${rankMode === 'change' ? 'bg-ai text-white' : 'border border-hairline text-ink-muted hover:text-white'}`}>涨幅</button>
              </div>
            </div>
            <div className="overflow-x-auto">
              <table className="w-full border-collapse">
                <thead>
                  <tr>
                    <th className="text-left px-2.5 py-2 text-[11px] font-medium text-ink-muted uppercase tracking-wider border-b border-hairline bg-bg sticky top-[52px] z-[1] whitespace-nowrap">#</th>
                    <th className="text-left px-2.5 py-2 text-[11px] font-medium text-ink-muted uppercase tracking-wider border-b border-hairline bg-bg sticky top-[52px] z-[1] whitespace-nowrap">名称</th>
                    <th className="text-left px-2.5 py-2 text-[11px] font-medium text-ink-muted uppercase tracking-wider border-b border-hairline bg-bg sticky top-[52px] z-[1] whitespace-nowrap">涨速</th>
                    <th className="text-left px-2.5 py-2 text-[11px] font-medium text-ink-muted uppercase tracking-wider border-b border-hairline bg-bg sticky top-[52px] z-[1] whitespace-nowrap">涨幅</th>
                  </tr>
                </thead>
                <tbody>
                  {ranked.slice(0, 40).map((st, i) => {
                    const upSp = st.mtm >= 0; const upCh = st.pcp >= 0
                    return (
                      <tr key={st.symbol} className="hover:bg-bg-hover transition-colors">
                        <td className={`w-8 font-mono font-semibold text-xs py-1.5 px-2.5 border-b border-hairline ${i < 3 ? 'text-ai' : 'text-ink-muted'}`}>{i + 1}</td>
                        <td className="py-1.5 px-2.5 border-b border-hairline">
                          <span className="font-medium text-xs text-white">{st.name}</span>
                          <span className="text-[11px] text-ink-muted ml-1">{st.symbol}</span>
                        </td>
                        <td className={`py-1.5 px-2.5 border-b border-hairline font-mono text-xs font-medium whitespace-nowrap ${upSp ? 'text-up' : 'text-down'}`}>
                          {upSp ? '+' : ''}{st.mtm.toFixed(2)}%
                          <span className="inline-block w-20 h-1 rounded-sm ml-1.5 align-middle bg-ink-dim">
                            <span className={`block h-full rounded-sm ${upSp ? 'bg-up' : 'bg-down'}`}
                              style={{ width: `${Math.min(80, (Math.abs(st.mtm) / maxSpeed) * 80)}px` }} />
                          </span>
                        </td>
                        <td className={`py-1.5 px-2.5 border-b border-hairline font-mono text-xs font-medium ${upCh ? 'text-up' : 'text-down'}`}>
                          {upCh ? '+' : ''}{st.pcp.toFixed(2)}%
                        </td>
                      </tr>
                    )
                  })}
                </tbody>
              </table>
            </div>
          </div>
        </div>
      </div>

      {/* Bottom Timeline */}
      <div className="flex-shrink-0 bg-bg-surface border-t border-hairline px-5 pt-2 pb-3">
        <div className="flex items-center gap-2.5 mb-1">
          <button onClick={() => stepTime(-1)} className="w-7 h-7 rounded-md border border-hairline bg-transparent text-ink-muted cursor-pointer flex items-center justify-center text-[11px] hover:border-ai hover:text-white transition-colors">⏮</button>
          <button onClick={togglePlay} className="w-[34px] h-[34px] rounded-full bg-ai text-white cursor-pointer flex items-center justify-center text-sm hover:opacity-90 transition-opacity">{isPlaying ? '⏸' : '▶'}</button>
          <button onClick={() => stepTime(1)} className="w-7 h-7 rounded-md border border-hairline bg-transparent text-ink-muted cursor-pointer flex items-center justify-center text-[11px] hover:border-ai hover:text-white transition-colors">⏭</button>
          <div className="flex gap-0.5">
            {[1, 5, 15, 60].map((sp) => (
              <button key={sp} onClick={() => setSpeed(sp)}
                className={`px-2.5 py-0.5 rounded text-[11px] font-mono cursor-pointer transition-colors ${playSpeed === sp ? 'bg-ai text-white' : 'border border-hairline text-ink-muted hover:text-white'}`}>{sp}×</button>
            ))}
          </div>
          <span className="font-mono text-sm font-semibold text-white ml-auto">{minToTime(currentMinute)}</span>
          <button onClick={() => goToTime(AM[0])} className="px-3 py-1 rounded-md border border-hairline text-[11px] text-ink-muted hover:text-white transition-colors cursor-pointer">开盘</button>
          <button onClick={() => goToTime(PM[1])} className="px-3 py-1 rounded-md border border-hairline text-[11px] text-ink-muted hover:text-white transition-colors cursor-pointer">收盘</button>
        </div>
        <div className="flex justify-between text-[10px] text-ink-muted px-0.5 mb-0.5">
          {['09:30','10:00','10:30','11:00','11:30','13:00','13:30','14:00','14:30','15:00'].map(t => <span key={t}>{t}</span>)}
        </div>
        <div className="relative h-8 rounded-md cursor-pointer bg-ink-dim"
          onClick={(e) => {
            stopPlay()
            const rect = e.currentTarget.getBoundingClientRect()
            const pct = (e.clientX - rect.left) / rect.width
            const idx = Math.floor(pct * (TRADING_MINUTES.length - 1))
            setCurrentMinute(TRADING_MINUTES[Math.max(0, Math.min(TRADING_MINUTES.length - 1, idx))])
          }}>
          <svg className="absolute inset-0 w-full h-full" viewBox="0 0 800 32" preserveAspectRatio="none">
            {plateEvents.map((e) => {
              const si = TRADING_MINUTES.indexOf(e.minute)
              if (si < 0) return null
              return <circle key={e.id} cx={(si / (TRADING_MINUTES.length - 1)) * 800} cy="16" r="3.5"
                fill={e.goodOrBad === 1 ? 'var(--color-up)' : 'var(--color-down)'} stroke="#fff" strokeWidth="1" />
            })}
          </svg>
          <div className="absolute -top-1.5 w-[3px] rounded-sm bg-ai pointer-events-none z-10 shadow-md"
            style={{ left: `${scrubberPct}%`, height: 'calc(100% + 12px)' }}>
            <span className="absolute -top-2.5 left-1/2 -translate-x-1/2 text-[10px] text-ai">▼</span>
          </div>
        </div>
      </div>
    </div>
  )
}
