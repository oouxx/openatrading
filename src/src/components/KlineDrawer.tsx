import { useState, useEffect, useMemo } from "react"
import { getKlineImageUrl } from "@/lib/stock-utils"
import { useRealtimeQuote } from "@/hooks/query/useRealtimeQuote"
import { usePlatesByStock } from "@/hooks/query/usePlatesByStock"
import { useFieldItems } from "@/hooks/query/useFieldItems"

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

interface KlineDrawerProps {
  symbol: string
  name: string
  onClose: () => void
}

function formatVolume(v: number) {
  if (!v) return "-"
  if (v > 1e8) return (v / 1e8).toFixed(2) + "亿"
  if (v > 1e4) return (v / 1e4).toFixed(2) + "万"
  return v.toFixed(0)
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
  const { data: quote } = useRealtimeQuote(xgSymbol)
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

  const klineUrl = getKlineImageUrl(symbol, period)

  const up = quote && quote.px_change_rate >= 0
  const _changeClass = up ? "text-up" : "text-down"

  return (
    <div className="bg-bg h-full overflow-y-auto">
      {/* Header */}
      <div className="border-hairline bg-bg-surface flex flex-shrink-0 items-center gap-3 border-b px-6 py-4">
        <button
          onClick={onClose}
          className="text-ink-muted p-1 text-lg leading-none hover:text-white"
        >
          &times;
        </button>
        <span className="text-xl font-bold text-white">{name}</span>
        <span className="text-ink-muted font-mono text-xs">{symbol}</span>
        {quote && (
          <div className="ml-auto flex items-center gap-3">
            <span className="font-mono text-[22px] font-bold">{quote.last_px?.toFixed(2)}</span>
            <span
              className={`rounded-card px-2.5 py-1 font-mono text-sm font-semibold ${up ? "bg-up/20 text-up" : "bg-down/20 text-down"}`}
            >
              {up ? "+" : ""}
              {quote.px_change_rate?.toFixed(2)}%
            </span>
          </div>
        )}
      </div>

      {/* Period selector */}
      <div className="border-hairline bg-bg-surface flex gap-1.5 border-b px-6 py-2.5">
        {periods.map((p) => (
          <button
            key={p.value}
            onClick={() => setPeriod(p.value)}
            className={`rounded-full border px-3.5 py-1 text-xs transition-colors ${
              period === p.value
                ? "border-ai bg-ai text-white"
                : "border-hairline bg-bg text-ink-muted hover:border-hairline-dark hover:text-white"
            }`}
          >
            {p.label}
          </button>
        ))}
      </div>

      {/* Chart */}
      <div className="px-6 py-2.5">
        <img
          src={klineUrl}
          alt={`${name} K线图`}
          className="rounded-card w-full"
          style={{ minHeight: 300, maxHeight: 420 }}
        />
      </div>

      {/* Metrics */}
      {quote && (
        <div className="px-6 py-2.5">
          <div className="grid grid-cols-4 gap-2 text-xs">
            {metrics.map((m) => (
              <div
                key={m.label}
                className="rounded-card border-hairline bg-bg-surface border p-2.5 text-center"
              >
                <p className="text-ink-muted mb-1 text-[10px] tracking-wide uppercase">{m.label}</p>
                <p className="font-mono text-xs text-white">{m.value}</p>
              </div>
            ))}
          </div>
        </div>
      )}

      {/* Plates */}
      {plateList.length > 0 && (
        <div className="border-hairline mt-2 border-t px-6 py-2.5">
          <p className="text-ink-muted mb-2 text-xs">所属板块</p>
          <div className="grid max-h-[200px] grid-cols-2 gap-2 overflow-y-auto">
            {plateList.map((p) => (
              <div
                key={p.plate_id}
                className="rounded-card border-hairline bg-bg-surface cursor-pointer border p-2.5 transition-colors hover:border-white/20"
                onClick={() => p.reason && setDialogPlate(p)}
              >
                <div className="flex items-center justify-between gap-2">
                  <span className="truncate text-xs font-medium text-white">{p.plate_name}</span>
                  <span
                    className={`shrink-0 font-mono text-xs ${p.core_avg_pcp >= 0 ? "text-up" : "text-down"}`}
                  >
                    {p.core_avg_pcp >= 0 ? "+" : ""}
                    {(p.core_avg_pcp * 100).toFixed(2)}%
                  </span>
                </div>
                {p.reason && (
                  <p className="text-ink-muted mt-1 line-clamp-2 text-[11px] leading-relaxed">
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
        <div className="border-hairline mt-2 border-t px-6 py-2.5">
          <p className="text-ink-muted mb-2 text-xs">AI 异动解读</p>
          <div className="rounded-card border-hairline bg-bg-surface border p-3">
            <p className="text-caption text-ink-muted leading-relaxed whitespace-pre-line">
              {expound}
            </p>
          </div>
        </div>
      )}

      {/* Plate dialog */}
      {dialogPlate && (
        <>
          <div className="fixed inset-0 z-[60] bg-black/50" onClick={() => setDialogPlate(null)} />
          <div className="pointer-events-none fixed inset-0 z-[61] flex items-center justify-center">
            <div className="rounded-card border-hairline bg-bg-surface pointer-events-auto w-[420px] max-w-[90vw] border p-5 shadow-2xl">
              <div className="mb-3 flex items-center justify-between">
                <div className="flex items-center gap-2">
                  <span className="text-sm font-bold text-white">{dialogPlate.plate_name}</span>
                  <span
                    className={`font-mono text-xs ${dialogPlate.core_avg_pcp >= 0 ? "text-up" : "text-down"}`}
                  >
                    {dialogPlate.core_avg_pcp >= 0 ? "+" : ""}
                    {(dialogPlate.core_avg_pcp * 100).toFixed(2)}%
                  </span>
                </div>
                <button
                  onClick={() => setDialogPlate(null)}
                  className="text-ink-muted p-1 text-lg leading-none hover:text-white"
                >
                  &times;
                </button>
              </div>
              <p className="text-[13px] leading-relaxed text-white/70">{dialogPlate.reason}</p>
            </div>
          </div>
        </>
      )}
    </div>
  )
}
