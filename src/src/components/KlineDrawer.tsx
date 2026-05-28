import { useState, useEffect, useMemo } from 'react'
import { motion, AnimatePresence } from 'framer-motion'
import { apiFetch } from '@/stores/data'

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

function normalizeSymbol(raw: string): string {
  const dot = raw.indexOf('.')
  if (dot !== -1) {
    const code = raw.substring(0, dot)
    let suffix = raw.substring(dot + 1).toLowerCase()
    if (suffix === 'ss') suffix = 'sh'
    return suffix + code
  }
  return (raw.startsWith('6') ? 'sh' : 'sz') + raw
}

function getKlineImageUrl(symbol: string, period: string): string {
  const s = normalizeSymbol(symbol)
  switch (period) {
    case 'minute': return `https://image.sinajs.cn/newchart/min/n/${s}.gif`
    case 'day': return `https://image.sinajs.cn/newchart/daily/n/${s}.gif`
    case 'week': return `https://image.sinajs.cn/newchart/weekly/n/${s}.gif`
    case 'month': return `https://image.sinajs.cn/newchart/monthly/n/${s}.gif`
    default: return `https://image.sinajs.cn/newchart/min/n/${s}.gif`
  }
}

export default function KlineDrawer({ symbol, name, onClose }: KlineDrawerProps) {
  const [period, setPeriod] = useState('minute')
  const [quote, setQuote] = useState<any>(null)
  const [plates, setPlates] = useState<Array<{ plate_id: number; plate_name: string; core_avg_pcp: number; reason: string }>>([])
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
    async function load() {
      try {
        const resp = await apiFetch(`/api/realtime/${symbol}`)
        if (resp.ok) setQuote(await resp.json())
      } catch {}
      try {
        const resp = await apiFetch(`/api/plates-by-any-stock/${symbol}`)
        if (resp.ok) setPlates(await resp.json())
      } catch {}
    }
    load()
  }, [symbol])

  useEffect(() => {
    const handleKey = (e: KeyboardEvent) => {
      if (e.key === 'Escape') onClose()
    }
    document.addEventListener('keydown', handleKey)
    return () => document.removeEventListener('keydown', handleKey)
  }, [onClose])

  const klineUrl = getKlineImageUrl(symbol, period)

  return (
    <>
      <div className="fixed inset-0 z-40" style={{ background: 'rgba(0,0,0,0.4)' }} onClick={onClose} />

      <motion.div
        className="fixed top-0 right-0 h-full z-50 overflow-y-auto bg-surface-dark border-l border-hairline-dark"
        style={{ width: 520 }}
        initial={{ x: 520 }}
        animate={{ x: 0 }}
        exit={{ x: 520 }}
        transition={{ duration: 0.3, ease: [0.32, 0.72, 0, 1] }}
      >
        <div className="sticky top-0 z-10 p-4 bg-surface-dark border-b border-hairline-dark">
          <div className="flex items-center justify-between mb-2">
            <div className="flex items-center gap-2">
              <span className="text-sm font-bold text-white">{name}</span>
              <span className="text-xs font-mono text-white/40">{symbol}</span>
            </div>
            <button onClick={onClose} className="text-lg leading-none p-1 text-white/40 hover:text-white/60">&times;</button>
          </div>
          {quote && (
            <div className="flex items-center gap-3">
              <span className={`text-xl font-bold font-mono ${quote.px_change_rate >= 0 ? 'text-up' : 'text-down'}`}>
                {quote.last_px?.toFixed(2)}
              </span>
              <span className={`text-sm font-mono ${quote.px_change_rate >= 0 ? 'text-up' : 'text-down'}`}>
                {quote.px_change_rate >= 0 ? '+' : ''}{quote.px_change_rate?.toFixed(2)}%
              </span>
              <span className="text-xs text-white/40">成交 {formatVolume(quote.turnover_value)}</span>
            </div>
          )}
        </div>

        <div className="flex gap-1 px-4 py-2 sticky z-10 bg-surface-dark border-b border-hairline-dark top-[97px]">
          {periods.map((p) => (
            <button
              key={p.value}
              onClick={() => setPeriod(p.value)}
              className={`px-3 py-1 text-xs font-mono transition rounded-button ${
                period === p.value ? 'bg-ai text-white' : 'bg-transparent text-white/40'
              }`}
            >
              {p.label}
            </button>
          ))}
        </div>

        <div className="p-4">
          <img
            src={klineUrl}
            alt={`${name} K线图`}
            className="w-full rounded-lg"
            style={{ minHeight: 300 }}
          />
        </div>

        {plates.length > 0 && (
          <div className="px-4 py-3 border-t border-hairline-dark">
            <p className="text-xs text-white/40 mb-2">所属板块</p>
            <div className="space-y-1.5 max-h-[240px] overflow-y-auto custom-scroll">
              {plates.map((p) => (
                <div
                  key={p.plate_id}
                  className="bg-surface-dark rounded-card border border-hairline-dark p-2.5 cursor-pointer hover:border-white/20 transition-colors"
                  onClick={() => p.reason && setDialogPlate(p)}
                >
                  <div className="flex items-center justify-between gap-2">
                    <span className="text-xs font-bold text-white truncate">{p.plate_name}</span>
                    <span className={`text-xs font-mono shrink-0 ${p.core_avg_pcp >= 0 ? 'text-up' : 'text-down'}`}>
                      {p.core_avg_pcp >= 0 ? '+' : ''}{(p.core_avg_pcp * 100).toFixed(2)}%
                    </span>
                  </div>
                  {p.reason && <p className="text-[11px] text-white/50 mt-1 leading-relaxed line-clamp-2">{p.reason}</p>}
                </div>
              ))}
            </div>
          </div>
        )}

        <AnimatePresence>
          {dialogPlate && (
            <>
              <div className="fixed inset-0 z-[60]" style={{ background: 'rgba(0,0,0,0.5)' }} onClick={() => setDialogPlate(null)} />
              <motion.div
                className="fixed inset-0 z-[61] flex items-center justify-center pointer-events-none"
                initial={{ opacity: 0, scale: 0.95 }}
                animate={{ opacity: 1, scale: 1 }}
                exit={{ opacity: 0, scale: 0.95 }}
                transition={{ duration: 0.2 }}
              >
                <div className="pointer-events-auto bg-surface-dark border border-hairline-dark rounded-card p-5 w-[420px] max-w-[90vw] shadow-2xl">
                  <div className="flex items-center justify-between mb-3">
                    <div className="flex items-center gap-2">
                      <span className="text-sm font-bold text-white">{dialogPlate.plate_name}</span>
                      <span className={`text-xs font-mono ${dialogPlate.core_avg_pcp >= 0 ? 'text-up' : 'text-down'}`}>
                        {dialogPlate.core_avg_pcp >= 0 ? '+' : ''}{(dialogPlate.core_avg_pcp * 100).toFixed(2)}%
                      </span>
                    </div>
                    <button onClick={() => setDialogPlate(null)} className="text-lg leading-none p-1 text-white/40 hover:text-white/60">&times;</button>
                  </div>
                  <p className="text-[13px] text-white/70 leading-relaxed">{dialogPlate.reason}</p>
                </div>
              </motion.div>
            </>
          )}
        </AnimatePresence>

        {quote && (
          <div className="p-4 grid grid-cols-3 gap-2 text-xs">
            {metrics.map((m) => (
              <div key={m.label} className="bg-surface-dark rounded-card border border-hairline-dark p-3 text-center">
                <p className="text-white/40 text-[11px]">{m.label}</p>
                <p className="font-mono mt-1 text-white text-xs">{m.value}</p>
              </div>
            ))}
          </div>
        )}
      </motion.div>
    </>
  )
}