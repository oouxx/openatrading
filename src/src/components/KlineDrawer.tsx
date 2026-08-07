import { useState, useEffect, useMemo, useRef, useCallback } from "react"
import { subscribeKline, subscribeRealtime } from "@/hooks/useRealtimeStream"
import { usePlatesByStock } from "@/hooks/query/usePlatesByStock"
import { useFieldItems } from "@/hooks/query/useFieldItems"
import KlineChart, { type KlineCandle, type CrosshairInfo } from "./KlineChart"
import { useWatchlist, useAddWatchlist, useDeleteWatchlist } from "@/hooks/query/useWatchlist"
import { toYahooSymbol } from "@/lib/stock-utils"
import { Star } from "lucide-react"

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

function formatVolume(v: number) {
  if (!v) return "-"
  if (v > 1e8) return (v / 1e8).toFixed(2) + "亿"
  if (v > 1e4) return (v / 1e4).toFixed(2) + "万"
  return v.toFixed(0)
}

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
  const [quote, setQuote] = useState<any>(null)

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
    ;(KlineChart as any).__pendingCrosshairListener = setChartCrosshair
    ;(KlineChart as any).setCrosshairListener?.(setChartCrosshair)
  }, [setChartCrosshair, klineData])

  useEffect(() => {
    return subscribeRealtime(xgSymbol, setQuote, (err) => console.warn("[realtime-stream]", err))
  }, [xgSymbol])

  // Subscribe to kline SSE stream
  useEffect(() => {
    // eslint-disable-next-line react-hooks/set-state-in-effect
    setKlineData([])
    crosshairRef.current = null
    // eslint-disable-next-line react-hooks/set-state-in-effect
    setCrosshair(null)
    return subscribeKline(xgSymbol, period, 256, (data: any) => {
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
  const [dialogPlate, setDialogPlate] = useState<{
    plate_id: number
    plate_name: string
    core_avg_pcp: number
    reason: string
  } | null>(null)

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
      { label: "总市值", value: formatVolume(quote.market_value) },
      { label: "流通市值", value: formatVolume(quote.circulation_value) },
      { label: "量比", value: quote.volume_ratio?.toFixed(2) || "-" },
      { label: "成交量", value: formatVolume(quote.turnover_volume) + "股" },
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
      volume: formatVolume(c.volume || 0),
      change: chg >= 0 ? `+${chg.toFixed(2)}%` : `${chg.toFixed(2)}%`,
      changeNum: chg,
    }
  }, [klineData, quote, period])

  const currCrosshair = crosshair ?? defaultCrosshair

  const up = quote && quote.px_change_rate >= 0

  return (
    <div className="bg-canvas h-full overflow-y-auto">
      {/* Header */}
      <div className="bg-surface border-hairline flex shrink-0 items-center gap-3 border-b px-6 py-4">
        <button
          onClick={onClose}
          className="text-secondary p-1 text-lg leading-none hover:text-primary"
        >
          &times;
        </button>
        <span className="text-h1 font-bold text-primary">{name}</span>
        <span className="text-secondary font-number text-caption">{symbol}</span>
        <button
          onClick={(e) => {
            e.stopPropagation()
            if (inWatchlist) {
              deleteWatchlist.mutate(watchSymbol)
            } else {
              addWatchlist.mutate({ code: watchSymbol, name })
            }
          }}
          className="ml-1 flex size-7 items-center justify-center rounded-full text-caption transition-colors hover:bg-white/10"
          title={inWatchlist ? "取消自选" : "添加自选"}
        >
          <Star
            className={`size-4 ${inWatchlist ? "fill-yellow-500 text-yellow-500" : "text-secondary"}`}
          />
        </button>
        {quote && (
          <div className="ml-auto flex items-center gap-3">
            <span className="font-number text-h1 font-bold">{quote.last_px?.toFixed(2)}</span>
            <span
              className={`rounded-lg px-3 py-1 font-number text-body font-semibold ${up ? "bg-up/20 text-up" : "bg-down/20 text-down"}`}
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
          <button
            key={p.value}
            onClick={() => setPeriod(p.value)}
            className={`rounded-full border px-4 py-1 text-caption transition-colors ${
              period === p.value
                ? "border-brand bg-brand text-primary"
                : "border-hairline bg-canvas text-secondary hover:border-hairline-dark hover:text-primary"
            }`}
          >
            {p.label}
          </button>
        ))}
      </div>

      {/* Crosshair info bar */}
      {currCrosshair && (
        <div className="bg-surface border-hairline flex items-center justify-between border-b px-3 py-1 font-number text-caption">
          <span className="shrink-0 text-primary">{currCrosshair.time}</span>
          <div className="flex shrink-0 items-center gap-2 text-primary/40">
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
            <span className={currCrosshair.changeNum >= 0 ? "text-up" : "text-down"}>
              {currCrosshair.change}
            </span>
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
          <div className="grid grid-cols-4 gap-2 text-caption">
            {metrics.map((m) => (
              <div
                key={m.label}
                className="bg-surface border-hairline rounded-lg border p-2.5 text-center"
              >
                <p className="text-secondary mb-1 text-caption tracking-wide uppercase">{m.label}</p>
                <p className="font-number text-caption text-primary">{m.value}</p>
              </div>
            ))}
          </div>
        </div>
      )}

      {/* Plates */}
      {plateList.length > 0 && (
        <div className="border-hairline mt-2 border-t px-6 py-3">
          <p className="text-secondary mb-2 text-caption">所属板块</p>
          <div className="grid max-h-[200px] grid-cols-2 gap-2 overflow-y-auto">
            {plateList.map((p) => (
              <div
                key={p.plate_id}
                className="bg-surface border-hairline rounded-lg cursor-pointer border p-2.5 transition-colors hover:border-white/20"
                onClick={() => p.reason && setDialogPlate(p)}
              >
                <div className="flex items-center justify-between gap-2">
                  <span className="truncate text-caption font-medium text-primary">{p.plate_name}</span>
                  <span
                    className={`shrink-0 font-number text-caption ${p.core_avg_pcp >= 0 ? "text-up" : "text-down"}`}
                  >
                    {p.core_avg_pcp >= 0 ? "+" : ""}
                    {(p.core_avg_pcp * 100).toFixed(2)}%
                  </span>
                </div>
                {p.reason && (
                  <p className="text-secondary mt-1 line-clamp-2 text-caption leading-relaxed">
                    {p.reason}
                  </p>
                )}
              </div>
            ))}
          </div>
        </div>
      )}

      {/* AI 异动解读 */}
      {expound && (
        <div className="border-hairline mt-2 border-t px-6 py-3">
          <p className="text-secondary mb-2 text-caption">AI 异动解读</p>
          <div className="bg-surface border-hairline rounded-lg border p-3">
            <p className="text-caption text-secondary leading-relaxed whitespace-pre-line">
              {expound}
            </p>
          </div>
        </div>
      )}

      {/* Plate dialog */}
      {dialogPlate && (
        <>
          <div className="fixed inset-0 z-60 bg-black/50" onClick={() => setDialogPlate(null)} />
          <div className="pointer-events-none fixed inset-0 z-61 flex items-center justify-center">
            <div className="bg-surface border-hairline rounded-lg pointer-events-auto w-[420px] max-w-[90vw] border p-5 shadow-modal">
              <div className="mb-3 flex items-center justify-between">
                <div className="flex items-center gap-2">
                  <span className="text-body font-bold text-primary">{dialogPlate.plate_name}</span>
                  <span
                    className={`font-number text-caption ${dialogPlate.core_avg_pcp >= 0 ? "text-up" : "text-down"}`}
                  >
                    {dialogPlate.core_avg_pcp >= 0 ? "+" : ""}
                    {(dialogPlate.core_avg_pcp * 100).toFixed(2)}%
                  </span>
                </div>
                <button
                  onClick={() => setDialogPlate(null)}
                  className="text-secondary p-1 text-lg leading-none hover:text-primary"
                >
                  &times;
                </button>
              </div>
              <p className="text-caption leading-relaxed text-primary/70">{dialogPlate.reason}</p>
            </div>
          </div>
        </>
      )}
    </div>
  )
}
