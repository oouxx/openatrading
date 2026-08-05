import { isTauri } from "@/lib/env"
import { useState, useMemo, useCallback, useEffect, useRef } from "react"
import { Star, Plus, Trash2, X, Search, ArrowUpDown, ArrowUp, ArrowDown } from "lucide-react"
import {
  useWatchlist,
  useAddWatchlist,
  useDeleteWatchlist,
  type WatchlistItem,
} from "@/hooks/query/useWatchlist"
import { subscribeRealtimeBatch } from "@/hooks/useRealtimeStream"
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

  // ── KlineDrawer ──
  const [klineStock, setKlineStock] = useState<{ symbol: string; name: string } | null>(null)

  // ── Real-time quotes via streams — single SSE for all symbols ──
  const [quotesMap, setQuotesMap] = useState<Map<string, QuoteData>>(new Map())
  const [quoteTick, setQuoteTick] = useState(0)

  useEffect(() => {
    if (items.length === 0) return
    // eslint-disable-next-line react-hooks/set-state-in-effect
    setQuotesMap(new Map())
    const codes = items.map((item) => item.code)
    return subscribeRealtimeBatch(codes, (code, data: any) => {
      setQuotesMap((prev) => {
        const next = new Map(prev)
        next.set(code, {
        last_px: data.last_px,
        px_change_rate: data.px_change_rate,
        market_value: data.market_value,
        circulation_value: data.circulation_value,
        })
        return next
      })
      setQuoteTick((n) => n + 1)
    })
  }, [items])

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
  const handleAddSearch = useCallback(
    async (q: string) => {
      setAddQuery(q)
      if (!q.trim()) {
        setAddResults([])
        return
      }
      setAddSearching(true)
      try {
        if (isTauri) {
          const { invoke } = await import("@tauri-apps/api/core")
          const data = await invoke("api_search_stocks", { q: q.trim() })
          setAddResults(data as { code: string; name: string }[])
        } else {
          const res = await fetch(`/api/search-stocks?q=${encodeURIComponent(q.trim())}`)
          if (res.ok) {
            const data = await res.json()
            setAddResults(data)
          }
        }
      } catch {
        // ignore
      } finally {
        setAddSearching(false)
      }
    },
    [isTauri]
  )

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
      quote: quotesMap.get(item.code),
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
  const renderSortIcon = (field: SortField) => {
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
            <h1 className="font-display text-heading font-bold text-foreground">自选股</h1>
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
                  className="absolute top-1/2 right-2 -translate-y-1/2 text-foreground/40 hover:text-foreground"
                  onClick={() => setLocalSearch("")}
                >
                  <X className="size-3" />
                </button>
              )}
            </div>

            <Button
              variant="outline"
              size="sm"
              className="h-8 gap-2 text-xs"
              onClick={() => setShowAdd(!showAdd)}
            >
              {showAdd ? <X className="size-3.5" /> : <Plus className="size-3.5" />}
              {showAdd ? "取消" : "添加自选"}
            </Button>
          </div>

          {/* ── Add-stock search ── */}
          {showAdd && (
            <div className="border-hairline rounded-lg mb-4 border bg-white/2 p-3">
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
                <div className="text-caption text-ink-muted py-4 text-center text-xs">
                  搜索中...
                </div>
              )}
              {addResults.length > 0 && (
                <div className="mt-2 max-h-60 space-y-1 overflow-y-auto">
                  {addResults.map((r) => (
                    <div
                      key={r.code}
                      className="hover:bg-bg-hover rounded-lg flex cursor-pointer items-center justify-between px-3 py-2 transition-colors"
                      onClick={() => handleAdd(r.code, r.name)}
                    >
                      <div className="flex items-baseline gap-2">
                        <span className="text-sm font-medium text-foreground">{r.name}</span>
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
                <div
                  key={i}
                  className="bg-bg border-hairline rounded-lg h-14 animate-pulse border"
                />
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
                <div className="text-caption flex items-center gap-4 px-3 py-2 text-caption tracking-wide text-foreground/40 uppercase">
                  <span className="flex-1">名称 / 代码</span>
                  <button
                    className="flex w-20 cursor-pointer items-center justify-end text-right transition-colors hover:text-foreground/70"
                    onClick={() => toggleSort("price")}
                  >
                    最新价 {renderSortIcon("price")}
                  </button>
                  <button
                    className="flex w-16 cursor-pointer items-center justify-end text-right transition-colors hover:text-foreground/70"
                    onClick={() => toggleSort("change")}
                  >
                    涨幅 {renderSortIcon("change")}
                  </button>
                  <button
                    className="flex w-20 cursor-pointer items-center justify-end text-right transition-colors hover:text-foreground/70"
                    onClick={() => toggleSort("market_value")}
                  >
                    总市值 {renderSortIcon("market_value")}
                  </button>
                  <button
                    className="flex w-20 cursor-pointer items-center justify-end text-right transition-colors hover:text-foreground/70"
                    onClick={() => toggleSort("circulation_value")}
                  >
                    流通市值 {renderSortIcon("circulation_value")}
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
        <div className="bg-bg-surface border-hairline w-[500px] shrink-0 overflow-y-auto border-l">
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
      className="bg-bg border-hairline hover:border-ai/30 rounded-lg flex cursor-pointer items-center gap-4 border p-3 transition-colors"
      onClick={() => onSelect({ symbol: code, name })}
    >
      <div className="flex min-w-0 flex-1 items-baseline gap-2">
        <span className="truncate text-sm font-semibold text-foreground">{name}</span>
        <span className="text-caption text-ink-muted font-mono">
          {code.replace(/^[a-z]+/i, "")}
        </span>
      </div>

      {loaded ? (
        <>
          <span className="w-20 text-right font-mono text-sm font-bold text-foreground">
            {quote.last_px?.toFixed(2)}
          </span>
          <span
            className={`rounded-lg w-16 px-2 py-0.5 text-right font-mono text-xs font-semibold ${
              up ? "bg-up/20 text-up" : "bg-down/20 text-down"
            }`}
          >
            {up ? "+" : ""}
            {quote.px_change_rate?.toFixed(2)}%
          </span>
          <span className="w-20 text-right font-mono text-xs text-foreground/70">
            {formatVolume(quote.market_value)}
          </span>
          <span className="w-20 text-right font-mono text-xs text-foreground/70">
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
        className="hover:text-down text-ink-muted w-6 shrink-0 p-1 text-right transition-colors"
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
