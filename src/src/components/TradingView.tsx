import { useMemo } from "react"
import { AnimatePresence } from "framer-motion"
import type { IndexQuote } from "@/stores/data"
import { useOverview } from "@/hooks/query/useOverview"
import { useIndexQuotes } from "@/hooks/query/useIndexQuotes"
import { useRightPanelStore } from "@/stores/rightPanelStore"
import KpiCards from "@/components/KpiCards"
import SentimentHistory from "@/components/SentimentHistory"
import KlineDrawer from "@/components/KlineDrawer"

export default function TradingView() {
  const klineTarget = useRightPanelStore((s) => s.klineTarget)
  const closeKline = useRightPanelStore((s) => s.closeKline)
  const setActiveView = useRightPanelStore((s) => s.setActiveView)

  const {
    data: overview,
    isLoading: ovLoading,
    error: ovError,
    refetch: refetchOverview,
  } = useOverview()
  const { data: indexQuotes = [], isLoading: idxLoading, error: idxError } = useIndexQuotes()

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
    <div className="bg-base min-h-screen pt-14">
      <div className="mx-auto max-w-data px-6 py-4">
        {/* 错误状态 */}
        {ovError && (
          <div className="py-4">
            <div className="bg-up/5 border-up/20 rounded-lg flex items-center justify-between border px-5 py-3">
              <div className="flex items-center gap-3">
                <span className="text-up text-body">⚠</span>
                <span className="text-caption text-up">{ovError.message}</span>
              </div>
              <button
                onClick={() => refetchOverview()}
                className="text-caption text-up cursor-pointer underline"
              >
                重试
              </button>
            </div>
          </div>
        )}

        {/* 加载状态 */}
        {ovLoading && !overview && (
          <div className="flex flex-col items-center justify-center gap-4 py-32">
            <div className="bg-brand size-3 animate-pulse rounded-full" />
            <span className="text-caption text-secondary font-number">加载市场数据...</span>
          </div>
        )}

        {overview && (
          <>
            {/* 指数加载状态 */}
            {idxLoading && !indexQuotes.length && (
              <div className="bg-surface border-hairline p-card-inner rounded-lg flex items-center justify-center border py-6">
                <span className="text-caption text-secondary font-number">加载指数数据...</span>
              </div>
            )}

            {/* 指数错误状态 */}
            {!idxLoading && idxError && (
              <div className="bg-surface border-hairline p-card-inner rounded-lg flex items-center justify-center border py-6">
                <span className="text-caption text-up">{idxError.message}</span>
              </div>
            )}

            {indexCards.length > 0 && (
              <div className="mb-6 grid grid-cols-3 gap-4">
                {indexCards.map((idx) => (
                  <div
                    key={idx.name}
                    className="bg-surface border-hairline p-card-inner rounded-lg flex flex-col gap-1 border"
                  >
                    <span className="text-caption text-secondary font-number">{idx.name}</span>
                    <span className="font-number text-h1 text-primary tabular-nums">
                      {idx.lastPx.toFixed(2)}
                    </span>
                    <span
                      className={`text-caption font-number tabular-nums ${idx.isUp ? "text-up" : "text-down"}`}
                    >
                      {idx.isUp ? "+" : ""}
                      {idx.pxChangeRate.toFixed(2)}%
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

            <div className="mt-6">
              <SentimentHistory />
            </div>

            {/* 快捷入口 */}
            <div className="mt-6 flex gap-3">
              {[
                { label: "强势股", view: "strong-stocks" as const },
                { label: "晋级分析", view: "promotion" as const },
                { label: "板块异动", view: "plate" as const },
                { label: "时间线", view: "timeline" as const },
                { label: "市场情报", view: "market-intel" as const },
              ].map((item) => (
                <button
                  key={item.view}
                  onClick={() => setActiveView(item.view)}
                  className="bg-surface border-hairline hover:border-brand/40 rounded-lg border px-4 py-2 text-caption text-primary transition-colors"
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
          <KlineDrawer symbol={klineTarget.symbol} name={klineTarget.name} onClose={closeKline} />
        )}
      </AnimatePresence>
    </div>
  )
}
