import { useState, useEffect, useMemo, useRef, useCallback } from "react"
import { subscribeKline, subscribeRealtime, type RealtimeQuote } from "@/hooks/useRealtimeStream"
import { usePlatesByStock, type PlateByStockItem } from "@/hooks/query/usePlatesByStock"
import { useFieldItems } from "@/hooks/query/useFieldItems"
import KlineChart, { type KlineCandle } from "./KlineChart"
import { getKlineChartStatics, type CrosshairInfo } from "@/lib/klineChartStatics"
import { formatCompactNumber, formatRatioPct, pctClass } from "@/lib/format"
import { useWatchlist, useAddWatchlist, useDeleteWatchlist } from "@/hooks/query/useWatchlist"
import { toYahooSymbol } from "@/lib/stock-utils"
import { Star } from "lucide-react"
import { Button } from "@/components/ui/button"
import { Dialog, DialogContent, DialogDescription, DialogTitle } from "@/components/ui/dialog"

interface KlineDrawerProps {
  symbol: string
  name: string
  onClose: () => void
}

const periods = [
  { label: "分时", value: "minute" },
  { label: "日K", value: "day" },
  { label: "周K", value: "week" },
  { label: "月K", value: "month" },
]

/** 归一化为 Yahoo 格式；失败时回退原始输入，与后端 watchlist 容错一致。 */
function canonicalWatchSymbol(raw: string): string {
  try {
    return toYahooSymbol(raw)
  } catch {
    return raw
  }
}

function toXuangubaoSymbol(code: string): string {
  const clean = code.replace(/[.].*$/, "")
  if (clean.startsWith("6")) return clean + ".SS"
  if (clean.startsWith("0") || clean.startsWith("3")) return clean + ".SZ"
  if (clean.startsWith("4") || clean.startsWith("8")) return clean + ".BJ"
  return clean
}

export default function KlineDrawer({ symbol, name, onClose }: KlineDrawerProps) {
  const [period, setPeriod] = useState("minute")
  const xgSymbol = toXuangubaoSymbol(symbol)
  const [quote, setQuote] = useState<RealtimeQuote | null>(null)

  // Watchlist
  const { data: watchlist = [] } = useWatchlist()
  const addWatchlist = useAddWatchlist()
  const deleteWatchlist = useDeleteWatchlist()
  // 自选股统一以 Yahoo 格式存储，比较/增删前先归一化，避免同一股票重复
  const watchSymbol = canonicalWatchSymbol(symbol)
  const inWatchlist = watchlist.some((w) => w.code === watchSymbol)

  // Kline data from SSE
  const [klineData, setKlineData] = useState<KlineCandle[]>([])
  const [crosshair, setCrosshair] = useState<CrosshairInfo | null>(null)
  const crosshairRef = useRef<CrosshairInfo | null>(null)

  const setChartCrosshair = useCallback((info: CrosshairInfo) => {
    crosshairRef.current = info
    setCrosshair(info)
  }, [])

  // Wire crosshair listener from KlineChart (re-wire on klineData change since chart is recreated)
  useEffect(() => {
    // Set pending crosshair listener so KlineChart can pick it up on remount
    const statics = getKlineChartStatics()
    statics.__pendingCrosshairListener = setChartCrosshair
    statics.setCrosshairListener?.(setChartCrosshair)
  }, [setChartCrosshair, klineData])

  useEffect(() => {
    return subscribeRealtime<RealtimeQuote>(xgSymbol, setQuote, (err) =>
      console.warn("[realtime-stream]", err)
    )
  }, [xgSymbol])

  // Subscribe to kline SSE stream
  useEffect(() => {
    // eslint-disable-next-line react-hooks/set-state-in-effect -- 切换股票/周期时先清空上一份数据
    setKlineData([])
    crosshairRef.current = null
    setCrosshair(null)
    return subscribeKline<{ candles?: KlineCandle[] }>(xgSymbol, period, 256, (data) => {
      if (data.candles) {
        setKlineData(data.candles)
      }
    })
  }, [xgSymbol, period])
  const { data: plates } = usePlatesByStock(xgSymbol)
  const { data: fieldItems } = useFieldItems()
  const expound = useMemo(() => {
    if (!fieldItems) return undefined
    for (const plate of fieldItems) {
      for (const item of plate.list) {
        if (item.code === symbol && item.article?.action_info?.expound) {
          return item.article.action_info.expound
        }
      }
    }
    return undefined
  }, [fieldItems, symbol])
  const plateList = Array.isArray(plates) ? plates : []
  const [dialogPlate, setDialogPlate] = useState<PlateByStockItem | null>(null)

  const metrics = useMemo(() => {
    if (!quote) return []
    return [
      { label: "今开", value: quote.open_px?.toFixed(2) || "-" },
      { label: "昨收", value: quote.preclose_px?.toFixed(2) || "-" },
      { label: "最高", value: quote.high_px?.toFixed(2) || "-" },
      { label: "最低", value: quote.low_px?.toFixed(2) || "-" },
      { label: "振幅", value: quote.amplitude != null ? quote.amplitude.toFixed(2) + "%" : "-" },
      {
        label: "换手率",
        value: quote.turnover_ratio != null ? quote.turnover_ratio.toFixed(2) + "%" : "-",
      },
      { label: "动态PE", value: quote.dyn_pe ? quote.dyn_pe.toFixed(1) : "-" },
      { label: "市净率", value: quote.dyn_pb_rate ? quote.dyn_pb_rate.toFixed(2) : "-" },
      { label: "总市值", value: formatCompactNumber(quote.market_value) },
      { label: "流通市值", value: formatCompactNumber(quote.circulation_value) },
      { label: "量比", value: quote.volume_ratio?.toFixed(2) || "-" },
      { label: "成交量", value: formatCompactNumber(quote.turnover_volume) + "股" },
    ]
  }, [quote])

  useEffect(() => {
    const handler = (e: KeyboardEvent) => {
      if (e.key === "Escape") onClose()
    }
    document.addEventListener("keydown", handler)
    return () => document.removeEventListener("keydown", handler)
  }, [onClose])

  // Default crosshair from last candle
  const defaultCrosshair = useMemo((): CrosshairInfo | null => {
    if (klineData.length === 0) return null
    const c = klineData[klineData.length - 1]
    const pre = quote?.preclose_px
    const chg = pre ? ((c.close - pre) / pre) * 100 : (c.change_rate ?? 0)
    const d = new Date(c.time * 1000)
    const pad = (n: number) => String(n).padStart(2, "0")
    const ts =
      period === "minute"
        ? `${d.getFullYear()}-${pad(d.getMonth() + 1)}-${pad(d.getDate())} ${pad(d.getHours())}:${pad(d.getMinutes())}`
        : `${d.getFullYear()}-${pad(d.getMonth() + 1)}-${pad(d.getDate())}`
    return {
      time: ts,
      open: c.open?.toFixed(2) || "-",
      high: c.high?.toFixed(2) || "-",
      low: c.low?.toFixed(2) || "-",
      close: c.close?.toFixed(2) || "-",
      volume: formatCompactNumber(c.volume || 0),
      change: chg >= 0 ? `+${chg.toFixed(2)}%` : `${chg.toFixed(2)}%`,
      changeNum: chg,
    }
  }, [klineData, quote, period])

  const currCrosshair = crosshair ?? defaultCrosshair

  const up = quote != null && (quote.px_change_rate ?? -1) >= 0

  return (
    <div className="bg-canvas h-full overflow-y-auto">
      {/* Header */}
      <div className="bg-surface border-hairline flex shrink-0 items-center gap-3 border-b px-6 py-4">
        <Button
          variant="ghost"
          size="icon-xs"
          className="text-lg leading-none"
          onClick={onClose}
          title="关闭"
        >
          &times;
        </Button>
        <span className="text-h1 text-primary font-bold">{name}</span>
        <span className="text-secondary font-number text-caption">{symbol}</span>
        <Button
          variant="ghost"
          size="icon-sm"
          className="ml-1 rounded-full"
          onClick={(e) => {
            e.stopPropagation()
            if (inWatchlist) {
              deleteWatchlist.mutate(watchSymbol)
            } else {
              addWatchlist.mutate({ code: watchSymbol, name })
            }
          }}
          title={inWatchlist ? "取消自选" : "添加自选"}
        >
          <Star
            className={`size-4 ${inWatchlist ? "fill-yellow-500 text-yellow-500" : "text-secondary"}`}
          />
        </Button>
        {quote && (
          <div className="ml-auto flex items-center gap-3">
            <span className="font-number text-h1 font-bold">{quote.last_px?.toFixed(2)}</span>
            <span
              className={`font-number text-body rounded-lg px-3 py-1 font-semibold ${up ? "bg-up/20 text-up" : "bg-down/20 text-down"}`}
            >
              {up ? "+" : ""}
              {quote.px_change_rate?.toFixed(2)}%
            </span>
          </div>
        )}
      </div>

      {/* Period selector */}
      <div className="bg-surface border-hairline flex gap-2 border-b px-6 py-3">
        {periods.map((p) => (
          <Button
            key={p.value}
            variant="outline"
            size="xs"
            onClick={() => setPeriod(p.value)}
            className={`text-caption h-auto rounded-full px-4 py-1 ${
              period === p.value
                ? "border-brand bg-brand text-primary hover:bg-brand hover:text-primary"
                : "border-hairline bg-canvas text-secondary hover:border-hairline-dark hover:text-primary"
            }`}
          >
            {p.label}
          </Button>
        ))}
      </div>

      {/* Crosshair info bar */}
      {currCrosshair && (
        <div className="bg-surface border-hairline font-number text-caption flex items-center justify-between border-b px-3 py-1">
          <span className="text-primary shrink-0">{currCrosshair.time}</span>
          <div className="text-primary/40 flex shrink-0 items-center gap-2">
            <span>
              开 <span className="text-primary">{currCrosshair.open}</span>
            </span>
            <span>
              高 <span className="text-primary">{currCrosshair.high}</span>
            </span>
            <span>
              低 <span className="text-primary">{currCrosshair.low}</span>
            </span>
            <span>
              价 <span className="text-primary">{currCrosshair.close}</span>
            </span>
            <span>
              量 <span className="text-primary">{currCrosshair.volume}</span>
            </span>
            <span className={pctClass(currCrosshair.changeNum)}>{currCrosshair.change}</span>
          </div>
        </div>
      )}

      {/* Chart */}
      <div className="px-0">
        <KlineChart
          data={klineData}
          period={period}
          preclose={quote?.preclose_px}
          symbol={xgSymbol}
        />
      </div>

      {/* Metrics */}
      {quote && (
        <div className="px-6 py-3">
          <div className="text-caption grid grid-cols-4 gap-2">
            {metrics.map((m) => (
              <div
                key={m.label}
                className="bg-surface border-hairline rounded-lg border p-2.5 text-center"
              >
                <p className="text-secondary text-caption mb-1 tracking-wide uppercase">
                  {m.label}
                </p>
                <p className="font-number text-caption text-primary">{m.value}</p>
              </div>
            ))}
          </div>
        </div>
      )}

      {/* Plates */}
      {plateList.length > 0 && (
        <div className="border-hairline mt-2 border-t px-6 py-3">
          <p className="text-secondary text-caption mb-2">所属板块</p>
          <div className="grid max-h-[200px] grid-cols-2 gap-2 overflow-y-auto">
            {plateList.map((p) => (
              <button
                key={p.plate_id}
                type="button"
                disabled={!p.reason}
                className={`bg-surface border-hairline w-full rounded-lg border p-2.5 text-left transition-colors ${
                  p.reason ? "cursor-pointer hover:border-white/20" : "cursor-default"
                }`}
                onClick={() => p.reason && setDialogPlate(p)}
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
                  <p className="text-secondary text-caption mt-1 line-clamp-2 leading-relaxed">
                    {p.reason}
                  </p>
                )}
              </button>
            ))}
          </div>
        </div>
      )}

      {/* AI 异动解读 */}
      {expound && (
        <div className="border-hairline mt-2 border-t px-6 py-3">
          <p className="text-secondary text-caption mb-2">AI 异动解读</p>
          <div className="bg-surface border-hairline rounded-lg border p-3">
            <p className="text-caption text-secondary leading-relaxed whitespace-pre-line">
              {expound}
            </p>
          </div>
        </div>
      )}

      {/* Plate dialog */}
      {dialogPlate && (
        <Dialog
          open
          onOpenChange={(open) => {
            if (!open) setDialogPlate(null)
          }}
        >
          <DialogContent className="bg-surface border-hairline shadow-modal w-[420px] max-w-[90vw] gap-3 rounded-lg border p-5 sm:max-w-[90vw]">
            <div className="flex items-center gap-2 pr-8">
              <DialogTitle className="text-body text-primary font-bold">
                {dialogPlate.plate_name}
              </DialogTitle>
              <span className={`font-number text-caption ${pctClass(dialogPlate.core_avg_pcp)}`}>
                {formatRatioPct(dialogPlate.core_avg_pcp)}
              </span>
            </div>
            <DialogDescription className="sr-only">板块详情</DialogDescription>
            <p className="text-caption text-primary/70 leading-relaxed">{dialogPlate.reason}</p>
          </DialogContent>
        </Dialog>
      )}
    </div>
  )
}
