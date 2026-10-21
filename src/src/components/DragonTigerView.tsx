import { useState, useEffect, useCallback } from "react"
import { Button } from "@/components/ui/button"
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogHeader,
  DialogTitle,
} from "@/components/ui/dialog"
import { Input } from "@/components/ui/input"
import {
  useDragonTigerDetail,
  useDragonTigerStockSeat,
  type DragonTigerDetailItem,
} from "@/hooks/query/useDragonTiger"
import { formatCompactNumber } from "@/lib/format"

export default function DragonTigerView() {
  const [page, setPage] = useState(1)
  const pageSize = 20
  const { data: resp, isLoading, error } = useDragonTigerDetail(page, pageSize)
  const [dirFilter, setDirFilter] = useState<"all" | "buy" | "sell">("all")
  const [search, setSearch] = useState("")
  const [selected, setSelected] = useState<DragonTigerDetailItem | null>(null)

  const { data: buySeats = [], isLoading: buyLoading } = useDragonTigerStockSeat(
    selected?.code ?? null,
    selected?.trade_date ? selected.trade_date.slice(0, 10).replace(/-/g, "") : null,
    "buy"
  )
  const { data: sellSeats = [], isLoading: sellLoading } = useDragonTigerStockSeat(
    selected?.code ?? null,
    selected?.trade_date ? selected.trade_date.slice(0, 10).replace(/-/g, "") : null,
    "sell"
  )

  useEffect(() => {
    const onKey = (e: KeyboardEvent) => {
      if (e.key === "Escape") setSelected(null)
    }
    document.addEventListener("keydown", onKey)
    return () => document.removeEventListener("keydown", onKey)
  }, [])

  const items = resp?.data ?? []
  const total = resp?.total ?? 0
  const totalPages = Math.max(1, Math.ceil(total / pageSize))

  const filtered = items.filter((s) => {
    if (dirFilter === "buy" && (s.billboard_net_amt ?? 0) <= 0) return false
    if (dirFilter === "sell" && (s.billboard_net_amt ?? 0) >= 0) return false
    if (
      search &&
      !s.name.toLowerCase().includes(search.toLowerCase()) &&
      !s.code.toLowerCase().includes(search.toLowerCase())
    ) {
      return false
    }
    return true
  })

  const goPrev = useCallback(() => {
    setPage((p) => Math.max(1, p - 1))
    setSelected(null)
  }, [])
  const goNext = useCallback(() => {
    setPage((p) => Math.min(totalPages, p + 1))
    setSelected(null)
  }, [totalPages])
  const goPage = useCallback((p: number) => {
    setPage(p)
    setSelected(null)
  }, [])

  if (isLoading) {
    return (
      <div
        className="bg-canvas flex min-h-screen items-center justify-center pt-11"
        style={{ height: "calc(100vh - 44px)" }}
      >
        <div className="text-secondary text-caption flex items-center gap-3">
          <span className="bg-brand size-2 animate-pulse rounded-full" />
          加载龙虎榜数据...
        </div>
      </div>
    )
  }

  if (error) {
    return (
      <div
        className="bg-canvas flex min-h-screen items-center justify-center pt-11"
        style={{ height: "calc(100vh - 44px)" }}
      >
        <p className="text-secondary text-caption">{error.message}</p>
      </div>
    )
  }

  const stats = [
    {
      l: "上榜家数",
      v: String(total),
      hl: true,
    },
    {
      l: "总净买入",
      v: `${((resp?.total_net_amt ?? 0) / 1e8).toFixed(2)}亿`,
      cl: (resp?.total_net_amt ?? 0) >= 0 ? "var(--color-up)" : "var(--color-down)",
    },
    {
      l: "净买入家数",
      v: String(resp?.net_buy_count ?? 0),
      s: `占比${total > 0 ? (((resp?.net_buy_count ?? 0) / total) * 100).toFixed(0) : 0}%`,
    },
    {
      l: "净卖出家数",
      v: String(resp?.net_sell_count ?? 0),
      s: `占比${total > 0 ? (((resp?.net_sell_count ?? 0) / total) * 100).toFixed(0) : 0}%`,
    },
  ]

  const th = {
    textAlign: "left" as const,
    padding: "9px 10px",
    fontSize: "var(--font-size-caption)",
    fontWeight: 500,
    color: "var(--color-tertiary)",
    borderBottom: "1px solid var(--color-border)",
    background: "rgb(var(--color-fg-rgb) / 0.04)",
    position: "sticky" as const,
    top: 0,
    zIndex: 1,
    whiteSpace: "nowrap" as const,
  }
  const td = {
    padding: "7px 10px",
    borderBottom: "1px solid rgb(var(--color-fg-rgb) / 0.04)",
    whiteSpace: "nowrap" as const,
  }

  return (
    <>
      <div className="min-h-full w-full p-4">
        <div className="mb-4 grid grid-cols-2 gap-3 md:grid-cols-4">
          {stats.map((s) => (
            <div
              key={s.l}
              className={`bg-surface rounded-lg border px-4 py-4 text-center ${s.hl ? "border-brand bg-brand/5" : "border-hairline"}`}
            >
              <div className="text-secondary text-caption mb-1">{s.l}</div>
              <div
                className="font-number text-h1 text-primary font-semibold"
                style={s.cl ? { color: s.cl } : undefined}
              >
                {s.v}
              </div>
              {s.s && <div className="text-secondary text-caption mt-0.5">{s.s}</div>}
            </div>
          ))}
        </div>

        <div className="grid grid-cols-1 gap-4 md:grid-cols-[220px_1fr]">
          <div className="bg-surface border-hairline order-2 overflow-hidden rounded-lg border md:order-0">
            <div className="border-hairline text-caption text-primary border-b px-4 py-3 font-medium">
              筛选
            </div>
            <div className="p-3">
              <div className="mb-3">
                <div className="text-secondary text-caption mb-2 font-semibold">搜索</div>
                <Input
                  type="text"
                  placeholder="名称或代码…"
                  value={search}
                  onChange={(e) => setSearch(e.target.value)}
                  className="bg-canvas border-hairline focus-visible:border-brand text-caption"
                />
              </div>
              <div>
                <div className="text-secondary text-caption mb-2 font-semibold">净买卖</div>
                <div className="flex flex-wrap gap-2">
                  {(["all", "buy", "sell"] as const).map((d) => {
                    const labels = {
                      all: "全部",
                      buy: "净买入",
                      sell: "净卖出",
                    }
                    const activeCls =
                      d === "buy"
                        ? "bg-up border-up text-primary"
                        : d === "sell"
                          ? "bg-down border-down text-primary"
                          : "bg-brand border-brand text-primary"
                    return (
                      <Button
                        key={d}
                        variant="outline"
                        size="xs"
                        onClick={() => setDirFilter(d)}
                        className={`text-caption h-auto px-3 py-1 ${dirFilter === d ? activeCls : "border-hairline text-secondary hover:text-primary"}`}
                      >
                        {labels[d]}
                      </Button>
                    )
                  })}
                </div>
              </div>
            </div>
          </div>

          <div className="bg-surface border-hairline order-1 overflow-hidden rounded-lg border md:order-0">
            <div className="border-hairline flex items-center justify-between gap-3 border-b px-4 py-3">
              <h3 className="text-caption text-primary font-medium">龙虎榜上榜个股</h3>
              <span className="text-secondary text-caption">
                第{page}/{totalPages}页 · {total} 只
              </span>
            </div>
            <div className="overflow-auto" style={{ maxHeight: "62vh" }}>
              <table className="w-full border-collapse">
                <thead>
                  <tr>
                    <th style={th}>名称</th>
                    <th style={th}>净买入(万)</th>
                    <th style={th}>涨跌幅</th>
                    <th style={th}>上榜原因</th>
                  </tr>
                </thead>
                <tbody>
                  {filtered.length === 0 ? (
                    <tr>
                      <td
                        colSpan={4}
                        style={{
                          textAlign: "center",
                          padding: "36px",
                          color: "var(--color-tertiary)",
                        }}
                      >
                        无匹配数据
                      </td>
                    </tr>
                  ) : (
                    filtered.map((s, i) => {
                      const up = (s.billboard_net_amt ?? 0) >= 0
                      return (
                        <tr
                          key={`${s.code}-${i}`}
                          className={`[&>td]:hover:bg-hover cursor-pointer transition-colors ${selected?.code === s.code ? "[&>td]:!bg-brand/10" : ""}`}
                          onClick={() => setSelected(selected?.code === s.code ? null : s)}
                        >
                          <td style={td}>
                            <button
                              type="button"
                              aria-expanded={selected?.code === s.code}
                              className="text-brand cursor-pointer font-medium hover:underline"
                              onClick={(e) => {
                                e.stopPropagation()
                                setSelected(selected?.code === s.code ? null : s)
                              }}
                            >
                              {s.name}
                            </button>
                            <span className="text-secondary text-caption ml-1">{s.code}</span>
                          </td>
                          <td style={td}>
                            <span
                              className={`font-number text-caption font-semibold ${up ? "text-up" : "text-down"}`}
                            >
                              {up ? "+" : ""}
                              {((s.billboard_net_amt ?? 0) / 1e8).toFixed(2)}亿
                            </span>
                          </td>
                          <td style={td}>
                            <span
                              className={`font-number text-caption font-semibold ${(s.change_rate ?? 0) >= 0 ? "text-up" : "text-down"}`}
                            >
                              {(s.change_rate ?? 0) >= 0 ? "+" : ""}
                              {s.change_rate?.toFixed(2) ?? "—"}%
                            </span>
                          </td>
                          <td style={td}>
                            <span className="text-secondary text-caption inline-block max-w-[200px] whitespace-normal">
                              {s.explanation || "—"}
                            </span>
                          </td>
                        </tr>
                      )
                    })
                  )}
                </tbody>
              </table>
            </div>
            {totalPages > 1 && (
              <div className="border-hairline flex items-center justify-center gap-2 border-t px-4 py-3">
                <Button
                  variant="outline"
                  size="xs"
                  onClick={goPrev}
                  disabled={page <= 1}
                  className="text-caption h-auto px-3 py-1"
                >
                  上一页
                </Button>
                {(() => {
                  const pages: (number | "...")[] = []
                  const maxVisible = 5
                  if (totalPages <= maxVisible + 2) {
                    for (let i = 1; i <= totalPages; i++) pages.push(i)
                  } else {
                    pages.push(1)
                    if (page > 3) pages.push("...")
                    const start = Math.max(2, page - 1)
                    const end = Math.min(totalPages - 1, page + 1)
                    for (let i = start; i <= end; i++) pages.push(i)
                    if (page < totalPages - 2) pages.push("...")
                    pages.push(totalPages)
                  }
                  return pages.map((p, i) =>
                    p === "..." ? (
                      <span key={`e${i}`} className="text-secondary text-caption px-1">
                        ...
                      </span>
                    ) : (
                      <Button
                        key={p}
                        variant="outline"
                        size="xs"
                        onClick={() => goPage(p)}
                        className={`text-caption h-auto px-3 py-1 ${page === p ? "border-brand bg-brand text-primary hover:bg-brand hover:text-primary" : "border-hairline text-secondary hover:text-primary"}`}
                      >
                        {p}
                      </Button>
                    )
                  )
                })()}
                <Button
                  variant="outline"
                  size="xs"
                  onClick={goNext}
                  disabled={page >= totalPages}
                  className="text-caption h-auto px-3 py-1"
                >
                  下一页
                </Button>
              </div>
            )}
          </div>
        </div>

        {/* Detail Modal */}
        {selected && (
          <Dialog
            open
            onOpenChange={(open) => {
              if (!open) setSelected(null)
            }}
          >
            <DialogContent className="bg-surface border-hairline w-modal-lg shadow-modal flex max-h-[90vh] max-w-[95vw] flex-col gap-0 overflow-hidden rounded-lg border p-0 sm:max-w-[95vw]">
              <DialogHeader className="border-hairline flex shrink-0 flex-row items-center border-b px-5 py-4">
                <DialogTitle className="text-h2 text-primary pr-10 font-medium">
                  {selected.name} ({selected.code})
                </DialogTitle>
                <DialogDescription className="sr-only">龙虎榜个股详情</DialogDescription>
              </DialogHeader>
              <div className="min-h-0 flex-1 overflow-y-auto p-6">
                {selected.explanation && (
                  <div className="mb-4">
                    <h4 className="text-caption text-primary mb-2 font-medium">上榜原因</h4>
                    <p className="text-body text-secondary">{selected.explanation}</p>
                  </div>
                )}
                <div className="mb-4 grid grid-cols-2 gap-3">
                  {[
                    { l: "收盘价", v: selected.close_price?.toFixed(2) || "—" },
                    {
                      l: "涨跌幅",
                      v: selected.change_rate != null ? `${selected.change_rate.toFixed(2)}%` : "—",
                      up: (selected.change_rate || 0) >= 0,
                    },
                    {
                      l: "换手率",
                      v:
                        selected.turnover_rate != null
                          ? `${selected.turnover_rate.toFixed(2)}%`
                          : "—",
                    },
                    {
                      l: "流通市值",
                      v: selected.free_market_cap
                        ? formatCompactNumber(selected.free_market_cap)
                        : "—",
                    },
                  ].map((m) => (
                    <div key={m.l} className="bg-ink-dim rounded-lg px-3 py-2 text-center">
                      <div className="text-secondary text-caption">{m.l}</div>
                      <div
                        className={`font-number text-caption font-semibold ${m.up ? "text-up" : "text-primary"}`}
                      >
                        {m.v}
                      </div>
                    </div>
                  ))}
                </div>
                {buyLoading || sellLoading ? (
                  <div className="flex items-center justify-center py-8">
                    <span className="bg-brand size-2 animate-pulse rounded-full" />
                    <span className="text-secondary text-caption ml-2">加载席位数据...</span>
                  </div>
                ) : (
                  <>
                    {buySeats.length > 0 && (
                      <div className="mb-4">
                        <h4 className="text-up text-caption mb-2 font-medium">
                          买方席位 ({buySeats.length})
                        </h4>
                        <table className="text-caption w-full border-collapse">
                          <thead>
                            <tr>
                              <th style={th}>营业部</th>
                              <th style={th}>买入(万)</th>
                              <th style={th}>买入占比</th>
                            </tr>
                          </thead>
                          <tbody>
                            {buySeats.map((s, i) => (
                              <tr key={i}>
                                <td style={td}>
                                  {s.dept_name}
                                  {s.seat_type === "机构" ? (
                                    <span className="bg-up/10 text-up text-caption ml-1 rounded px-2 py-0.5">
                                      机构
                                    </span>
                                  ) : null}
                                </td>
                                <td
                                  style={{
                                    ...td,
                                    fontFamily: "Geist Mono Variable, monospace",
                                    color: "var(--color-up)",
                                  }}
                                >
                                  {s.buy_amt ? (s.buy_amt / 1e4).toFixed(0) : "—"}
                                </td>
                                <td
                                  style={{
                                    ...td,
                                    fontFamily: "Geist Mono Variable, monospace",
                                  }}
                                >
                                  {s.buy_ratio != null ? `${(s.buy_ratio * 100).toFixed(1)}%` : "—"}
                                </td>
                              </tr>
                            ))}
                          </tbody>
                        </table>
                      </div>
                    )}
                    {sellSeats.length > 0 && (
                      <div className="mb-4">
                        <h4 className="text-down text-caption mb-2 font-medium">
                          卖方席位 ({sellSeats.length})
                        </h4>
                        <table className="text-caption w-full border-collapse">
                          <thead>
                            <tr>
                              <th style={th}>营业部</th>
                              <th style={th}>卖出(万)</th>
                              <th style={th}>卖出占比</th>
                            </tr>
                          </thead>
                          <tbody>
                            {sellSeats.map((s, i) => (
                              <tr key={i}>
                                <td style={td}>
                                  {s.dept_name}
                                  {s.seat_type === "机构" ? (
                                    <span className="bg-up/10 text-up text-caption ml-1 rounded px-2 py-0.5">
                                      机构
                                    </span>
                                  ) : null}
                                </td>
                                <td
                                  style={{
                                    ...td,
                                    fontFamily: "Geist Mono Variable, monospace",
                                    color: "var(--color-down)",
                                  }}
                                >
                                  {s.sell_amt ? (s.sell_amt / 1e4).toFixed(0) : "—"}
                                </td>
                                <td
                                  style={{
                                    ...td,
                                    fontFamily: "Geist Mono Variable, monospace",
                                  }}
                                >
                                  {s.sell_ratio != null
                                    ? `${(s.sell_ratio * 100).toFixed(1)}%`
                                    : "—"}
                                </td>
                              </tr>
                            ))}
                          </tbody>
                        </table>
                      </div>
                    )}
                  </>
                )}
                {selected.d1_chg != null && (
                  <div className="mb-4">
                    <h4 className="text-caption text-primary mb-2 font-medium">上榜后表现</h4>
                    <div className="grid grid-cols-4 gap-2">
                      {[
                        { l: "T+1", v: selected.d1_chg },
                        { l: "T+2", v: selected.d2_chg },
                        { l: "T+5", v: selected.d5_chg },
                        { l: "T+10", v: selected.d10_chg },
                      ].map((d, i) => {
                        const dv = d.v || 0
                        return (
                          <div key={i} className="bg-ink-dim rounded-lg px-3 py-2 text-center">
                            <div className="text-secondary text-caption">{d.l}</div>
                            <div
                              className={`font-number text-caption font-semibold ${dv >= 0 ? "text-up" : "text-down"}`}
                            >
                              {dv >= 0 ? "+" : ""}
                              {dv.toFixed(2)}%
                            </div>
                          </div>
                        )
                      })}
                    </div>
                  </div>
                )}
              </div>
            </DialogContent>
          </Dialog>
        )}
      </div>
    </>
  )
}
