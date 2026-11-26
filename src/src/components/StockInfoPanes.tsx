import { useEffect, useMemo, useState } from "react"
import { ActiveStockHeader } from "@/components/ActiveStockHeader"
import { Dialog, DialogContent, DialogDescription, DialogTitle } from "@/components/ui/dialog"
import { subscribeRealtime, type RealtimeQuote } from "@/hooks/useRealtimeStream"
import { usePlatesByStock, type PlateByStockItem } from "@/hooks/query/usePlatesByStock"
import { usePaneStock } from "@/hooks/usePaneStock"
import { formatCompactNumber, formatRatioPct, pctClass } from "@/lib/format"
import { classifySymbol, toUpstreamSymbol } from "@/lib/stock-utils"
import type { ActiveStock } from "@/stores/activeStockStore"

/**
 * 「关键指标」「所属板块」两个网格格子。
 *
 * 它们以前是 K 线详情底部的两段内容：K 线一格塞了三件事（图 + 12 个指标 + 板块
 * 卡片），想看指标就得先有 K 线格，窄格里还会挤成一坨。拆出来之后各自是一格，
 * 跟「五档行情」「下单」一样跟随工作区当前标的，摆在哪、给多大由布局决定。
 */

/** 交易时段内的快照。切换标的时先清空，避免短暂显示上一只票的数字。 */
function useQuote(stock: ActiveStock | null): {
  quote: RealtimeQuote | null
  upstream: string
  kind: SymbolKindName
  hasStock: boolean
} {
  const upstream = stock ? toUpstreamSymbol(stock.code) : ""
  const kind = stock ? classifySymbol(upstream) : "stock"
  const [quote, setQuote] = useState<RealtimeQuote | null>(null)

  useEffect(() => {
    // eslint-disable-next-line react-hooks/set-state-in-effect -- 换标的先清掉上一只的报价
    setQuote(null)
    if (!upstream) return
    return subscribeRealtime<RealtimeQuote>(upstream, setQuote, (err) =>
      console.warn("[realtime-stream]", err)
    )
  }, [upstream])

  return { quote, upstream, kind, hasStock: !!stock }
}

type SymbolKindName = ReturnType<typeof classifySymbol>

/** 个股与指数的指标集不同：指数量价齐全，但 PE / 市净率 / 市值上游恒 0。 */
const TRADEABLE_LABELS = new Set([
  "今开",
  "昨收",
  "最高",
  "最低",
  "振幅",
  "换手率",
  "量比",
  "成交量",
])

function metricsOf(quote: RealtimeQuote, isStock: boolean) {
  const rows = [
    { label: "今开", value: quote.open_px?.toFixed(2) || "-" },
    { label: "昨收", value: quote.preclose_px?.toFixed(2) || "-" },
    { label: "最高", value: quote.high_px?.toFixed(2) || "-" },
    { label: "最低", value: quote.low_px?.toFixed(2) || "-" },
    { label: "振幅", value: quote.amplitude != null ? quote.amplitude.toFixed(2) + "%" : "-" },
    { label: "换手率", value: quote.turnover_ratio ? quote.turnover_ratio.toFixed(2) + "%" : "-" },
    { label: "动态PE", value: quote.dyn_pe ? quote.dyn_pe.toFixed(1) : "-" },
    { label: "市净率", value: quote.dyn_pb_rate ? quote.dyn_pb_rate.toFixed(2) : "-" },
    { label: "总市值", value: formatCompactNumber(quote.market_value) },
    { label: "流通市值", value: formatCompactNumber(quote.circulation_value) },
    { label: "量比", value: quote.volume_ratio?.toFixed(2) || "-" },
    {
      label: "成交量",
      value:
        quote.turnover_volume != null ? `${formatCompactNumber(quote.turnover_volume)}股` : "-",
    },
  ]
  // 非个股留着 PE / 市净率 / 市值只会是几格 "0" / "-"，去掉后剩 8 格正好四行
  return isStock ? rows : rows.filter((row) => TRADEABLE_LABELS.has(row.label))
}

export function QuoteMetricsPane({
  layoutId,
  leafId,
  symbol,
  symbolName,
}: {
  layoutId: string
  leafId: string
  symbol?: string
  symbolName?: string
}) {
  const { stock, select } = usePaneStock({ layoutId, leafId, symbol, symbolName })
  const { quote, kind, hasStock } = useQuote(stock)

  return (
    <div className="bg-surface @container flex h-full min-w-0 flex-col overflow-hidden">
      <ActiveStockHeader stock={stock} onPick={select} />
      <div className="min-h-0 flex-1 overflow-y-auto p-2">
        {!hasStock ? (
          // 没有标的时不能一直挂骨架屏：那读起来像"正在加载"
          <p className="text-tertiary text-caption px-1 py-2">在自选/榜单里点一只股票。</p>
        ) : !quote ? (
          <div className="flex flex-col gap-1.5">
            {Array.from({ length: 4 }).map((_, i) => (
              <div key={i} className="bg-hover/60 h-8 rounded" />
            ))}
          </div>
        ) : (
          <div className="text-caption grid grid-cols-2 gap-1.5 @md:grid-cols-3">
            {metricsOf(quote, kind === "stock").map((m) => (
              <div
                key={m.label}
                className="bg-canvas border-hairline rounded-md border px-2.5 py-2"
              >
                <p className="text-tertiary text-micro mb-1">{m.label}</p>
                <p className="font-number text-caption text-primary">{m.value}</p>
              </div>
            ))}
          </div>
        )}
      </div>
    </div>
  )
}

export function SectorsPane({
  layoutId,
  leafId,
  symbol,
  symbolName,
}: {
  layoutId: string
  leafId: string
  symbol?: string
  symbolName?: string
}) {
  const { stock, select } = usePaneStock({ layoutId, leafId, symbol, symbolName })
  const upstream = stock ? toUpstreamSymbol(stock.code) : ""
  const isStock = stock != null && classifySymbol(upstream) === "stock"
  // 指数 / 基金没有所属板块：不请求，直接说明
  const { data, isLoading } = usePlatesByStock(isStock ? upstream : "")
  const plates = useMemo<PlateByStockItem[]>(() => (Array.isArray(data) ? data : []), [data])
  const [detail, setDetail] = useState<PlateByStockItem | null>(null)

  const empty = !stock
    ? "在自选/榜单里点一只股票。"
    : !isStock
      ? "指数 / 基金没有所属板块。"
      : isLoading && plates.length === 0
        ? null
        : plates.length === 0
          ? "没有找到这只股票的所属板块。"
          : null

  return (
    <div className="bg-surface @container flex h-full min-w-0 flex-col overflow-hidden">
      <ActiveStockHeader stock={stock} onPick={select} />
      <div className="min-h-0 flex-1 overflow-y-auto p-2">
        {empty ? (
          <p className="text-tertiary text-caption px-1 py-2">{empty}</p>
        ) : isLoading && plates.length === 0 ? (
          <div className="flex flex-col gap-1.5">
            {Array.from({ length: 3 }).map((_, i) => (
              <div key={i} className="bg-hover/60 h-14 rounded" />
            ))}
          </div>
        ) : (
          <div className="grid grid-cols-1 gap-1.5 @md:grid-cols-2">
            {plates.map((p) => (
              <button
                key={p.plate_id}
                type="button"
                disabled={!p.reason}
                className={`bg-canvas border-hairline w-full rounded-md border p-2.5 text-left transition-colors ${
                  p.reason ? "hover:border-brand/30 cursor-pointer" : "cursor-default"
                }`}
                onClick={() => p.reason && setDetail(p)}
              >
                <div className="flex items-center justify-between gap-2">
                  <span className="text-caption text-primary truncate font-medium">
                    {p.plate_name}
                  </span>
                  <span className={`font-number text-caption shrink-0 ${pctClass(p.core_avg_pcp)}`}>
                    {formatRatioPct(p.core_avg_pcp)}
                  </span>
                </div>
                {p.reason && (
                  <p className="text-caption text-secondary mt-1 line-clamp-2 leading-relaxed">
                    {p.reason}
                  </p>
                )}
              </button>
            ))}
          </div>
        )}
      </div>

      {detail && (
        <Dialog
          open
          onOpenChange={(open) => {
            if (!open) setDetail(null)
          }}
        >
          <DialogContent className="bg-surface border-hairline shadow-modal w-[420px] max-w-[90vw] gap-3 rounded-lg border p-5 sm:max-w-[90vw]">
            <div className="flex items-center gap-2 pr-8">
              <DialogTitle className="text-body text-primary font-semibold">
                {detail.plate_name}
              </DialogTitle>
              <span className={`font-number text-caption ${pctClass(detail.core_avg_pcp)}`}>
                {formatRatioPct(detail.core_avg_pcp)}
              </span>
            </div>
            <DialogDescription className="sr-only">板块详情</DialogDescription>
            <p className="text-body text-secondary leading-relaxed">{detail.reason}</p>
          </DialogContent>
        </Dialog>
      )}
    </div>
  )
}
