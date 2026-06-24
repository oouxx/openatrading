import { useMemo } from 'react'
import { AnimatePresence, motion } from 'framer-motion'
import type { IndexQuote } from '@/stores/data'
import { useOverview } from '@/hooks/query/useOverview'
import { useIndexQuotes } from '@/hooks/query/useIndexQuotes'
import { useRightPanelStore } from '@/stores/rightPanelStore'
import KpiCards from '@/components/KpiCards'
import KlineDrawer from '@/components/KlineDrawer'

export default function TradingView() {
  const klineTarget = useRightPanelStore((s) => s.klineTarget)
  const closeKline = useRightPanelStore((s) => s.closeKline)
  const setActiveView = useRightPanelStore((s) => s.setActiveView)

  const { data: overview, isLoading: ovLoading, error: ovError, refetch: refetchOverview } = useOverview()
  const { data: indexQuotes = [], isLoading: idxLoading, error: idxError } = useIndexQuotes()
  const refreshData = useMemo(() => refetchOverview, [refetchOverview])

  const indexCards = useMemo(() => {
    if (!indexQuotes.length) return []
    return indexQuotes.map((q: IndexQuote) => ({
      name: q.name,
      lastPx: q.last_px,
      pxChangeRate: q.px_change_rate,
      isUp: q.px_change_rate >= 0,
    }))
  }, [indexQuotes])

  return (
    <div className="min-h-screen bg-bg pt-14">
      <div className="max-w-[1600px] mx-auto px-6 py-4">
        {/* 错误状态 */}
        {ovError && (
          <div className="py-4">
            <div className="bg-up/5 border border-up/20 rounded-card px-5 py-3 flex items-center justify-between">
              <div className="flex items-center gap-3">
                <span className="text-up text-sm">⚠</span>
                <span className="text-caption text-up">{ovError.message}</span>
              </div>
              <button
                onClick={refreshData}
                className="text-caption text-up underline cursor-pointer"
              >
                重试
              </button>
            </div>
          </div>
        )}

        {/* 加载状态 */}
        {ovLoading && !overview && (
          <div className="flex flex-col items-center justify-center py-32 gap-4">
            <div className="w-3 h-3 rounded-full bg-ai animate-pulse" />
            <span className="text-caption font-mono text-ink-muted">加载市场数据...</span>
          </div>
        )}

        {overview && (
          <>
            {/* 指数加载状态 */}
            {idxLoading && !indexQuotes.length && (
              <div className="bg-bg-surface rounded-card border border-hairline p-card-inner flex items-center justify-center py-6">
                <span className="text-caption font-mono text-ink-muted">加载指数数据...</span>
              </div>
            )}

            {/* 指数错误状态 */}
            {!idxLoading && idxError && (
              <div className="bg-bg-surface rounded-card border border-hairline p-card-inner flex items-center justify-center py-6">
                <span className="text-caption text-up">{idxError.message}</span>
              </div>
            )}

            {indexCards.length > 0 && (
              <div className="grid grid-cols-3 gap-4 mb-6">
                {indexCards.map((idx) => (
                  <div key={idx.name} className="bg-bg-surface rounded-card border border-hairline p-card-inner flex flex-col gap-1">
                    <span className="text-caption text-ink-muted font-mono">{idx.name}</span>
                    <span className="text-xl font-mono text-white tabular-nums">{idx.lastPx.toFixed(2)}</span>
                    <span className={`text-caption font-mono tabular-nums ${idx.isUp ? 'text-up' : 'text-down'}`}>
                      {idx.isUp ? '+' : ''}{idx.pxChangeRate.toFixed(2)}%
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

            {/* 快捷入口 */}
            <div className="mt-6 flex gap-3">
              {[
                { label: '强势股', view: 'strong-stocks' as const },
                { label: '晋级分析', view: 'promotion' as const },
                { label: '板块异动', view: 'plate' as const },
                { label: '时间线', view: 'timeline' as const },
                { label: '市场情报', view: 'market-intel' as const },
              ].map((item) => (
                <button
                  key={item.view}
                  onClick={() => setActiveView(item.view)}
                  className="px-4 py-2 text-xs bg-bg-surface border border-hairline rounded-card hover:border-ai/40 transition-colors text-white"
                >
                  {item.label} →
                </button>
              ))}
            </div>
          </>
        )}
      </div>

      <AnimatePresence>
        {klineTarget && (
          <KlineDrawer
            symbol={klineTarget.symbol}
            name={klineTarget.name}
            onClose={closeKline}
          />
        )}
      </AnimatePresence>
    </div>
  )
}
