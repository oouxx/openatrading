import { useState, useEffect, useMemo, useRef, useCallback } from "react"
import { subscribeKline, subscribeRealtime, type RealtimeQuote } from "@/hooks/useRealtimeStream"
import { usePlatesByStock, type PlateByStockItem } from "@/hooks/query/usePlatesByStock"
import KlineChart, { type KlineCandle } from "./KlineChart"
import { getKlineChartStatics, type CrosshairInfo } from "@/lib/klineChartStatics"
import { formatCompactNumber, formatRatioPct, pctClass } from "@/lib/format"
import { useWatchlist, useAddWatchlist, useDeleteWatchlist } from "@/hooks/query/useWatchlist"
import { classifySymbol, toYahooSymbol } from "@/lib/stock-utils"
import { Star, X } from "lucide-react"
import { Button } from "@/components/ui/button"
import { Dialog, DialogContent, DialogDescription, DialogTitle } from "@/components/ui/dialog"
import { DataEmpty, DataError, DataLoading } from "@/components/data-state"

interface KlineDrawerProps {
  symbol: string
  name: string
  onClose: () => void
  /** 嵌进网格格子时没有"关闭"这回事：隐藏关闭按钮，也不再响应 Esc。 */
  showClose?: boolean
}

const periods = [
  { label: "分时", value: "minute" },
  { label: "日K", value: "day" },
  { label: "周K", value: "week" },
  { label: "月K", value: "month" },
]

/**
 * 归一化为 Yahoo 格式；失败时回退原始输入，与后端 watchlist 容错一致。
 *
 * 交易所后缀必须透传：指数与个股共用 6 位代码（`000001.SS` 上证指数 vs
 * `000001.SZ` 平安银行），按首位数字重推会把指数画成另一只票。
 */
function toUpstreamSymbol(raw: string): string {
  try {
    return toYahooSymbol(raw)
  } catch {
    return raw.trim()
  }
}

export default function KlineDrawer({ symbol, name, onClose, showClose = true }: KlineDrawerProps) {
  const [period, setPeriod] = useState("minute")
  const xgSymbol = toUpstreamSymbol(symbol)
  // 指数/基金/债券没有市盈率、市净率（上游恒 0），也没有所属板块：
  // 只有个股才渲染这些区域
  const kind = classifySymbol(xgSymbol)
  const isStock = kind === "stock"
  const [quote, setQuote] = useState<RealtimeQuote | null>(null)

  // Watchlist
  const { data: watchlist = [] } = useWatchlist()
  const addWatchlist = useAddWatchlist()
  const deleteWatchlist = useDeleteWatchlist()
  // 自选股统一以 Yahoo 格式存储，比较/增删前先归一化，避免同一股票重复
  const watchSymbol = xgSymbol
  const inWatchlist = watchlist.some((w) => w.code === watchSymbol)

  // Kline data from SSE
  const [klineData, setKlineData] = useState<KlineCandle[]>([])
  const [klineLoading, setKlineLoading] = useState(true)
  const [klineError, setKlineError] = useState<string | null>(null)
  const [streamNonce, setStreamNonce] = useState(0)
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
    // 切换标的时先清掉上一只股票的报价，避免头部短暂显示旧价格。
    // eslint-disable-next-line react-hooks/set-state-in-effect -- 切换标的后重置可见状态
    setQuote(null)
    return subscribeRealtime<RealtimeQuote>(xgSymbol, setQuote, (err) =>
      console.warn("[realtime-stream]", err)
    )
  }, [xgSymbol])

  // Subscribe to kline SSE stream
  useEffect(() => {
    // eslint-disable-next-line react-hooks/set-state-in-effect -- 切换股票/周期时先清空上一份数据
    setKlineData([])
    setKlineLoading(true)
    setKlineError(null)
    crosshairRef.current = null
    setCrosshair(null)
    return subscribeKline<{ candles?: KlineCandle[] }>(
      xgSymbol,
      period,
      256,
      (data) => {
        if (data.candles) {
          setKlineData(data.candles)
          setKlineLoading(false)
          setKlineError(null)
        }
      },
      (err) => {
        setKlineLoading(false)
        setKlineError(err)
      }
    )
  }, [xgSymbol, period, streamNonce])
  const { data: plates } = usePlatesByStock(isStock ? xgSymbol : "")
  const plateList = Array.isArray(plates) ? plates : []
  const [dialogPlate, setDialogPlate] = useState<PlateByStockItem | null>(null)

  const metrics = useMemo(() => {
    if (!quote) return []
    const rows = [
      { label: "今开", value: quote.open_px?.toFixed(2) || "-" },
      { label: "昨收", value: quote.preclose_px?.toFixed(2) || "-" },
      { label: "最高", value: quote.high_px?.toFixed(2) || "-" },
      { label: "最低", value: quote.low_px?.toFixed(2) || "-" },
      { label: "振幅", value: quote.amplitude != null ? quote.amplitude.toFixed(2) + "%" : "-" },
      {
        label: "换手率",
        value: quote.turnover_ratio ? quote.turnover_ratio.toFixed(2) + "%" : "-",
      },
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
    // 非个股的 PE / 市净率 / 市值上游一律给 0（指数量还是成分股加总），
    // 留着只是几格 "0" / "-"：去掉后剩 8 格正好两行。
    if (!isStock) {
      const tradeableFields = new Set([
        "今开",
        "昨收",
        "最高",
        "最低",
        "振幅",
        "换手率",
        "量比",
        "成交量",
      ])
      return rows.filter((m) => tradeableFields.has(m.label))
    }
    return rows
  }, [quote, isStock])

  useEffect(() => {
    const handler = (e: KeyboardEvent) => {
      if (!showClose) return
      if (e.key === "Escape") onClose()
    }
    document.addEventListener("keydown", handler)
    return () => document.removeEventListener("keydown", handler)
  }, [onClose, showClose])

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

  // 图例始终渲染（数据未到时是占位符），高度才不会在加载前后跳一下。
  const legend: CrosshairInfo = currCrosshair ?? {
    time: "—",
    open: "—",
    high: "—",
    low: "—",
    close: "—",
    volume: "—",
    change: "—",
    changeNum: 0,
  }

  const up = quote != null && (quote.px_change_rate ?? -1) >= 0

  return (
    <div className="bg-canvas @container h-full overflow-y-auto">
      {/* Header */}
      <div className="bg-surface border-hairline flex shrink-0 flex-wrap items-center gap-2 border-b px-4 py-4 @md:gap-3 @md:px-6">
        {showClose && (
          <Button
            variant="ghost"
            size="icon-xs"
            className="text-secondary hover:text-primary"
            onClick={onClose}
            title="关闭"
            aria-label="关闭行情详情"
          >
            <X className="size-4" />
          </Button>
        )}
        <span className="text-h1 text-primary shrink-0 font-semibold tracking-[-0.01em] whitespace-nowrap">
          {name}
        </span>
        <span className="text-secondary font-number text-caption hidden @md:inline">{symbol}</span>
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
          aria-label={inWatchlist ? "取消自选" : "添加自选"}
        >
          <Star className={`size-4 ${inWatchlist ? "fill-brand text-brand" : "text-secondary"}`} />
        </Button>
        {quote && (
          <div className="ml-auto flex shrink-0 items-center gap-2 @md:gap-3">
            <span className="font-number text-h1 font-semibold">{quote.last_px?.toFixed(2)}</span>
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
      <div className="bg-surface border-hairline flex border-b px-4 py-2 @md:px-6">
        <div className="bg-hover border-hairline inline-flex rounded-md border p-0.5">
          {periods.map((p) => {
            const active = period === p.value
            return (
              <button
                key={p.value}
                type="button"
                aria-pressed={active}
                onClick={() => setPeriod(p.value)}
                className={`text-caption focus-visible:ring-brand/40 h-7 rounded-sm px-3 transition-colors outline-none focus-visible:ring-2 ${
                  active
                    ? "bg-surface text-primary shadow-card"
                    : "text-tertiary hover:text-secondary"
                }`}
              >
                {p.label}
              </button>
            )
          })}
        </div>
      </div>

      {/* Crosshair info bar（同花顺式图例）
       *
       * 固定两行、每行 18px（text-caption 行高），高度不随内容变：
       * 这里原先用 flex-wrap，而自选股详情面板宽 500px 正好卡在换行边界——
       * 分时的时间串比日 K 长 40px、量的位数随游标变化，于是同一处会一行/两行
       * 来回跳；下面 K 线是固定高度（420px），上面一变高就把图表推下去，看着就是闪。
       * 换行（而不是换行后高度自适）是根本原因，所以这里写死行数。 */}
      <div className="bg-surface border-hairline font-number text-caption border-b px-3 py-1">
        <div className="flex items-center justify-between gap-3 whitespace-nowrap">
          <span className="text-primary min-w-0 truncate">{legend.time}</span>
          <span className={`shrink-0 ${pctClass(legend.changeNum)}`}>{legend.change}</span>
        </div>
        <div className="text-tertiary flex items-center gap-x-2 overflow-hidden whitespace-nowrap">
          <span>
            开 <span className="text-primary">{legend.open}</span>
          </span>
          <span>
            高 <span className="text-primary">{legend.high}</span>
          </span>
          <span>
            低 <span className="text-primary">{legend.low}</span>
          </span>
          <span>
            价 <span className="text-primary">{legend.close}</span>
          </span>
          <span>
            量 <span className="text-primary">{legend.volume}</span>
          </span>
        </div>
      </div>

      {/* Chart */}
      <div className="relative px-5">
        <KlineChart
          data={klineData}
          period={period}
          preclose={quote?.preclose_px}
          symbol={xgSymbol}
        />
        {klineData.length === 0 && (
          <div className="bg-canvas absolute inset-0 z-10 flex items-center justify-center px-6">
            {klineLoading ? (
              <DataLoading size="compact" label="加载 K 线…" />
            ) : klineError ? (
              <DataError
                size="compact"
                title="K 线加载失败"
                description={
                  klineError === "connection error" ? "行情连接中断，请稍后重试" : klineError
                }
                onRetry={() => setStreamNonce((n) => n + 1)}
              />
            ) : (
              <DataEmpty
                size="compact"
                title="暂无 K 线数据"
                action={
                  <Button
                    type="button"
                    variant="outline"
                    size="xs"
                    onClick={() => setStreamNonce((n) => n + 1)}
                  >
                    重试
                  </Button>
                }
              />
            )}
          </div>
        )}
      </div>

      {/* Metrics */}
      {quote && (
        <div className="px-4 py-3 @md:px-6">
          <div className="text-caption grid grid-cols-2 gap-2 @md:grid-cols-4">
            {metrics.map((m) => (
              <div key={m.label} className="bg-surface border-hairline rounded-md border p-2.5">
                <p className="text-tertiary text-micro mb-1">{m.label}</p>
                <p className="font-number text-caption text-primary">{m.value}</p>
              </div>
            ))}
          </div>
        </div>
      )}

      {/* Plates */}
      {plateList.length > 0 && (
        <div className="border-hairline mt-2 border-t px-4 py-3 @md:px-6">
          <p className="text-secondary text-caption mb-2">所属板块</p>
          <div className="grid max-h-[200px] grid-cols-2 gap-2 overflow-y-auto">
            {plateList.map((p) => (
              <button
                key={p.plate_id}
                type="button"
                disabled={!p.reason}
                className={`bg-surface border-hairline w-full rounded-md border p-2.5 text-left transition-colors ${
                  p.reason ? "hover:border-brand/30 cursor-pointer" : "cursor-default"
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
                  <p className="text-caption text-secondary mt-1 line-clamp-2 leading-relaxed">
                    {p.reason}
                  </p>
                )}
              </button>
            ))}
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
              <DialogTitle className="text-body text-primary font-semibold">
                {dialogPlate.plate_name}
              </DialogTitle>
              <span className={`font-number text-caption ${pctClass(dialogPlate.core_avg_pcp)}`}>
                {formatRatioPct(dialogPlate.core_avg_pcp)}
              </span>
            </div>
            <DialogDescription className="sr-only">板块详情</DialogDescription>
            <p className="text-body text-secondary leading-relaxed">{dialogPlate.reason}</p>
          </DialogContent>
        </Dialog>
      )}
    </div>
  )
}
