import { formatCompactNumber } from "@/lib/format"
import { searchStocks } from "@/lib/stock-utils"
import { useState, useMemo, useCallback, useEffect } from "react"
import {
  AlertTriangle,
  ArrowDown,
  ArrowUp,
  ArrowUpDown,
  Plus,
  Search,
  Star,
  Trash2,
  X,
} from "lucide-react"
import {
  useWatchlist,
  useAddWatchlist,
  useDeleteWatchlist,
  type WatchlistItem,
} from "@/hooks/query/useWatchlist"
import { subscribeRealtimeBatch, type RealtimeQuote } from "@/hooks/useRealtimeStream"
import { Button } from "@/components/ui/button"
import { Input } from "@/components/ui/input"
import { DataEmpty, DataLoading } from "@/components/data-state"
import { cn } from "@/lib/utils"
import KlineDrawer from "./KlineDrawer"
import { useLayoutStore } from "@/stores/layoutStore"
import { resolveStockTarget } from "@/stores/paneLinkStore"
import StockLinkSelect from "@/components/StockLinkSelect"

type SortField = "price" | "change" | "market_value" | "circulation_value" | "added_at"
type SortDir = "asc" | "desc"

/** Pair a watchlist item with its realtime quote. */
interface RichStock extends WatchlistItem {
  quote: RealtimeQuote | undefined
}

/**
 * 自选股。
 *
 * 放进网格时（带 `leafId` / `layoutId`）：点某只票把 K 线**派发出去**——默认在本格
 * 打开子页面，也可以在表头手动关联到另一格（如「个股详情」或某个行情格），点行就
 * 在那一格打开。关联 / 解关联只影响"点行去哪一格"，不改这一格的内容。
 *
 * 作为独立页面（侧栏「自选」）时没有别的格子可派发，K 线就在本页内开。
 */
export default function WatchlistView({
  leafId,
  layoutId,
}: {
  leafId?: string
  layoutId?: string
} = {}) {
  const { data: items = [], isLoading } = useWatchlist()
  const addWatchlist = useAddWatchlist()
  const deleteWatchlist = useDeleteWatchlist()

  // 独立页面模式的 K 线（网格模式下派发到别的格子，不用这份状态）
  const [pageKline, setPageKline] = useState<{ symbol: string; name: string } | null>(null)
  const openStockInPane = useLayoutStore((s) => s.openStockInPane)

  const openStock = useCallback(
    (code: string, name: string) => {
      if (!layoutId || !leafId) {
        setPageKline({ symbol: code, name })
        return
      }
      openStockInPane(layoutId, resolveStockTarget(layoutId) ?? leafId, code, name)
    },
    [layoutId, leafId, openStockInPane]
  )

  // ── Real-time quotes via streams（内部按 60 一片拆成多条 SSE） ──
  const [quotesMap, setQuotesMap] = useState<Map<string, RealtimeQuote>>(new Map())
  const [quoteTick, setQuoteTick] = useState(0)
  // 行情流断开时给一句明确的话：以前是静默失败，行里只剩「—」，看着像数据源没数
  const [streamError, setStreamError] = useState<string | null>(null)

  useEffect(() => {
    if (items.length === 0) return
    // eslint-disable-next-line react-hooks/set-state-in-effect -- 切换自选列表时先清空上一轮行情
    setQuotesMap(new Map())
    const codes = items.map((item) => item.code)
    return subscribeRealtimeBatch<RealtimeQuote>(
      codes,
      (code, data) => {
        setStreamError(null)
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
      },
      (err) => setStreamError(err)
    )
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

  return (
    <div className="relative flex h-full">
      {/* Main content */}
      <div className="min-w-0 flex-1 overflow-y-auto">
        <div className="mx-auto w-full max-w-5xl px-6 py-8">
          {/* ── Header ── */}
          <header className="flex flex-wrap items-end justify-between gap-4 pb-5">
            <div>
              <h1 className="text-h1 text-primary font-semibold tracking-[-0.01em]">自选股</h1>
              <p className="text-caption text-tertiary mt-1">
                {items.length > 0 ? `${items.length} 只 · 实时行情` : "实时行情与 K 线"}
              </p>
            </div>

            <div className="flex items-center gap-2">
              <div className="relative hidden w-52 sm:block">
                <Search className="text-tertiary pointer-events-none absolute top-1/2 left-2.5 size-3.5 -translate-y-1/2" />
                <Input
                  placeholder="搜索自选…"
                  aria-label="搜索自选股"
                  value={localSearch}
                  onChange={(e) => setLocalSearch(e.target.value)}
                  className="text-caption h-8 rounded-md pl-8"
                />
                {localSearch && (
                  <Button
                    variant="ghost"
                    size="icon-xs"
                    className="text-tertiary hover:text-primary absolute top-1/2 right-1.5 -translate-y-1/2"
                    onClick={() => setLocalSearch("")}
                    title="清空搜索"
                    aria-label="清空搜索"
                  >
                    <X />
                  </Button>
                )}
              </div>

              {layoutId && <StockLinkSelect layoutId={layoutId} />}

              <Button size="sm" onClick={() => setShowAdd(!showAdd)}>
                {showAdd ? <X /> : <Plus />}
                {showAdd ? "取消" : "添加自选"}
              </Button>
            </div>
          </header>

          {/* ── Add-stock search ── */}
          {showAdd && (
            <div className="bg-surface border-hairline mb-5 rounded-lg border p-4">
              <div className="relative">
                <Search className="text-tertiary pointer-events-none absolute top-1/2 left-3 size-3.5 -translate-y-1/2" />
                <Input
                  placeholder="输入股票名称或代码…"
                  aria-label="搜索要添加的股票"
                  value={addQuery}
                  onChange={(e) => handleAddSearch(e.target.value)}
                  className="text-body h-9 rounded-md pl-9"
                  autoFocus
                />
              </div>
              {addSearching && (
                <p className="text-caption text-tertiary py-4 text-center">搜索中…</p>
              )}
              {!addSearching && addQuery.trim() && addResults.length === 0 && (
                <p className="text-caption text-tertiary py-4 text-center">没有找到匹配的股票</p>
              )}
              {addResults.length > 0 && (
                <div className="mt-2 max-h-64 overflow-y-auto">
                  {addResults.map((r) => (
                    <Button
                      key={r.code}
                      type="button"
                      variant="ghost"
                      className="hover:bg-hover h-auto w-full justify-between rounded-md px-3 py-2.5 text-left font-normal"
                      onClick={() => handleAdd(r.code, r.name)}
                    >
                      <span className="flex items-baseline gap-2">
                        <span className="text-body text-primary font-medium">{r.name}</span>
                        <span className="text-caption text-tertiary font-number">
                          {r.code.replace(/^[a-z]+/i, "")}
                        </span>
                      </span>
                      <Plus className="text-brand size-3.5" />
                    </Button>
                  ))}
                </div>
              )}
            </div>
          )}

          {/* 行情流断开：明确说是"连接断了"，别让空行「—」被读成"没有数据" */}
          {streamError && !isLoading && sorted.length > 0 && (
            <div className="border-warning/30 bg-warning/10 text-warning text-caption mb-3 flex items-center gap-1.5 rounded-md border px-3 py-2">
              <AlertTriangle className="size-3.5 shrink-0" />
              <span className="min-w-0 truncate">行情连接中断（{streamError}），正在自动重连…</span>
            </div>
          )}

          {/* ── Loading / Empty / List ── */}
          <section className="bg-surface border-hairline @container overflow-hidden rounded-lg border">
            {isLoading ? (
              <DataLoading size="table" rows={6} label="加载自选股…" />
            ) : sorted.length === 0 ? (
              <DataEmpty
                size="panel"
                icon={<Star className="size-4" />}
                title={localSearch.trim() ? "没有匹配的自选股" : "还没有自选股"}
                description={
                  localSearch.trim()
                    ? "换个名称或代码试试"
                    : "添加后在这里查看实时行情、涨跌幅与 K 线"
                }
                action={
                  !localSearch.trim() ? (
                    <Button variant="outline" size="sm" onClick={() => setShowAdd(true)}>
                      <Plus />
                      添加自选
                    </Button>
                  ) : undefined
                }
              />
            ) : (
              <div className="overflow-x-auto">
                <table className="data-table">
                  <thead>
                    <tr>
                      <th>名称 / 代码</th>
                      <SortHeader
                        label="最新价"
                        field="price"
                        sortField={sortField}
                        sortDir={sortDir}
                        onSort={toggleSort}
                      />
                      <SortHeader
                        label="涨跌幅"
                        field="change"
                        sortField={sortField}
                        sortDir={sortDir}
                        onSort={toggleSort}
                      />
                      <SortHeader
                        label="总市值"
                        field="market_value"
                        sortField={sortField}
                        sortDir={sortDir}
                        onSort={toggleSort}
                        className="hidden @md:table-cell"
                      />
                      <SortHeader
                        label="流通市值"
                        field="circulation_value"
                        sortField={sortField}
                        sortDir={sortDir}
                        onSort={toggleSort}
                        className="hidden @lg:table-cell"
                      />
                      <th className="w-10 pr-3">
                        <span className="sr-only">操作</span>
                      </th>
                    </tr>
                  </thead>
                  <tbody>
                    {sorted.map((s) => (
                      <StockRow
                        key={s.code}
                        quote={s.quote}
                        name={s.name}
                        code={s.code}
                        onSelect={(v) => openStock(v.symbol, v.name)}
                        onDelete={() => deleteWatchlist.mutate(s.code)}
                      />
                    ))}
                  </tbody>
                </table>
              </div>
            )}
          </section>
        </div>
      </div>

      {/* 独立页面模式：K 线盖在本页内（网格模式由 PaneStockPage 负责） */}
      {pageKline && (
        <div className="bg-canvas absolute inset-0 z-20 overflow-y-auto">
          <KlineDrawer
            symbol={pageKline.symbol}
            name={pageKline.name}
            onClose={() => setPageKline(null)}
          />
        </div>
      )}
    </div>
  )
}

// ── SortHeader ──

function SortHeader({
  label,
  field,
  sortField,
  sortDir,
  onSort,
  className,
}: {
  label: string
  field: SortField
  sortField: SortField
  sortDir: SortDir
  onSort: (field: SortField) => void
  className?: string
}) {
  const active = sortField === field
  return (
    <th className={cn("num", className)}>
      <button
        type="button"
        onClick={() => onSort(field)}
        aria-sort={active ? (sortDir === "asc" ? "ascending" : "descending") : "none"}
        className={`hover:text-secondary inline-flex items-center gap-1 transition-colors ${
          active ? "text-secondary" : ""
        }`}
      >
        {label}
        {active ? (
          sortDir === "asc" ? (
            <ArrowUp className="size-2.5" aria-hidden />
          ) : (
            <ArrowDown className="size-2.5" aria-hidden />
          )
        ) : (
          <ArrowUpDown className="size-2.5 opacity-40" aria-hidden />
        )}
      </button>
    </th>
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
    <tr className="group/row cursor-pointer" onClick={() => onSelect({ symbol: code, name })}>
      <td>
        <div className="flex min-w-0 items-baseline gap-2">
          <span className="text-primary text-body truncate font-medium">{name}</span>
          <span className="text-tertiary text-caption font-number">
            {code.replace(/^[a-z]+/i, "")}
          </span>
        </div>
      </td>

      {loaded ? (
        <>
          <td className="num text-body text-primary font-medium">{quote.last_px?.toFixed(2)}</td>
          <td className={`num text-body font-medium ${up ? "text-up" : "text-down"}`}>
            {up ? "+" : ""}
            {quote.px_change_rate?.toFixed(2)}%
          </td>
          <td className="num text-secondary hidden @md:table-cell">
            {formatCompactNumber(quote.market_value)}
          </td>
          <td className="num text-secondary hidden @lg:table-cell">
            {formatCompactNumber(quote.circulation_value)}
          </td>
        </>
      ) : (
        <>
          <td className="num">
            <span className="bg-hover inline-block h-3.5 w-14 animate-pulse rounded-sm" />
          </td>
          <td className="num">
            <span className="bg-hover inline-block h-3.5 w-12 animate-pulse rounded-sm" />
          </td>
          <td className="num hidden @md:table-cell">
            <span className="bg-hover inline-block h-3.5 w-14 animate-pulse rounded-sm" />
          </td>
          <td className="num hidden @lg:table-cell">
            <span className="bg-hover inline-block h-3.5 w-14 animate-pulse rounded-sm" />
          </td>
        </>
      )}

      <td className="w-10 pr-2">
        <Button
          type="button"
          variant="ghost"
          size="icon-xs"
          aria-label={`删除自选 ${name}`}
          className="text-tertiary hover:text-danger ml-auto opacity-0 transition-opacity group-hover/row:opacity-100 focus-visible:opacity-100"
          onClick={(e) => {
            e.stopPropagation()
            onDelete()
          }}
          title="删除自选"
        >
          <Trash2 />
        </Button>
      </td>
    </tr>
  )
}
