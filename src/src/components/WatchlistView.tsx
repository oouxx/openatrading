import { useState, useMemo, useCallback, useEffect, useRef } from "react"
import { Star, Plus, Trash2, X, Search, ArrowUpDown, ArrowUp, ArrowDown } from "lucide-react"
import {
  useWatchlist,
  useAddWatchlist,
  useDeleteWatchlist,
  type WatchlistItem,
} from "@/hooks/query/useWatchlist"
import { subscribeRealtime } from "@/hooks/useRealtimeStream"
import { Button } from "@/components/ui/button"
import { Input } from "@/components/ui/input"
import KlineDrawer from "./KlineDrawer"

function formatVolume(v: number | undefined): string {
  if (!v) return "-"
  if (v > 1e8) return (v / 1e8).toFixed(2) + "亿"
  if (v > 1e4) return (v / 1e4).toFixed(2) + "万"
  return v.toFixed(0)
}

type SortField = "price" | "change" | "market_value" | "circulation_value" | "added_at"
type SortDir = "asc" | "desc"

interface QuoteData {
  last_px?: number
  px_change_rate?: number
  market_value?: number
  circulation_value?: number
}

/** Pair a watchlist item with its realtime quote. */
interface RichStock extends WatchlistItem {
  quote: QuoteData | undefined
}

export default function WatchlistView() {
  const { data: items = [], isLoading } = useWatchlist()
  const addWatchlist = useAddWatchlist()
  const deleteWatchlist = useDeleteWatchlist()

  // ── Real-time quotes via streams ──
  const quotesRef = useRef<Map<string, QuoteData>>(new Map())
  const [quoteTick, setQuoteTick] = useState(0)

  useEffect(() => {
    quotesRef.current = new Map()
    const unsubs = items.map((item) =>
      subscribeRealtime(item.code, (data: any) => {
        quotesRef.current.set(item.code, data)
        setQuoteTick((n) => n + 1)
      })
    )
    return () => unsubs.forEach((fn) => fn())
  }, [items])

  // ── KlineDrawer ──
  const [klineStock, setKlineStock] = useState<{ symbol: string; name: string } | null>(null)

  // ── Add-stock search ──
  const [showAdd, setShowAdd] = useState(false)
  const [addQuery, setAddQuery] = useState("")
  const [addResults, setAddResults] = useState<{ code: string; name: string }[]>([])
  const [addSearching, setAddSearching] = useState(false)

  // ── Local search ──
  const [localSearch, setLocalSearch] = useState("")

  // ── Sort ──
  const [sortField, setSortField] = useState<SortField>("added_at")
  const [sortDir, setSortDir] = useState<SortDir>("desc")

  const toggleSort = useCallback(
    (field: SortField) => {
      if (sortField === field) {
        setSortDir((d) => (d === "asc" ? "desc" : "asc"))
      } else {
        setSortField(field)
        setSortDir(field === "added_at" ? "asc" : "desc")
      }
    },
    [sortField]
  )

  // ── Add-stock search ──
  const handleAddSearch = useCallback(async (q: string) => {
    setAddQuery(q)
    if (!q.trim()) {
      setAddResults([])
      return
    }
    setAddSearching(true)
    try {
      const res = await fetch(`/api/search-stocks?q=${encodeURIComponent(q.trim())}`)
      if (res.ok) {
        const data = await res.json()
        setAddResults(data)
      }
    } catch {
      // ignore
    } finally {
      setAddSearching(false)
    }
  }, [])

  const handleAdd = useCallback(
    (code: string, name: string) => {
      addWatchlist.mutate({ code, name })
      setAddQuery("")
      setAddResults([])
      setShowAdd(false)
    },
    [addWatchlist]
  )

  // ── Build rich list + sort ──
  const richList = useMemo((): RichStock[] => {
    return items.map((item, i) => ({
      ...item,
      quote: quotesRef.current.get(item.code),
    }))
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [items, quoteTick])

  const sorted = useMemo(() => {
    let list = richList

    // Local search filter
    if (localSearch.trim()) {
      const q = localSearch.trim().toLowerCase()
      list = list.filter(
        (s) => s.name.toLowerCase().includes(q) || s.code.replace(/^[a-z]+/i, "").includes(q)
      )
    }

    // Sort
    const dir = sortDir === "asc" ? 1 : -1
    return [...list].sort((a, b) => {
      const getVal = (s: RichStock): number => {
        switch (sortField) {
          case "price":
            return s.quote?.last_px ?? -Infinity
          case "change":
            return s.quote?.px_change_rate ?? -Infinity
          case "market_value":
            return s.quote?.market_value ?? -Infinity
          case "circulation_value":
            return s.quote?.circulation_value ?? -Infinity
          case "added_at":
            return Date.parse(s.added_at) || 0
        }
      }
      return (getVal(a) - getVal(b)) * dir
    })
  }, [richList, sortField, sortDir, localSearch])

  // ── Sort icon ──
  const SortIcon = ({ field }: { field: SortField }) => {
    if (sortField !== field) return <ArrowUpDown className="ml-0.5 inline size-2.5 opacity-40" />
    return sortDir === "asc" ? (
      <ArrowUp className="ml-0.5 inline size-2.5" />
    ) : (
      <ArrowDown className="ml-0.5 inline size-2.5" />
    )
  }

  return (
    <div className="flex h-full">
      {/* Main content */}
      <div className="min-w-0 flex-1 overflow-y-auto">
        <div className="mx-auto max-w-[1200px] px-6 py-4">
          {/* ── Header ── */}
          <div className="mb-4 flex items-center gap-3">
            <Star className="size-5 text-yellow-500" />
            <h1 className="text-lg font-bold text-white">自选股</h1>
            <span className="text-caption text-ink-muted">{items.length}只</span>

            {/* Local search */}
            <div className="relative ml-auto hidden w-48 sm:block">
              <Search className="text-ink-muted absolute top-1/2 left-2.5 size-3 -translate-y-1/2" />
              <Input
                placeholder="搜索自选..."
                value={localSearch}
                onChange={(e) => setLocalSearch(e.target.value)}
                className="h-8 pl-8 text-xs"
              />
              {localSearch && (
                <button
                  className="absolute top-1/2 right-2 -translate-y-1/2 text-white/40 hover:text-white"
                  onClick={() => setLocalSearch("")}
                >
                  <X className="size-3" />
                </button>
              )}
            </div>

            <Button
              variant="outline"
              size="sm"
              className="h-8 gap-1.5 text-xs"
              onClick={() => setShowAdd(!showAdd)}
            >
              {showAdd ? <X className="size-3.5" /> : <Plus className="size-3.5" />}
              {showAdd ? "取消" : "添加自选"}
            </Button>
          </div>

          {/* ── Add-stock search ── */}
          {showAdd && (
            <div className="border-hairline mb-4 rounded-card border bg-white/[0.02] p-3">
              <div className="relative">
                <Search className="text-ink-muted absolute top-1/2 left-3 size-3.5 -translate-y-1/2" />
                <Input
                  placeholder="输入股票名称或代码搜索..."
                  value={addQuery}
                  onChange={(e) => handleAddSearch(e.target.value)}
                  className="h-9 pl-9 text-xs"
                  autoFocus
                />
              </div>
              {addSearching && (
                <div className="text-caption text-ink-muted py-4 text-center text-xs">搜索中...</div>
              )}
              {addResults.length > 0 && (
                <div className="mt-2 max-h-60 space-y-1 overflow-y-auto">
                  {addResults.map((r) => (
                    <div
                      key={r.code}
                      className="rounded-card hover:bg-bg-hover flex cursor-pointer items-center justify-between px-3 py-2 transition-colors"
                      onClick={() => handleAdd(r.code, r.name)}
                    >
                      <div className="flex items-baseline gap-2">
                        <span className="text-sm font-medium text-white">{r.name}</span>
                        <span className="text-caption text-ink-muted font-mono text-xs">
                          {r.code.replace(/^[a-z]+/i, "")}
                        </span>
                      </div>
                      <Plus className="text-ai size-3.5" />
                    </div>
                  ))}
                </div>
              )}
            </div>
          )}

          {/* ── Loading / Empty / List ── */}
          {isLoading ? (
            <div className="space-y-2">
              {Array.from({ length: 5 }).map((_, i) => (
                <div key={i} className="rounded-card border-hairline bg-bg h-14 animate-pulse border" />
              ))}
            </div>
          ) : sorted.length === 0 ? (
            <div className="text-caption text-ink-muted py-16 text-center">
              {localSearch.trim()
                ? "未找到匹配的自选股"
                : showAdd
                  ? "在上方搜索并添加股票"
                  : "暂无自选股，点击右上角「添加自选」开始"}
            </div>
          ) : (
            <div className="space-y-2">
              {/* Column headers — clickable sort */}
              {!showAdd && (
                <div className="text-caption flex items-center gap-4 px-3 py-1.5 text-[11px] tracking-wide uppercase text-white/40">
                  <span className="flex-1">名称 / 代码</span>
                  <button
                    className="flex w-20 cursor-pointer items-center justify-end text-right transition-colors hover:text-white/70"
                    onClick={() => toggleSort("price")}
                  >
                    最新价 <SortIcon field="price" />
                  </button>
                  <button
                    className="flex w-16 cursor-pointer items-center justify-end text-right transition-colors hover:text-white/70"
                    onClick={() => toggleSort("change")}
                  >
                    涨幅 <SortIcon field="change" />
                  </button>
                  <button
                    className="flex w-20 cursor-pointer items-center justify-end text-right transition-colors hover:text-white/70"
                    onClick={() => toggleSort("market_value")}
                  >
                    总市值 <SortIcon field="market_value" />
                  </button>
                  <button
                    className="flex w-20 cursor-pointer items-center justify-end text-right transition-colors hover:text-white/70"
                    onClick={() => toggleSort("circulation_value")}
                  >
                    流通市值 <SortIcon field="circulation_value" />
                  </button>
                  <span className="w-6" />
                </div>
              )}
              {sorted.map((s) => (
                <StockRow
                  key={s.code}
                  quote={s.quote}
                  name={s.name}
                  code={s.code}
                  onSelect={(v) => setKlineStock(v)}
                  onDelete={() => deleteWatchlist.mutate(s.code)}
                />
              ))}
            </div>
          )}
        </div>
      </div>

      {/* KlineDrawer overlay */}
      {klineStock && (
        <div className="border-hairline w-[500px] flex-shrink-0 border-l overflow-y-auto bg-bg-surface">
          <KlineDrawer
            symbol={klineStock.symbol}
            name={klineStock.name}
            onClose={() => setKlineStock(null)}
          />
        </div>
      )}
    </div>
  )
}

// ── StockRow ──

function StockRow({
  quote,
  name,
  code,
  onSelect,
  onDelete,
}: {
  quote: QuoteData | undefined
  name: string
  code: string
  onSelect: (s: { symbol: string; name: string }) => void
  onDelete: () => void
}) {
  const up = quote && quote.px_change_rate != null && quote.px_change_rate >= 0
  const loaded = quote?.last_px != null

  return (
    <div
      className="rounded-card border-hairline bg-bg hover:border-ai/30 flex cursor-pointer items-center gap-4 border p-3 transition-colors"
      onClick={() => onSelect({ symbol: code, name })}
    >
      <div className="flex min-w-0 flex-1 items-baseline gap-2">
        <span className="truncate text-sm font-semibold text-white">{name}</span>
        <span className="text-caption text-ink-muted font-mono">{code.replace(/^[a-z]+/i, "")}</span>
      </div>

      {loaded ? (
        <>
          <span className="w-20 text-right font-mono text-sm font-bold text-white">
            {quote.last_px?.toFixed(2)}
          </span>
          <span
            className={`rounded-card w-16 px-2 py-0.5 text-right font-mono text-xs font-semibold ${
              up ? "bg-up/20 text-up" : "bg-down/20 text-down"
            }`}
          >
            {up ? "+" : ""}
            {quote.px_change_rate?.toFixed(2)}%
          </span>
          <span className="w-20 text-right font-mono text-xs text-white/70">
            {formatVolume(quote.market_value)}
          </span>
          <span className="w-20 text-right font-mono text-xs text-white/70">
            {formatVolume(quote.circulation_value)}
          </span>
        </>
      ) : (
        <>
          <div className="text-caption text-ink-muted h-4 w-20 animate-pulse rounded bg-white/5" />
          <div className="text-caption text-ink-muted h-4 w-16 animate-pulse rounded bg-white/5" />
          <div className="text-caption text-ink-muted h-4 w-20 animate-pulse rounded bg-white/5" />
          <div className="text-caption text-ink-muted h-4 w-20 animate-pulse rounded bg-white/5" />
        </>
      )}

      <button
        className="text-ink-muted hover:text-down w-6 flex-shrink-0 p-1 text-right transition-colors"
        onClick={(e) => {
          e.stopPropagation()
          onDelete()
        }}
        title="删除自选"
      >
        <Trash2 className="size-3.5" />
      </button>
    </div>
  )
}
