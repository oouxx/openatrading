import { useState, useEffect, useMemo } from 'react'
import { motion, AnimatePresence } from 'framer-motion'
import { type Stock } from '@/stores/data'
import { useLimitUp } from '@/hooks/query/useLimitUp'
import { useYesterdayLimitUp } from '@/hooks/query/useYesterdayLimitUp'
import { getKlineImageUrl } from '@/lib/stock-utils'

// ── Types ──────────────────────────────────────────────

interface PromotionItem {
  symbol: string
  name: string
  yesterdayDays: number
  todayDays: number
  status: 'promoted' | 'broken' | 'first'
  turnover: number
  capital: number
  changePercent: number
  reason: string
  breakTimes?: number
}

interface LadderRow {
  yesterdayDays: number
  yesterdayCount: number
  promotedCount: number
  brokenCount: number
  rate: number
}

function isST(name: string) { return name.includes('ST') || name.includes('*') }

function mergeData(todayList: Stock[], yesterdayList: Stock[]): PromotionItem[] {
  const yesterdayMap = new Map<string, Stock>()
  for (const s of yesterdayList) yesterdayMap.set(s.symbol, s)
  const result: PromotionItem[] = []
  for (const s of todayList) {
    const prev = yesterdayMap.get(s.symbol)
    if (prev) yesterdayMap.delete(s.symbol)
    const plates = s.surge_reason?.related_plates?.map((p) => p.plate_name).join(', ') || ''
    if (prev) {
      const yestDays = s.yesterday_limit_up_days || prev.yesterday_limit_up_days || 0
      result.push({
        symbol: s.symbol, name: s.stock_chi_name,
        yesterdayDays: yestDays, todayDays: s.limit_up_days,
        status: 'promoted', turnover: s.turnover_ratio,
        capital: s.total_capital, changePercent: s.change_percent,
        reason: plates, breakTimes: s.break_limit_up_times,
      })
    } else {
      result.push({
        symbol: s.symbol, name: s.stock_chi_name,
        yesterdayDays: 0, todayDays: s.limit_up_days,
        status: 'first', turnover: s.turnover_ratio,
        capital: s.total_capital, changePercent: s.change_percent,
        reason: plates,
      })
    }
  }
  for (const s of yesterdayMap.values()) {
    result.push({
      symbol: s.symbol, name: s.stock_chi_name,
      yesterdayDays: s.yesterday_limit_up_days || 0, todayDays: 0,
      status: 'broken', turnover: s.turnover_ratio || 0,
      capital: s.total_capital || 0, changePercent: 0, reason: '',
    })
  }
  return result
}

function fmtCap(v: number): string {
  if (v >= 1e12) return (v / 1e12).toFixed(2) + '万亿'
  if (v >= 1e8) return (v / 1e8).toFixed(0) + '亿'
  return v.toFixed(0)
}

function dayLabel(d: number): string {
  return d === 1 ? '首板' : d === 2 ? '二板' : d === 3 ? '三板' : d + '板'
}

// ── Table styles ───────────────────────────────────────

const thStyle: React.CSSProperties = {
  textAlign: 'left', padding: '9px 10px', fontSize: '0.66rem', fontWeight: 500,
  color: 'var(--color-muted, #888780)', textTransform: 'uppercase', letterSpacing: '0.03em',
  borderBottom: '1px solid var(--color-border)', background: 'rgb(var(--color-fg-rgb) / 0.04)',
  position: 'sticky', top: 0, zIndex: 1, whiteSpace: 'nowrap',
}

const tdStyle: React.CSSProperties = {
  padding: '7px 10px', borderBottom: '1px solid rgb(var(--color-fg-rgb) / 0.04)', whiteSpace: 'nowrap',
}

const csBase: React.CSSProperties = { fontFamily: 'JetBrains Mono, monospace', fontSize: '0.7rem', fontWeight: 600, padding: '2px 7px', borderRadius: 4 }

function chainEl(s: PromotionItem) {
  if (s.status === 'promoted') {
    return (
      <span style={{ display: 'inline-flex', alignItems: 'center', gap: 4 }}>
        <span style={{ ...csBase, color: 'oklch(50% 0.10 240)', background: 'oklch(92% 0.02 240 / 0.15)' }}>{dayLabel(s.yesterdayDays)}</span>
        <span style={{ color: 'var(--color-muted, #888780)', fontSize: '0.6rem' }}>→</span>
        <span style={{ ...csBase, color: 'var(--color-up)', background: 'oklch(90% 0.04 22 / 0.18)' }}>{dayLabel(s.todayDays)}</span>
      </span>
    )
  }
  if (s.status === 'broken') {
    return (
      <span style={{ display: 'inline-flex', alignItems: 'center', gap: 4 }}>
        <span style={{ ...csBase, color: 'oklch(50% 0.10 240)', background: 'oklch(92% 0.02 240 / 0.15)' }}>{dayLabel(s.yesterdayDays)}</span>
        <span style={{ color: 'var(--color-muted, #888780)', fontSize: '0.6rem' }}>→</span>
        <span style={{ ...csBase, color: 'var(--color-down)', background: 'oklch(90% 0.04 155 / 0.18)' }}>断</span>
      </span>
    )
  }
  return (
    <span style={{ display: 'inline-flex', alignItems: 'center', gap: 4 }}>
      <span style={{ fontSize: '0.65rem', color: 'var(--color-muted, #888780)' }}>—</span>
      <span style={{ color: 'var(--color-muted, #888780)', fontSize: '0.6rem' }}>→</span>
      <span style={{ ...csBase, color: 'oklch(55% 0.14 240)', background: 'oklch(90% 0.04 240 / 0.18)' }}>首板</span>
    </span>
  )
}

function resultEl(s: PromotionItem) {
  const styles: Record<string, React.CSSProperties> = {
    promoted: { background: 'oklch(90% 0.04 22 / 0.18)', color: 'var(--color-up)' },
    broken: { background: 'oklch(90% 0.04 155 / 0.2)', color: 'var(--color-down)' },
    first: { background: 'oklch(90% 0.04 240 / 0.18)', color: 'oklch(55% 0.14 240)' },
  }
  const labels = { promoted: '晋级', broken: '断板', first: '新进' }
  return (
    <span style={{ display: 'inline-block', padding: '2px 8px', borderRadius: 4, fontSize: '0.65rem', fontWeight: 600, ...styles[s.status] }}>
      {labels[s.status]}
    </span>
  )
}

// ── Main Component ────────────────────────────────────

export default function BreakoutView() {
  const [filter, setFilter] = useState<'all' | 'promoted' | 'broken' | 'first'>('all')
  const [activeLv, setActiveLv] = useState<number | null>(null)
  const [hideST, setHideST] = useState(true)
  const [selectedStock, setSelectedStock] = useState<PromotionItem | null>(null)
  const [klinePeriod, setKlinePeriod] = useState('minute')

  const { data: todayList = [], isLoading, error } = useLimitUp()
  const { data: yesterdayList = [] } = useYesterdayLimitUp()

  const hasYesterdayData = yesterdayList.length > 0

  const allItems = useMemo(() => {
    if (!todayList.length) return []
    if (!hasYesterdayData) {
      return todayList.map((s) => {
        const yestDays = s.yesterday_limit_up_days || 0
        return {
          symbol: s.symbol, name: s.stock_chi_name,
          yesterdayDays: yestDays, todayDays: s.limit_up_days,
          status: (yestDays > 0 ? 'promoted' : 'first') as 'promoted' | 'first',
          turnover: s.turnover_ratio, capital: s.total_capital,
          changePercent: s.change_percent,
          reason: s.surge_reason?.related_plates?.map((p) => p.plate_name).join(', ') || '',
          breakTimes: s.break_limit_up_times,
        }
      })
    }
    return mergeData(todayList, yesterdayList)
  }, [todayList, yesterdayList, hasYesterdayData])

  // Escape to close modal
  useEffect(() => {
    const onKey = (e: KeyboardEvent) => { if (e.key === 'Escape') setSelectedStock(null) }
    document.addEventListener('keydown', onKey)
    return () => document.removeEventListener('keydown', onKey)
  }, [])

  const items = useMemo(() => {
    let list = hideST ? allItems.filter(s => !isST(s.name)) : allItems
    if (filter !== 'all') list = list.filter(s => s.status === filter)
    if (activeLv !== null) {
      list = list.filter(s => {
        if (activeLv === 4) return s.yesterdayDays >= 4 || (s.status === 'first' && s.todayDays >= 4)
        return s.yesterdayDays === activeLv || (s.status === 'first' && activeLv === 0)
      })
    }
    return list
  }, [allItems, filter, activeLv, hideST])

  const promoted = allItems.filter(s => s.status === 'promoted')
  const broken = allItems.filter(s => s.status === 'broken')
  const first = allItems.filter(s => s.status === 'first')

  const ladder = useMemo((): LadderRow[] => {
    const groups = new Map<number, { promoted: PromotionItem[]; broken: PromotionItem[] }>()
    for (const s of promoted) {
      const g = groups.get(s.yesterdayDays) || { promoted: [], broken: [] }
      g.promoted.push(s); groups.set(s.yesterdayDays, g)
    }
    for (const s of broken) {
      const g = groups.get(s.yesterdayDays) || { promoted: [], broken: [] }
      g.broken.push(s); groups.set(s.yesterdayDays, g)
    }
    return [...groups.entries()].map(([days, g]) => {
      const total = g.promoted.length + g.broken.length
      return { yesterdayDays: days, yesterdayCount: total, promotedCount: g.promoted.length, brokenCount: g.broken.length, rate: total > 0 ? g.promoted.length / total : 0 }
    }).sort((a, b) => a.yesterdayDays - b.yesterdayDays)
  }, [promoted, broken])

  const stFiltered = hideST ? allItems.filter(s => isST(s.name)).length : 0
  const todayTotal = promoted.length + first.length
  const yesterdayTotal = promoted.length + broken.length
  const overallRate = yesterdayTotal > 0 ? (promoted.length / yesterdayTotal * 100) : 0

  const l1 = ladder.find(r => r.yesterdayDays === 1) || { promotedCount: 0, yesterdayCount: 0, rate: 0 }
  const l2 = ladder.find(r => r.yesterdayDays === 2) || { promotedCount: 0, yesterdayCount: 0, rate: 0 }
  const l3Above = ladder.filter(r => r.yesterdayDays >= 3)
  const l3Promoted = l3Above.reduce((s, r) => s + r.promotedCount, 0)
  const l3Total = l3Above.reduce((s, r) => s + r.yesterdayCount, 0)
  const l3Rate = l3Total > 0 ? l3Promoted / l3Total * 100 : 0

  if (isLoading) {
    return (
      <div className="flex min-h-screen items-center justify-center bg-bg pt-11" style={{ height: 'calc(100vh - 44px)' }}>
        <div className="flex items-center gap-3 text-xs text-ink-muted">
          <span className="size-2 animate-pulse rounded-full bg-ai" />加载晋级分析数据...
        </div>
      </div>
    )
  }

  if (error && allItems.length === 0) {
    return (
      <div className="flex min-h-screen items-center justify-center bg-bg pt-11" style={{ height: 'calc(100vh - 44px)' }}>
        <p className="text-xs text-ink-muted">{error.message}</p>
      </div>
    )
  }

  const statStyle = (s: { label: string; value: string; sub: string; hl?: boolean; color?: string }, idx: number) => (
    <div key={idx} className={`rounded-[10px] border bg-bg-surface px-4 py-3.5 text-center ${s.hl ? 'border-ai bg-ai/5' : 'border-hairline'}`}>
      <div className="mb-1 text-[11px] tracking-wider text-ink-muted uppercase">{s.label}</div>
      <div className="font-mono text-[1.4rem] font-semibold text-white" style={s.color ? { color: s.color } : undefined}>{s.value}</div>
      <div className="mt-0.5 text-[10px] text-ink-muted">{s.sub}</div>
    </div>
  )

  return (
    <div className="p-4">
      {/* Stats Row */}
        <div className="mb-[18px] grid grid-cols-3 gap-3 md:grid-cols-6">
          {[
            { label: '今日涨停', value: String(todayTotal), sub: `晋级${promoted.length}+新进${first.length}`, hl: true },
            { label: '1→2 晋级率', value: `${l1.rate >= 0 ? (l1.rate * 100).toFixed(1) : '—'}%`, sub: `${l1.promotedCount}/${l1.yesterdayCount}`, color: l1.rate >= 0.5 ? 'var(--color-up)' : 'var(--color-warn, oklch(55%_0.16_55))' },
            { label: '2→3 晋级率', value: `${l2.rate >= 0 ? (l2.rate * 100).toFixed(1) : '—'}%`, sub: `${l2.promotedCount}/${l2.yesterdayCount}`, color: l2.rate >= 0.5 ? 'var(--color-up)' : 'var(--color-warn, oklch(55%_0.16_55))' },
            { label: '3→4+ 晋级率', value: `${l3Rate.toFixed(1)}%`, sub: `${l3Promoted}/${l3Total}`, color: l3Rate >= 50 ? 'var(--color-up)' : 'var(--color-warn, oklch(55%_0.16_55))' },
            { label: '整体晋级率', value: `${overallRate.toFixed(1)}%`, sub: `${promoted.length}/${yesterdayTotal}`, color: overallRate >= 40 ? 'var(--color-up)' : 'var(--color-down)' },
            { label: '断板/新进', value: `🔻${broken.length} 🔷${first.length}`, sub: `断板率${yesterdayTotal > 0 ? (broken.length / yesterdayTotal * 100).toFixed(1) : '—'}%` },
          ].map(statStyle)}
        </div>

        {!hasYesterdayData && (
          <div className="mb-[18px] rounded-xl border border-hairline bg-bg-surface px-4 py-3 text-center text-xs text-ink-muted">
            未获取到昨日涨停列表，断板数据可能缺失
          </div>
        )}

        <div className="grid grid-cols-1 items-start gap-4 md:grid-cols-[220px_1fr]">
          {/* Filter Panel */}
          <div className="order-2 overflow-hidden rounded-xl border border-hairline bg-bg-surface md:order-none">
            <div className="border-b border-hairline px-3.5 py-3 text-[13px] font-medium text-white">筛选 & 统计</div>
            <div className="p-3">
              <div className="mb-3.5">
                <div className="mb-1.5 text-[10px] font-semibold tracking-wider text-ink-muted uppercase">结果类型</div>
                <div className="flex flex-wrap gap-1.5">
                  {(['all', 'promoted', 'broken', 'first'] as const).map((fid) => {
                    const labels = { all: '全部', promoted: '晋级', broken: '断板', first: '新进' }
                    const activeCls: Record<string, string> = {
                      promoted: 'bg-up border-up text-white',
                      broken: 'bg-down border-down text-white',
                      first: 'bg-[oklch(55%_0.14_240)] border-[oklch(55%_0.14_240)] text-white',
                      all: 'bg-ai border-ai text-white',
                    }
                    return (
                      <button key={fid} onClick={() => { setFilter(fid); if (fid !== 'all') setActiveLv(null) }}
                        className={`cursor-pointer rounded-2xl border px-3 py-1 text-[11px] transition-colors ${
                          filter === fid ? activeCls[fid] : 'border-hairline text-ink-muted hover:text-white'
                        }`}>{labels[fid]}</button>
                    )
                  })}
                </div>
              </div>
              <div className="mb-3.5">
                <label className="flex cursor-pointer items-center gap-2 text-[11px] text-ink-muted">
                  <input type="checkbox" checked={hideST} onChange={(e) => setHideST(e.target.checked)} />
                  隐藏ST ({stFiltered}只)
                </label>
              </div>
              <div>
                <div className="mb-1.5 text-[10px] font-semibold tracking-wider text-ink-muted uppercase">各板级晋级率</div>
                {[1, 2, 3, 4].map((b) => {
                  const row = b < 4 ? ladder.find(r => r.yesterdayDays === b) : null
                  const rate = b < 4 ? (row ? row.rate * 100 : 0) : l3Rate
                  const lb = b === 1 ? '首板' : b === 2 ? '二板' : b === 3 ? '三板' : '四板+'
                  const rc = rate >= 50 ? 'text-up' : rate >= 30 ? 'text-[oklch(55%_0.16_55)]' : 'text-down'
                  return (
                    <div key={b}
                      className={`mb-0.5 flex cursor-pointer items-center justify-between rounded-md px-2 py-1.5 text-[11px] transition-colors hover:bg-bg-hover ${activeLv === b ? 'bg-ai/10' : ''}`}
                      onClick={() => { if (activeLv === b) { setActiveLv(null) } else { setActiveLv(b); setFilter('all') } }}>
                      <span className="text-ink-muted">{lb} →</span>
                      <span className={`font-mono text-[10px] font-medium ${rc}`}>{rate.toFixed(1)}%</span>
                    </div>
                  )
                })}
              </div>
            </div>
          </div>

          {/* Table */}
          <div className="order-1 overflow-hidden rounded-xl border border-hairline bg-bg-surface md:order-none">
            <div className="flex items-center justify-between gap-2.5 border-b border-hairline px-4 py-2.5">
              <h3 className="text-[13px] font-medium text-white">涨停对比明细</h3>
              <span className="text-[11px] text-ink-muted">{items.length} 只</span>
            </div>
            <div className="overflow-auto" style={{ maxHeight: '62vh' }}>
              <table className="w-full border-collapse" style={{ fontSize: '0.77rem' }}>
                <thead>
                  <tr>
                    <th style={thStyle}>名称</th>
                    <th style={thStyle}>板块</th>
                    <th style={thStyle}>晋级链</th>
                    <th style={thStyle}>结果</th>
                    <th style={thStyle}>封板</th>
                    <th style={thStyle}>封单</th>
                  </tr>
                </thead>
                <tbody>
                  {items.length === 0 ? (
                    <tr><td colSpan={6} style={{ textAlign: 'center', padding: '36px', color: 'var(--color-muted, #888780)' }}>无匹配数据</td></tr>
                  ) : items.map((s) => {
                    const isSel = selectedStock?.symbol === s.symbol
                    const code = s.symbol.replace(/\.[A-Z]+$/, '')
                    return (
                      <tr key={s.symbol} style={{ cursor: 'pointer' }}
                        className={`${isSel ? '[&>td]:!bg-ai/10' : '[&>td]:hover:bg-bg-hover'}`}
                        onClick={() => { setSelectedStock(isSel ? null : s); if (!isSel) setKlinePeriod('minute') }}>
                        <td style={tdStyle}>
                          <span className="cursor-pointer font-medium text-ai underline decoration-[oklch(80%_0.04_28)] underline-offset-[3px]"
                            onClick={(e) => { e.stopPropagation(); setSelectedStock(s); setKlinePeriod('minute') }}
                            style={{ fontWeight: 500 }}>{s.name}</span>
                          <span style={{ fontSize: '0.68rem', color: 'var(--color-muted, #888780)', marginLeft: 3 }}>{code}</span>
                        </td>
                        <td style={tdStyle}>
                          <span style={{ fontSize: '0.64rem', color: 'var(--color-muted, #888780)', background: 'rgb(var(--color-fg-rgb) / 0.04)', padding: '2px 7px', borderRadius: 4 }}>
                            {s.reason.split(',')[0] || '—'}
                          </span>
                        </td>
                        <td style={tdStyle}>{chainEl(s)}</td>
                        <td style={tdStyle}>{resultEl(s)}</td>
                        <td style={{ ...tdStyle, fontFamily: 'JetBrains Mono, monospace', fontSize: '0.73rem', color: 'var(--color-muted, #888780)' }}>—</td>
                        <td style={{ ...tdStyle, color: 'var(--color-muted, #888780)' }}>—</td>
                      </tr>
                    )
                  })}
                </tbody>
              </table>
            </div>
          </div>
        </div>

      {/* Drawer */}
      <AnimatePresence>
        {selectedStock && (
          <>
            <div className="fixed inset-0 z-40" style={{ background: 'rgba(0,0,0,0.4)' }} onClick={() => setSelectedStock(null)} />
            <motion.div
              className="fixed top-0 right-0 z-50 h-full overflow-y-auto border-l border-hairline bg-bg-surface"
              style={{ width: 520 }}
              initial={{ x: 520 }}
              animate={{ x: 0 }}
              exit={{ x: 520 }}
              transition={{ duration: 0.3, ease: [0.32, 0.72, 0, 1] }}
            >
              {/* Header */}
              <div className="sticky top-0 z-10 border-b border-hairline bg-bg-surface p-4">
                <div className="mb-2 flex items-center justify-between">
                  <div className="flex items-center gap-2">
                    <span className="text-sm font-bold text-white">{selectedStock.name}</span>
                    <span className="font-mono text-xs text-ink-muted">{selectedStock.symbol.replace(/\.[A-Z]+$/, '')}</span>
                  </div>
                  <button onClick={() => setSelectedStock(null)} className="p-1 text-lg leading-none text-ink-muted hover:text-white">✕</button>
                </div>
                <div className="flex items-center gap-3 text-xs">
                  <span className={`font-mono font-semibold ${selectedStock.changePercent >= 0 ? 'text-up' : 'text-down'}`}>
                    {selectedStock.changePercent >= 0 ? '+' : ''}{(selectedStock.changePercent * 100).toFixed(2)}%
                  </span>
                  <span className="text-ink-muted">换手 {((selectedStock.turnover || 0) * 100).toFixed(2)}%</span>
                  <span className="text-ink-muted">市值 {fmtCap(selectedStock.capital)}</span>
                </div>
              </div>

              {/* K-line Tabs */}
              <div className="sticky top-[89px] z-10 flex gap-1 border-b border-hairline bg-bg-surface px-4 py-2">
                {[
                  { label: '分时', value: 'minute' },
                  { label: '日K', value: 'day' },
                  { label: '周K', value: 'week' },
                  { label: '月K', value: 'month' },
                ].map((p) => (
                  <button
                    key={p.value}
                    onClick={() => setKlinePeriod(p.value)}
                    className={`rounded-button px-3 py-1 font-mono text-xs transition ${
                      klinePeriod === p.value ? 'bg-ai text-white' : 'bg-transparent text-ink-muted'
                    }`}
                  >
                    {p.label}
                  </button>
                ))}
              </div>

              {/* K-line Image */}
              <div className="p-4">
                <img
                  src={getKlineImageUrl(selectedStock.symbol, klinePeriod)}
                  alt={`${selectedStock.name} K线图`}
                  className="w-full rounded-lg"
                  style={{ minHeight: 300 }}
                />
              </div>

              {/* Overview Metrics */}
              <OverviewPanel stock={selectedStock} />

              {/* Sector/Reason below */}
              {selectedStock.reason && (
                <div className="border-t border-hairline px-4 py-3">
                  <p className="mb-2 text-xs text-ink-muted">板块分析</p>
                  <p className="text-xs leading-relaxed text-white/70">{selectedStock.reason}</p>
                </div>
              )}
            </motion.div>
          </>
        )}
      </AnimatePresence>
    </div>
  )
}

function OverviewPanel({ stock }: { stock: PromotionItem }) {
  const cap = stock.capital || 0
  const up = stock.changePercent >= 0
  const statusLabel = stock.status === 'promoted' ? `${dayLabel(stock.yesterdayDays)}→${dayLabel(stock.todayDays)}` : stock.status === 'first' ? '首板' : `${dayLabel(stock.yesterdayDays)}→断板`
  const metrics = [
    { label: '晋级状态', value: statusLabel, cls: stock.status === 'promoted' ? 'text-up' : stock.status === 'broken' ? 'text-down' : 'text-ai' },
    { label: '今日涨幅', value: `${up ? '+' : ''}${(stock.changePercent * 100).toFixed(2)}%`, cls: up ? 'text-up' : 'text-down' },
    { label: '换手率', value: `${((stock.turnover || 0) * 100).toFixed(2)}%` },
    { label: '总市值', value: fmtCap(cap) },
    { label: '昨板天数', value: stock.status === 'first' ? '—' : `${stock.yesterdayDays}板` },
    { label: '今板天数', value: stock.status === 'broken' ? '断板' : `${stock.todayDays}板` },
    { label: '封板次数', value: stock.breakTimes != null ? String(stock.breakTimes) : '—' },
    { label: '连板', value: stock.todayDays > 1 ? `${stock.todayDays}连板` : '—', cls: stock.todayDays > 1 ? 'text-ai' : '' },
    { label: '昨收', value: '—' },
    { label: '量比', value: '—' },
    { label: '成交额', value: '—' },
    { label: '成交量', value: '—' },
  ]
  return (
    <div className="grid grid-cols-3 gap-2 p-4 text-xs">
      {metrics.map((m) => (
        <div key={m.label} className="rounded-card border border-hairline bg-bg p-3 text-center">
          <p className="text-[11px] text-ink-muted">{m.label}</p>
          <p className={`mt-1 font-mono text-xs text-white ${m.cls || ''}`}>{m.value}</p>
        </div>
      ))}
    </div>
  )
}
