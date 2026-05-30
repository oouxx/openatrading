import { useState, useEffect } from 'react'
import { apiFetch } from '@/stores/data'

interface DeptDetail {
  name: string
  amount: number
  dept_type: string
}

interface DragonTigerEntry {
  stock_code: string
  stock_name: string
  trade_date: string
  reason: string
  close_price: number | null
  change_pct: number | null
  inst_buy_amount: number | null
  inst_sell_amount: number | null
  dept_buy_amount: number | null
  dept_sell_amount: number | null
  net_buy: number
  buy_depts: DeptDetail[]
  sell_depts: DeptDetail[]
}

export default function DragonTigerView() {
  const [data, setData] = useState<DragonTigerEntry[]>([])
  const [loading, setLoading] = useState(true)
  const [error, setError] = useState('')
  const [typeFilter, setTypeFilter] = useState<'all' | 'inst' | 'retail'>('all')
  const [dirFilter, setDirFilter] = useState<'all' | 'buy' | 'sell'>('all')
  const [selected, setSelected] = useState<DragonTigerEntry | null>(null)

  useEffect(() => {
    async function fetch() {
      setLoading(true)
      setError('')
      try {
        const resp = await apiFetch('/api/dragon-tiger')
        if (!resp.ok) throw new Error(`HTTP ${resp.status}`)
        const json: DragonTigerEntry[] = await resp.json()
        setData(json)
      } catch (e: unknown) {
        setError((e as Error).message || '加载失败')
      } finally { setLoading(false) }
    }
    fetch()
  }, [])

  useEffect(() => {
    const onKey = (e: KeyboardEvent) => { if (e.key === 'Escape') setSelected(null) }
    document.addEventListener('keydown', onKey)
    return () => document.removeEventListener('keydown', onKey)
  }, [])

  const getType = (e: DragonTigerEntry): 'inst' | 'retail' =>
    (e.inst_buy_amount || 0) + (e.inst_sell_amount || 0) > (e.dept_buy_amount || 0) + (e.dept_sell_amount || 0) ? 'inst' : 'retail'

  const filtered = data.filter(s => {
    if (typeFilter !== 'all' && getType(s) !== typeFilter) return false
    if (dirFilter === 'buy' && s.net_buy <= 0) return false
    if (dirFilter === 'sell' && s.net_buy >= 0) return false
    return true
  })

  if (loading) {
    return (
      <div className="min-h-screen bg-bg pt-11 flex items-center justify-center" style={{ height: 'calc(100vh - 44px)' }}>
        <div className="flex items-center gap-3 text-xs text-ink-muted">
          <span className="w-2 h-2 rounded-full bg-ai animate-pulse" />加载龙虎榜数据...
        </div>
      </div>
    )
  }

  if (error) {
    return (
      <div className="min-h-screen bg-bg pt-11 flex items-center justify-center" style={{ height: 'calc(100vh - 44px)' }}>
        <p className="text-xs text-ink-muted">{error}</p>
      </div>
    )
  }

  const inst = data.filter(s => getType(s) === 'inst')
  const retail = data.filter(s => getType(s) === 'retail')
  const totalNetInst = inst.reduce((s, i) => s + i.net_buy, 0)
  const totalNetRetail = retail.reduce((s, i) => s + i.net_buy, 0)
  const totalNet = totalNetInst + totalNetRetail

  const stats = [
    { l: '上榜家数', v: String(data.length), s: `机构${inst.length} · 游资${retail.length}`, hl: true },
    { l: '机构净买入', v: `${(totalNetInst / 10000).toFixed(2)}亿`, s: `${inst.length}家机构`, cl: totalNetInst >= 0 ? 'var(--color-up)' : 'var(--color-down)' },
    { l: '游资净买入', v: `${(totalNetRetail / 10000).toFixed(2)}亿`, s: `${retail.length}家游资`, cl: totalNetRetail >= 0 ? 'var(--color-up)' : 'var(--color-down)' },
    { l: '总净买入', v: `${(totalNet / 10000).toFixed(2)}亿`, s: '机构+游资合计', cl: totalNet >= 0 ? 'var(--color-up)' : 'var(--color-down)' },
    { l: '净买入家数', v: String(data.filter(s => s.net_buy > 0).length), s: `占比${(data.filter(s => s.net_buy > 0).length / data.length * 100).toFixed(0)}%` },
    { l: '净卖出家数', v: String(data.filter(s => s.net_buy < 0).length), s: `占比${(data.filter(s => s.net_buy < 0).length / data.length * 100).toFixed(0)}%` },
  ]

  const th = { textAlign: 'left' as const, padding: '9px 10px', fontSize: '0.66rem', fontWeight: 500, color: 'var(--color-muted, #888780)', textTransform: 'uppercase' as const, letterSpacing: '0.03em', borderBottom: '1px solid var(--color-border)', background: 'rgb(var(--color-fg-rgb) / 0.04)', position: 'sticky' as const, top: 0, zIndex: 1, whiteSpace: 'nowrap' as const }
  const td = { padding: '7px 10px', borderBottom: '1px solid rgb(var(--color-fg-rgb) / 0.04)', whiteSpace: 'nowrap' as const }

  return (
    <div className="min-h-screen bg-bg pt-11">
      <div className="max-w-[1500px] mx-auto p-6 w-full">
        {/* Stats */}
        <div className="grid grid-cols-3 md:grid-cols-6 gap-3 mb-[18px]">
          {stats.map(s => (
            <div key={s.l} className={`bg-bg-surface border rounded-[10px] px-4 py-3.5 text-center ${s.hl ? 'border-ai bg-ai/5' : 'border-hairline'}`}>
              <div className="text-[11px] text-ink-muted uppercase tracking-wider mb-1">{s.l}</div>
              <div className="font-mono text-[1.4rem] font-semibold text-white" style={s.cl ? { color: s.cl } : undefined}>{s.v}</div>
              <div className="text-[10px] text-ink-muted mt-0.5">{s.s}</div>
            </div>
          ))}
        </div>

        <div className="grid grid-cols-1 md:grid-cols-[220px_1fr] gap-4 items-start">
          {/* Filter */}
          <div className="bg-bg-surface border border-hairline rounded-xl overflow-hidden md:order-none order-2">
            <div className="px-3.5 py-3 border-b border-hairline text-[13px] font-medium text-white">筛选</div>
            <div className="p-3">
              <div className="mb-3.5">
                <div className="text-[10px] font-semibold text-ink-muted uppercase tracking-wider mb-1.5">上榜类型</div>
                <div className="flex flex-wrap gap-1.5">
                  {(['all', 'inst', 'retail'] as const).map(t => {
                    const labels = { all: '全部', inst: '机构', retail: '游资' }
                    const activeCls = t === 'inst' ? 'bg-up border-up text-white' : t === 'retail' ? 'bg-down border-down text-white' : 'bg-ai border-ai text-white'
                    return (
                      <button key={t} onClick={() => setTypeFilter(t)}
                        className={`px-3 py-1 rounded-2xl border text-[11px] cursor-pointer transition-colors ${typeFilter === t ? activeCls : 'border-hairline text-ink-muted hover:text-white'}`}>{labels[t]}</button>
                    )
                  })}
                </div>
              </div>
              <div>
                <div className="text-[10px] font-semibold text-ink-muted uppercase tracking-wider mb-1.5">净买卖</div>
                <div className="flex flex-wrap gap-1.5">
                  {(['all', 'buy', 'sell'] as const).map(d => {
                    const labels = { all: '全部', buy: '净买入', sell: '净卖出' }
                    const activeCls = d === 'buy' ? 'bg-up border-up text-white' : d === 'sell' ? 'bg-down border-down text-white' : 'bg-ai border-ai text-white'
                    return (
                      <button key={d} onClick={() => setDirFilter(d)}
                        className={`px-3 py-1 rounded-2xl border text-[11px] cursor-pointer transition-colors ${dirFilter === d ? activeCls : 'border-hairline text-ink-muted hover:text-white'}`}>{labels[d]}</button>
                    )
                  })}
                </div>
              </div>
            </div>
          </div>

          {/* Table */}
          <div className="bg-bg-surface border border-hairline rounded-xl overflow-hidden md:order-none order-1">
            <div className="px-4 py-2.5 border-b border-hairline flex items-center justify-between gap-2.5">
              <h3 className="text-[13px] font-medium text-white">龙虎榜上榜个股</h3>
              <span className="text-[11px] text-ink-muted">{filtered.length} 只</span>
            </div>
            <div className="overflow-auto" style={{ maxHeight: '62vh' }}>
              <table className="w-full border-collapse" style={{ fontSize: '0.77rem' }}>
                <thead><tr><th style={th}>名称</th><th style={th}>净买入(万)</th><th style={th}>买方营业部</th><th style={th}>卖方营业部</th><th style={th}>上榜原因</th></tr></thead>
                <tbody>
                  {filtered.length === 0 ? (
                    <tr><td colSpan={5} style={{ textAlign: 'center', padding: '36px', color: 'var(--color-muted, #888780)' }}>无匹配数据</td></tr>
                  ) : filtered.map(s => {
                    const up = s.net_buy >= 0
                    return (
                      <tr key={s.stock_code} className={`cursor-pointer transition-colors [&>td]:hover:bg-bg-hover ${selected?.stock_code === s.stock_code ? '[&>td]:!bg-ai/10' : ''}`}
                        onClick={() => setSelected(selected?.stock_code === s.stock_code ? null : s)}>
                        <td style={td}><span className="font-medium text-ai cursor-pointer hover:underline">{s.stock_name}</span><span className="text-[11px] text-ink-muted ml-1">{s.stock_code}</span></td>
                        <td style={td}><span className={`font-mono font-semibold text-xs ${up ? 'text-up' : 'text-down'}`}>{up ? '+' : ''}{(s.net_buy / 10000).toFixed(2)}亿</span></td>
                        <td style={{ ...td, fontSize: '0.7rem', maxWidth: 180, whiteSpace: 'normal' }}>{s.inst_buy_amount || s.dept_buy_amount ? `${(s.inst_buy_amount || 0) + (s.dept_buy_amount || 0) > 0 ? '有买入席位' : '—'}` : '—'}</td>
                        <td style={{ ...td, fontSize: '0.7rem', maxWidth: 180, whiteSpace: 'normal' }}>{s.inst_sell_amount || s.dept_sell_amount ? `${(s.inst_sell_amount || 0) + (s.dept_sell_amount || 0) > 0 ? '有卖出席位' : '—'}` : '—'}</td>
                        <td style={td}><span className="text-[10px] text-ink-muted max-w-[180px] inline-block whitespace-normal">{s.reason}</span></td>
                      </tr>
                    )
                  })}
                </tbody>
              </table>
            </div>
          </div>
        </div>

        {/* Modal */}
        {selected && (
          <div className="fixed inset-0 z-50 flex items-center justify-center p-5" style={{ background: 'rgba(0,0,0,0.35)' }}
            onClick={e => { if (e.target === e.currentTarget) setSelected(null) }}>
            <div className="bg-bg-surface rounded-2xl border border-hairline w-[700px] max-w-[95vw] max-h-[90vh] flex flex-col shadow-2xl overflow-hidden">
              <div className="px-5 py-4 border-b border-hairline flex items-center justify-between flex-shrink-0">
                <span className="text-[15px] font-medium text-white">{selected.stock_name} ({selected.stock_code}) 龙虎榜明细</span>
                <button onClick={() => setSelected(null)} className="text-xl text-ink-muted hover:text-white px-2 py-0.5 rounded-md cursor-pointer">✕</button>
              </div>
              <div className="flex-1 overflow-y-auto p-5">
                <div className="mb-4">
                  <h4 className="text-[13px] font-medium text-white mb-2">上榜原因</h4>
                  <p className="text-xs text-ink-muted">{selected.reason}</p>
                </div>
                <div className="mb-4">
                  <h4 className="text-[13px] font-medium text-up mb-2">买入</h4>
                  <table className="w-full border-collapse text-xs">
                    <thead><tr><th style={th}>来源</th><th style={th}>买入金额(万)</th></tr></thead>
                    <tbody>
                      {selected.inst_buy_amount ? <tr><td style={td}>机构专用</td><td style={{ ...td, fontFamily: 'JetBrains Mono, monospace', color: 'var(--color-up)' }}>{selected.inst_buy_amount.toLocaleString()}</td></tr> : null}
                      {selected.dept_buy_amount ? <tr><td style={td}>营业部</td><td style={{ ...td, fontFamily: 'JetBrains Mono, monospace', color: 'var(--color-up)' }}>{selected.dept_buy_amount.toLocaleString()}</td></tr> : null}
                      {!selected.inst_buy_amount && !selected.dept_buy_amount && <tr><td colSpan={2} style={{ ...td, color: 'var(--color-muted, #888780)' }}>暂无数据</td></tr>}
                    </tbody>
                  </table>
                </div>
                <div className="mb-4">
                  <h4 className="text-[13px] font-medium text-down mb-2">卖出</h4>
                  <table className="w-full border-collapse text-xs">
                    <thead><tr><th style={th}>来源</th><th style={th}>卖出金额(万)</th></tr></thead>
                    <tbody>
                      {selected.inst_sell_amount ? <tr><td style={td}>机构专用</td><td style={{ ...td, fontFamily: 'JetBrains Mono, monospace', color: 'var(--color-down)' }}>{selected.inst_sell_amount.toLocaleString()}</td></tr> : null}
                      {selected.dept_sell_amount ? <tr><td style={td}>营业部</td><td style={{ ...td, fontFamily: 'JetBrains Mono, monospace', color: 'var(--color-down)' }}>{selected.dept_sell_amount.toLocaleString()}</td></tr> : null}
                      {!selected.inst_sell_amount && !selected.dept_sell_amount && <tr><td colSpan={2} style={{ ...td, color: 'var(--color-muted, #888780)' }}>暂无数据</td></tr>}
                    </tbody>
                  </table>
                </div>
                <div className="text-center text-sm font-semibold" style={{ color: selected.net_buy >= 0 ? 'var(--color-up)' : 'var(--color-down)' }}>
                  净买入：{selected.net_buy >= 0 ? '+' : ''}{(selected.net_buy / 10000).toFixed(2)}亿
                </div>
              </div>
            </div>
          </div>
        )}
      </div>
    </div>
  )
}
