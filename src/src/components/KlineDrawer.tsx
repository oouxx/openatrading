import { useState, useEffect, useMemo } from 'react'
import { normalizeSymbol, getKlineImageUrl } from '@/lib/stock-utils'
import { useRealtimeQuote } from '@/hooks/query/useRealtimeQuote'
import { usePlatesByStock } from '@/hooks/query/usePlatesByStock'

interface KlineDrawerProps {
  symbol: string
  name: string
  onClose: () => void
}

const periods = [
  { label: '分时', value: 'minute' },
  { label: '日K', value: 'day' },
  { label: '周K', value: 'week' },
  { label: '月K', value: 'month' },
]

const FALLBACK_IMG = 'data:image/svg+xml,' + encodeURIComponent('<svg xmlns="http://www.w3.org/2000/svg" width="400" height="300"><rect fill="#1a1a2e" width="400" height="300"/><text x="50%" y="50%" dominant-baseline="middle" text-anchor="middle" fill="#666" font-size="14">K线图加载失败</text></svg>')

interface KlineDrawerProps {
  symbol: string
  name: string
  onClose: () => void
}

const periods = [
  { label: '分时', value: 'minute' },
  { label: '日K', value: 'day' },
  { label: '周K', value: 'week' },
  { label: '月K', value: 'month' },
]

function formatVolume(v: number) {
  if (!v) return '-'
  if (v > 1e8) return (v / 1e8).toFixed(2) + '亿'
  if (v > 1e4) return (v / 1e4).toFixed(2) + '万'
  return v.toFixed(0)
}

function toXuangubaoSymbol(code: string): string {
  const clean = code.replace(/[.].*$/, '')
  if (clean.startsWith('6')) return clean + '.SS'
  if (clean.startsWith('0') || clean.startsWith('3')) return clean + '.SZ'
  if (clean.startsWith('4') || clean.startsWith('8')) return clean + '.BJ'
  return clean
}

export default function KlineDrawer({ symbol, name, onClose }: KlineDrawerProps) {
  const [period, setPeriod] = useState('minute')
  const xgSymbol = toXuangubaoSymbol(symbol)
  const { data: quote } = useRealtimeQuote(xgSymbol)
  const { data: plates } = usePlatesByStock(xgSymbol)
  const plateList = Array.isArray(plates) ? plates : []
  const [dialogPlate, setDialogPlate] = useState<{ plate_id: number; plate_name: string; core_avg_pcp: number; reason: string } | null>(null)

  const metrics = useMemo(() => {
    if (!quote) return []
    return [
      { label: '今开', value: quote.open_px?.toFixed(2) || '-' },
      { label: '昨收', value: quote.preclose_px?.toFixed(2) || '-' },
      { label: '最高', value: quote.high_px?.toFixed(2) || '-' },
      { label: '最低', value: quote.low_px?.toFixed(2) || '-' },
      { label: '振幅', value: quote.amplitude != null ? quote.amplitude.toFixed(2) + '%' : '-' },
      { label: '换手率', value: quote.turnover_ratio != null ? quote.turnover_ratio.toFixed(2) + '%' : '-' },
      { label: '动态PE', value: quote.dyn_pe ? quote.dyn_pe.toFixed(1) : '-' },
      { label: '市净率', value: quote.dyn_pb_rate ? quote.dyn_pb_rate.toFixed(2) : '-' },
      { label: '总市值', value: formatVolume(quote.market_value) },
      { label: '流通市值', value: formatVolume(quote.circulation_value) },
      { label: '量比', value: quote.volume_ratio?.toFixed(2) || '-' },
      { label: '成交量', value: formatVolume(quote.turnover_volume) + '股' },
    ]
  }, [quote])

  useEffect(() => {
    const handler = (e: KeyboardEvent) => {
      if (e.key === 'Escape') onClose()
    }
    document.addEventListener('keydown', handler)
    return () => document.removeEventListener('keydown', handler)
  }, [onClose])

  const klineUrl = getKlineImageUrl(symbol, period)
  const [imgError, setImgError] = useState(false)

  useEffect(() => { setImgError(false); }, [klineUrl])

  const up = quote && quote.px_change_rate >= 0
  const changeClass = up ? 'text-up' : 'text-down'

  return (
    <div className="h-full overflow-y-auto bg-bg">
      {/* Header */}
      <div className="flex items-center gap-3 px-6 py-4 border-b border-hairline bg-bg-surface flex-shrink-0">
        <span className="text-xl font-bold text-white">{name}</span>
        <span className="text-xs text-ink-muted font-mono">{symbol}</span>
        {quote && (
          <div className="ml-auto flex items-center gap-3">
            <span className="text-[22px] font-bold font-mono">{quote.last_px?.toFixed(2)}</span>
            <span className={`text-sm font-mono font-semibold px-2.5 py-1 rounded-card ${up ? 'bg-up/20 text-up' : 'bg-down/20 text-down'}`}>
              {up ? '+' : ''}{quote.px_change_rate?.toFixed(2)}%
            </span>
          </div>
        )}
      </div>

      {/* Period selector */}
      <div className="flex gap-1.5 px-6 py-2.5 border-b border-hairline bg-bg-surface">
        {periods.map((p) => (
          <button
            key={p.value}
            onClick={() => setPeriod(p.value)}
            className={`px-3.5 py-1 text-xs rounded-full border transition-colors ${
              period === p.value
                ? 'bg-ai text-white border-ai'
                : 'bg-bg text-ink-muted border-hairline hover:border-hairline-dark hover:text-white'
            }`}
          >
            {p.label}
          </button>
        ))}
      </div>

      {/* Chart */}
      <div className="px-6 py-2.5">
        <img
          src={imgError ? FALLBACK_IMG : klineUrl}
          alt={`${name} K线图`}
          className="w-full rounded-card"
          style={{ minHeight: 300, maxHeight: 420 }}
          onError={() => setImgError(true)}
        />
      </div>

      {/* Metrics */}
      {quote && (
        <div className="px-6 py-2.5">
          <div className="grid grid-cols-4 gap-2 text-xs">
            {metrics.map((m) => (
              <div key={m.label} className="bg-bg-surface border border-hairline rounded-card p-2.5 text-center">
                <p className="text-ink-muted text-[10px] uppercase tracking-wide mb-1">{m.label}</p>
                <p className="font-mono text-white text-xs">{m.value}</p>
              </div>
            ))}
          </div>
        </div>
      )}

      {/* Plates */}
      {plateList.length > 0 && (
        <div className="px-6 py-2.5 border-t border-hairline mt-2">
          <p className="text-xs text-ink-muted mb-2">所属板块</p>
          <div className="grid grid-cols-2 gap-2 max-h-[200px] overflow-y-auto">
            {plateList.map((p) => (
              <div
                key={p.plate_id}
                className="bg-bg-surface rounded-card border border-hairline p-2.5 cursor-pointer hover:border-white/20 transition-colors"
                onClick={() => p.reason && setDialogPlate(p)}
              >
                <div className="flex items-center justify-between gap-2">
                  <span className="text-xs font-medium text-white truncate">{p.plate_name}</span>
                  <span className={`text-xs font-mono shrink-0 ${p.core_avg_pcp >= 0 ? 'text-up' : 'text-down'}`}>
                    {p.core_avg_pcp >= 0 ? '+' : ''}{(p.core_avg_pcp * 100).toFixed(2)}%
                  </span>
                </div>
                {p.reason && <p className="text-[11px] text-ink-muted mt-1 leading-relaxed line-clamp-2">{p.reason}</p>}
              </div>
            ))}
          </div>
        </div>
      )}

      {/* Plate dialog */}
      {dialogPlate && (
        <>
          <div className="fixed inset-0 z-[60] bg-black/50" onClick={() => setDialogPlate(null)} />
          <div className="fixed inset-0 z-[61] flex items-center justify-center pointer-events-none">
            <div className="pointer-events-auto bg-bg-surface border border-hairline rounded-card p-5 w-[420px] max-w-[90vw] shadow-2xl">
              <div className="flex items-center justify-between mb-3">
                <div className="flex items-center gap-2">
                  <span className="text-sm font-bold text-white">{dialogPlate.plate_name}</span>
                  <span className={`text-xs font-mono ${dialogPlate.core_avg_pcp >= 0 ? 'text-up' : 'text-down'}`}>
                    {dialogPlate.core_avg_pcp >= 0 ? '+' : ''}{(dialogPlate.core_avg_pcp * 100).toFixed(2)}%
                  </span>
                </div>
                <button onClick={() => setDialogPlate(null)} className="text-lg leading-none p-1 text-ink-muted hover:text-white">&times;</button>
              </div>
              <p className="text-[13px] text-white/70 leading-relaxed">{dialogPlate.reason}</p>
            </div>
          </div>
        </>
      )}
    </div>
  )
}
