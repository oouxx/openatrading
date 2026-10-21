import { formatCompactNumber } from "@/lib/format"
import { searchStocks } from "@/lib/stock-utils"
import { useState, useMemo, useCallback, useEffect } from "react"
import { Star, Plus, Trash2, X, Search, ArrowUpDown, ArrowUp, ArrowDown } from "lucide-react"
import {
  useWatchlist,
  useAddWatchlist,
  useDeleteWatchlist,
  type WatchlistItem,
} from "@/hooks/query/useWatchlist"
import { subscribeRealtimeBatch, type RealtimeQuote } from "@/hooks/useRealtimeStream"
import { Button } from "@/components/ui/button"
import { Input } from "@/components/ui/input"
import KlineDrawer from "./KlineDrawer"

type SortField = "price" | "change" | "market_value" | "circulation_value" | "added_at"
type SortDir = "asc" | "desc"

/** Pair a watchlist item with its realtime quote. */
interface RichStock extends WatchlistItem {
  quote: RealtimeQuote | undefined
}

export default function WatchlistView() {
  const { data: items = [], isLoading } = useWatchlist()
  const addWatchlist = useAddWatchlist()
  const deleteWatchlist = useDeleteWatchlist()

  // ── KlineDrawer ──
  const [klineStock, setKlineStock] = useState<{ symbol: string; name: string } | null>(null)

  // ── Real-time quotes via streams — single SSE for all symbols ──
  const [quotesMap, setQuotesMap] = useState<Map<string, RealtimeQuote>>(new Map())
  const [quoteTick, setQuoteTick] = useState(0)

  useEffect(() => {
    if (items.length === 0) return
    // eslint-disable-next-line react-hooks/set-state-in-effect -- 切换自选列表时先清空上一轮行情
    setQuotesMap(new Map())
    const codes = items.map((item) => item.code)
    return subscribeRealtimeBatch<RealtimeQuote>(codes, (code, data) => {
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
  const handleAddSearch = useCallback(async (q: string) => {
    setAddQuery(q)
    if (!q.trim()) {
      setAddResults([])
      return
    }
    setAddSearching(true)
    try {
      setAddResults(await searchStocks(q))
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
    return items.map((item) => ({
      ...item,
      quote: quotesMap.get(item.code),
    }))
    // eslint-disable-next-line react-hooks/exhaustive-deps -- quoteTick 仅用于在实时推送后触发重算
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
        <div className="max-w-wide mx-auto px-6 py-6">
          {/* ── Header ── */}
          <div className="mb-4 flex items-center gap-3">
            <Star className="text-warning size-5" />
            <h1 className="text-h1 text-primary font-bold">自选股</h1>
            <span className="text-caption text-secondary">{items.length}只</span>

            {/* Local search */}
            <div className="relative ml-auto hidden w-48 sm:block">
              <Search className="text-secondary absolute top-1/2 left-2.5 size-3 -translate-y-1/2" />
              <Input
                placeholder="搜索自选..."
                value={localSearch}
                onChange={(e) => setLocalSearch(e.target.value)}
                className="text-caption h-8 pl-8"
              />
              {localSearch && (
                <Button
                  variant="ghost"
                  size="icon-xs"
                  className="absolute top-1/2 right-2 -translate-y-1/2"
                  onClick={() => setLocalSearch("")}
                  title="清空搜索"
                >
                  <X />
                </Button>
              )}
            </div>

            <Button
              variant="outline"
              size="sm"
              className="text-caption h-8 gap-2"
              onClick={() => setShowAdd(!showAdd)}
            >
              {showAdd ? <X className="size-3.5" /> : <Plus className="size-3.5" />}
              {showAdd ? "取消" : "添加自选"}
            </Button>
          </div>

          {/* ── Add-stock search ── */}
          {showAdd && (
            <div className="border-hairline bg-canvas mb-4 rounded-lg border p-3">
              <div className="relative">
                <Search className="text-secondary absolute top-1/2 left-3 size-3.5 -translate-y-1/2" />
                <Input
                  placeholder="输入股票名称或代码搜索..."
                  value={addQuery}
                  onChange={(e) => handleAddSearch(e.target.value)}
                  className="text-caption h-9 pl-9"
                  autoFocus
                />
              </div>
              {addSearching && (
                <div className="text-caption text-secondary py-4 text-center">搜索中...</div>
              )}
              {addResults.length > 0 && (
                <div className="mt-2 max-h-60 space-y-1 overflow-y-auto">
                  {addResults.map((r) => (
                    <button
                      key={r.code}
                      type="button"
                      className="hover:bg-hover flex w-full cursor-pointer items-center justify-between rounded-lg px-3 py-2 text-left transition-colors"
                      onClick={() => handleAdd(r.code, r.name)}
                    >
                      <span className="flex items-baseline gap-2">
                        <span className="text-body text-primary font-medium">{r.name}</span>
                        <span className="text-caption text-secondary font-number">
                          {r.code.replace(/^[a-z]+/i, "")}
                        </span>
                      </span>
                      <Plus className="text-brand size-3.5" />
                    </button>
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
                  className="bg-canvas border-hairline h-14 animate-pulse rounded-lg border"
                />
              ))}
            </div>
          ) : sorted.length === 0 ? (
            <div className="text-caption text-secondary py-16 text-center">
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
                <div className="text-caption text-tertiary flex items-center gap-4 px-3 py-2">
                  <span className="flex-1">名称 / 代码</span>
                  <Button
                    variant="ghost"
                    size="xs"
                    className="text-caption text-tertiary hover:text-secondary h-auto w-20 justify-end px-0 font-normal hover:bg-transparent"
                    onClick={() => toggleSort("price")}
                  >
                    最新价 {renderSortIcon("price")}
                  </Button>
                  <Button
                    variant="ghost"
                    size="xs"
                    className="text-caption text-tertiary hover:text-secondary h-auto w-16 justify-end px-0 font-normal hover:bg-transparent"
                    onClick={() => toggleSort("change")}
                  >
                    涨幅 {renderSortIcon("change")}
                  </Button>
                  <Button
                    variant="ghost"
                    size="xs"
                    className="text-caption text-tertiary hover:text-secondary h-auto w-20 justify-end px-0 font-normal hover:bg-transparent"
                    onClick={() => toggleSort("market_value")}
                  >
                    总市值 {renderSortIcon("market_value")}
                  </Button>
                  <Button
                    variant="ghost"
                    size="xs"
                    className="text-caption text-tertiary hover:text-secondary h-auto w-20 justify-end px-0 font-normal hover:bg-transparent"
                    onClick={() => toggleSort("circulation_value")}
                  >
                    流通市值 {renderSortIcon("circulation_value")}
                  </Button>
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
        <div className="bg-surface border-hairline w-detail-panel shrink-0 overflow-y-auto border-l">
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
  quote: RealtimeQuote | undefined
  name: string
  code: string
  onSelect: (s: { symbol: string; name: string }) => void
  onDelete: () => void
}) {
  const up = quote && quote.px_change_rate != null && quote.px_change_rate >= 0
  const loaded = quote?.last_px != null

  return (
    <div className="bg-canvas border-hairline hover:border-brand/30 flex items-center gap-4 rounded-lg border p-3 transition-colors">
      <button
        type="button"
        className="flex min-w-0 flex-1 cursor-pointer items-center gap-4 text-left"
        onClick={() => onSelect({ symbol: code, name })}
      >
        <div className="flex min-w-0 flex-1 items-baseline gap-2">
          <span className="text-body text-primary truncate font-semibold">{name}</span>
          <span className="text-caption text-secondary font-number">
            {code.replace(/^[a-z]+/i, "")}
          </span>
        </div>

        {loaded ? (
          <>
            <span className="font-number text-body text-primary w-20 text-right font-bold">
              {quote.last_px?.toFixed(2)}
            </span>
            <span
              className={`font-number text-caption w-16 rounded-lg px-2 py-0.5 text-right font-semibold ${
                up ? "bg-up/20 text-up" : "bg-down/20 text-down"
              }`}
            >
              {up ? "+" : ""}
              {quote.px_change_rate?.toFixed(2)}%
            </span>
            <span className="font-number text-caption text-secondary w-20 text-right">
              {formatCompactNumber(quote.market_value)}
            </span>
            <span className="font-number text-caption text-secondary w-20 text-right">
              {formatCompactNumber(quote.circulation_value)}
            </span>
          </>
        ) : (
          <>
            <div className="text-caption text-secondary bg-hover h-4 w-20 animate-pulse rounded" />
            <div className="text-caption text-secondary bg-hover h-4 w-16 animate-pulse rounded" />
            <div className="text-caption text-secondary bg-hover h-4 w-20 animate-pulse rounded" />
            <div className="text-caption text-secondary bg-hover h-4 w-20 animate-pulse rounded" />
          </>
        )}
      </button>

      <Button
        type="button"
        variant="ghost"
        size="icon-xs"
        aria-label={`删除自选 ${name}`}
        className="text-secondary hover:text-down size-6 shrink-0"
        onClick={onDelete}
        title="删除自选"
      >
        <Trash2 />
      </Button>
    </div>
  )
}
