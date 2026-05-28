import { useState, useEffect, useCallback, useMemo } from 'react'
import { AnimatePresence, motion } from 'framer-motion'
import { useDataStore, type Stock, type FieldPlate, type IndexQuote } from '@/stores/data'
import KpiCards from '@/components/KpiCards'
import TabBar from '@/components/TabBar'
import NewsFeed from '@/components/NewsFeed'
import AiSidebar from '@/components/AiSidebar'
import SentimentHistory from '@/components/SentimentHistory'
import MiniChartPreview from '@/components/MiniChartPreview'
import KlineDrawer from '@/components/KlineDrawer'

function formatCap(v: number) {
  if (v >= 1e12) return (v / 1e12).toFixed(1) + '万亿'
  if (v >= 1e8) return (v / 1e8).toFixed(1) + '亿'
  if (v >= 1e4) return (v / 1e4).toFixed(1) + '万'
  return String(v)
}

function formatPct(v: number) {
  return (v >= 0 ? '+' : '') + (v * 100).toFixed(2) + '%'
}

function pctClass(v: number) {
  return v >= 0 ? 'text-up' : 'text-down'
}

export default function TradingView() {
  const store = useDataStore()
  const [activeTab, setActiveTab] = useState('strong-stocks')
  const [showAi, setShowAi] = useState(false)
  const [klineTarget, setKlineTarget] = useState<{ symbol: string; name: string } | null>(null)

  useEffect(() => {
    store.fetchOverview()
    store.fetchLimitUp()
    store.fetchFieldItems()
    store.fetchIndexQuotes()
    store.fetchSentimentHistory()
  }, [])

  const overview = store.overview.data
  const limitUpStocks = store.limitUp.data
  const fieldItems = store.fieldItems.data
  const indexQuotes = store.indexQuotes.data
  const sentimentHistory = store.sentimentHistory.data

  const sentimentChartData = useMemo(
    () =>
      sentimentHistory.map((d) => ({
        date: d.date,
        value: d.sentiment,
      })),
    [sentimentHistory],
  )

  const indexCards = useMemo(() => {
    if (!indexQuotes.length) return []
    return indexQuotes.map((q: IndexQuote) => ({
      name: q.name,
      value: q.last_px?.toFixed(2) || '--',
      change: q.px_change?.toFixed(2) || '--',
      changeRate: q.px_change_rate,
      isUp: q.px_change_rate >= 0,
    }))
  }, [indexQuotes])

  const openKline = useCallback((symbol: string, name: string) => {
    setKlineTarget({ symbol, name })
  }, [])

  return (
    <div className="min-h-screen bg-bg pt-14">
      <div className="max-w-[1600px] mx-auto px-6 py-4">
        {indexCards.length > 0 && (
          <div className="flex gap-4 mb-6">
            {indexCards.map((idx) => (
              <div key={idx.name} className="bg-bg-surface rounded-card border border-hairline px-4 py-2 flex items-center gap-3">
                <span className="text-caption text-ink-muted">{idx.name}</span>
                <span className={`font-mono text-sm font-semibold ${idx.isUp ? 'text-up' : 'text-down'}`}>{idx.value}</span>
                <span className={`font-mono text-caption ${idx.isUp ? 'text-up' : 'text-down'}`}>
                  {idx.isUp ? '+' : ''}{idx.change} ({idx.isUp ? '+' : ''}{(idx.changeRate * 100).toFixed(2)}%)
                </span>
              </div>
            ))}
          </div>
        )}

        {overview && (
          <KpiCards
            overview={overview}
            riskLevel={overview.risk_level}
            decisionSummary={overview.decision_summary}
          />
        )}

        <TabBar activeTab={activeTab} onTabChange={setActiveTab} />

        <div className="flex gap-6">
          <div className="flex-1 min-w-0">
            {activeTab === 'strong-stocks' && (
              <div className="bg-bg-surface rounded-card border border-hairline overflow-hidden">
                <div className="px-4 py-3 border-b border-hairline flex items-center justify-between">
                  <span className="text-caption-strong text-ink-muted font-mono uppercase tracking-wider">
                    涨停股 · {limitUpStocks.length}
                  </span>
                </div>
                <div className="overflow-x-auto">
                  <table className="data-table">
                    <thead>
                      <tr>
                        <th>代码</th>
                        <th>名称</th>
                        <th>涨幅</th>
                        <th>连板</th>
                        <th>封板次数</th>
                        <th>换手率</th>
                        <th>市值</th>
                        <th>原因</th>
                      </tr>
                    </thead>
                    <tbody>
                      {limitUpStocks.map((s: Stock) => (
                        <tr key={s.symbol} className="cursor-pointer" onClick={() => openKline(s.symbol, s.stock_chi_name)}>
                          <td className="text-ink-muted">{s.symbol.replace(/\.[A-Z]+$/, '')}</td>
                          <td>
                            <MiniChartPreview symbol={s.symbol} name={s.stock_chi_name}>
                              <span className="text-white hover:text-ai transition-colors">{s.stock_chi_name}</span>
                            </MiniChartPreview>
                          </td>
                          <td className={pctClass(s.change_percent)}>{formatPct(s.change_percent)}</td>
                          <td className="text-ai font-semibold">{s.limit_up_days > 1 ? s.limit_up_days + '连板' : '-'}</td>
                          <td>{s.break_limit_up_times || '-'}</td>
                          <td>{s.turnover_ratio?.toFixed(2) + '%' || '-'}</td>
                          <td>{formatCap(s.total_capital)}</td>
                          <td className="max-w-[200px] truncate text-ink-muted text-xs">
                            {s.surge_reason?.stock_reason || '-'}
                          </td>
                        </tr>
                      ))}
                    </tbody>
                  </table>
                </div>
              </div>
            )}

            {activeTab === 'promotion' && (
              <PromotionTab stocks={limitUpStocks} onOpenKline={openKline} />
            )}

            {activeTab === 'plate' && (
              <PlateTab plates={fieldItems} onOpenKline={openKline} />
            )}

            {activeTab === 'timeline' && (
              <div className="bg-bg-surface rounded-card border border-hairline p-card-inner">
                <p className="text-caption text-ink-muted">时间线功能开发中...</p>
              </div>
            )}

            {activeTab === 'sentiment-chip' && (
              <div className="space-y-4">
                <SentimentHistory data={sentimentChartData} />
              </div>
            )}

            {activeTab === 'market-intel' && (
              <div className="space-y-4">
                <SentimentHistory data={sentimentChartData} />
                <NewsFeed variant="tab" />
              </div>
            )}

            {activeTab === 'news' && (
              <NewsFeed variant="tab" />
            )}
          </div>

          <div className="w-[340px] flex-shrink-0 hidden lg:block">
            <div className="bg-bg-surface rounded-card border border-hairline overflow-hidden sticky top-16" style={{ height: 'calc(100vh - 80px)' }}>
              <AiSidebar overview={overview} />
            </div>
          </div>
        </div>

        {!showAi && (
          <button
            onClick={() => setShowAi(true)}
            className="fixed bottom-6 right-6 z-30 lg:hidden w-12 h-12 bg-ai/20 text-ai rounded-full flex items-center justify-center shadow-lg hover:bg-ai/30 transition-colors"
          >
            AI
          </button>
        )}

        <AnimatePresence>
          {showAi && (
            <motion.div
              className="fixed inset-0 z-40 bg-bg/95 lg:hidden"
              initial={{ opacity: 0 }}
              animate={{ opacity: 1 }}
              exit={{ opacity: 0 }}
              transition={{ duration: 0.2 }}
            >
              <div className="h-full flex flex-col">
                <div className="flex items-center justify-between px-4 py-3 border-b border-hairline">
                  <span className="text-caption-strong text-ink-muted">AI 副驾驶</span>
                  <button onClick={() => setShowAi(false)} className="text-white/40 hover:text-white/60 text-lg">&times;</button>
                </div>
                <div className="flex-1 overflow-hidden">
                  <AiSidebar overview={overview} />
                </div>
              </div>
            </motion.div>
          )}
        </AnimatePresence>
      </div>

      <AnimatePresence>
        {klineTarget && (
          <KlineDrawer
            symbol={klineTarget.symbol}
            name={klineTarget.name}
            onClose={() => setKlineTarget(null)}
          />
        )}
      </AnimatePresence>
    </div>
  )
}

function PromotionTab({ stocks, onOpenKline }: { stocks: Stock[]; onOpenKline: (s: string, n: string) => void }) {
  const grouped = useMemo(() => {
    const map = new Map<number, Stock[]>()
    stocks.forEach((s) => {
      const days = s.limit_up_days || 1
      if (!map.has(days)) map.set(days, [])
      map.get(days)!.push(s)
    })
    return Array.from(map.entries()).sort((a, b) => b[0] - a[0])
  }, [stocks])

  return (
    <div className="space-y-4">
      {grouped.map(([days, list]) => (
        <div key={days} className="bg-bg-surface rounded-card border border-hairline overflow-hidden">
          <div className="px-4 py-3 border-b border-hairline flex items-center gap-2">
            <span className="text-ai font-semibold text-sm">{days}连板</span>
            <span className="text-caption text-ink-muted">{list.length}只</span>
          </div>
          <div className="p-4 grid grid-cols-2 md:grid-cols-3 lg:grid-cols-4 gap-3">
            {list.map((s) => (
              <div
                key={s.symbol}
                className="bg-bg rounded-card border border-hairline p-3 cursor-pointer hover:border-ai/30 transition-colors"
                onClick={() => onOpenKline(s.symbol, s.stock_chi_name)}
              >
                <div className="flex items-center justify-between mb-1">
                  <span className="text-white text-sm font-semibold">{s.stock_chi_name}</span>
                  <span className={`font-mono text-xs ${pctClass(s.change_percent)}`}>{formatPct(s.change_percent)}</span>
                </div>
                <span className="text-caption text-ink-muted font-mono">{s.symbol.replace(/\.[A-Z]+$/, '')}</span>
              </div>
            ))}
          </div>
        </div>
      ))}
    </div>
  )
}

function PlateTab({ plates, onOpenKline }: { plates: FieldPlate[]; onOpenKline: (s: string, n: string) => void }) {
  const [expanded, setExpanded] = useState<string | null>(null)

  return (
    <div className="space-y-3">
      {plates.map((plate) => (
        <div key={plate.name} className="bg-bg-surface rounded-card border border-hairline overflow-hidden">
          <div
            className="px-4 py-3 flex items-center justify-between cursor-pointer hover:bg-bg-hover transition-colors"
            onClick={() => setExpanded(expanded === plate.name ? null : plate.name)}
          >
            <div className="flex items-center gap-3">
              <span className="text-white text-sm font-semibold">{plate.name}</span>
              <span className="text-caption text-ink-muted">{plate.count}只</span>
            </div>
            <span className="text-caption text-ink-muted">{expanded === plate.name ? '▲' : '▼'}</span>
          </div>
          {plate.reason && (
            <div className="px-4 pb-2">
              <p className="text-caption text-ink-muted leading-relaxed">{plate.reason}</p>
            </div>
          )}
          {expanded === plate.name && plate.list.length > 0 && (
            <div className="border-t border-hairline">
              <div className="p-3 grid grid-cols-2 md:grid-cols-3 gap-2">
                {plate.list.map((item) => (
                  <div
                    key={item.code}
                    className="bg-bg rounded-card border border-hairline p-2 cursor-pointer hover:border-ai/30 transition-colors"
                    onClick={() => onOpenKline(item.code, item.name)}
                  >
                    <span className="text-white text-xs font-semibold">{item.name}</span>
                    <span className="text-caption text-ink-muted font-mono ml-2">{item.code.replace(/\.[A-Z]+$/, '')}</span>
                  </div>
                ))}
              </div>
            </div>
          )}
        </div>
      ))}
    </div>
  )
}