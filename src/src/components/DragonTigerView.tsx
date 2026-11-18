import { useState, useEffect, useCallback } from "react"
import { ChevronDownIcon, Search, X } from "lucide-react"
import { Button } from "@/components/ui/button"
import {
  Command,
  CommandEmpty,
  CommandInput,
  CommandItem,
  CommandList,
} from "@/components/ui/command"
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogHeader,
  DialogTitle,
} from "@/components/ui/dialog"
import { Input } from "@/components/ui/input"
import { Popover, PopoverContent, PopoverTrigger } from "@/components/ui/popover"
import { DataEmpty, DataError, DataLoading } from "@/components/data-state"
import {
  useDragonTigerDetail,
  useDragonTigerStockSeat,
  type DragonTigerDetailItem,
  type SeatDetail,
} from "@/hooks/query/useDragonTiger"
import { formatCompactNumber } from "@/lib/format"

export default function DragonTigerView() {
  const [dept, setDept] = useState<string | null>(null)
  const [date, setDate] = useState("")
  const {
    data: resp,
    isLoading,
    isFetching,
    error,
    refetch,
  } = useDragonTigerDetail(date || undefined)
  const [deptOpen, setDeptOpen] = useState(false)
  const [search, setSearch] = useState("")
  const [selected, setSelected] = useState<DragonTigerDetailItem | null>(null)
  const [selectedDate, setSelectedDate] = useState<string | null>(null)
  const activeDate = date || resp?.date || ""

  const seatDate = selectedDate ?? selected?.trade_date ?? null
  const { data: buySeats = [], isLoading: buyLoading } = useDragonTigerStockSeat(
    selected?.code ?? null,
    seatDate ? seatDate.slice(0, 10).replace(/-/g, "") : null,
    "buy"
  )
  const { data: sellSeats = [], isLoading: sellLoading } = useDragonTigerStockSeat(
    selected?.code ?? null,
    seatDate ? seatDate.slice(0, 10).replace(/-/g, "") : null,
    "sell"
  )

  useEffect(() => {
    const onKey = (e: KeyboardEvent) => {
      if (e.key === "Escape") {
        setSelected(null)
        setSelectedDate(null)
      }
    }
    document.addEventListener("keydown", onKey)
    return () => document.removeEventListener("keydown", onKey)
  }, [])

  const items = resp?.data ?? []
  const deptItems = dept ? items.filter((s) => s.dept_names?.includes(dept)) : items
  const filtered = deptItems.filter((s) => {
    if (!search) return true
    const query = search.toLowerCase()
    return s.name.toLowerCase().includes(query) || s.code.toLowerCase().includes(query)
  })
  const total = filtered.length
  const totalNetAmt = filtered.reduce((sum, s) => sum + (s.billboard_net_amt ?? 0), 0)
  const netBuyCount = filtered.filter((s) => (s.billboard_net_amt ?? 0) > 0).length
  const netSellCount = filtered.filter((s) => (s.billboard_net_amt ?? 0) < 0).length
  const hasFilter = Boolean(search) || Boolean(dept)

  const selectedReasons = selected?.reasons?.length
    ? selected.reasons
    : selected?.explanation
      ? [selected.explanation]
      : []

  const closeSelected = useCallback(() => {
    setSelected(null)
    setSelectedDate(null)
  }, [])

  const toggleSelected = useCallback(
    (item: DragonTigerDetailItem) => {
      if (selected?.code === item.code) {
        closeSelected()
        return
      }
      setSelected(item)
      setSelectedDate(item.trade_date)
    },
    [selected?.code, closeSelected]
  )

  const handleDeptChange = useCallback(
    (next: string | null) => {
      setDept(next)
      closeSelected()
    },
    [closeSelected]
  )

  const handleDateChange = useCallback(
    (next: string) => {
      setDate(next)
      closeSelected()
    },
    [closeSelected]
  )

  if (isLoading && !resp) {
    return <DataLoading label="加载龙虎榜…" size="panel" rows={7} />
  }

  if (error) {
    return (
      <DataError
        title="龙虎榜加载失败"
        description={error.message}
        onRetry={() => void refetch()}
      />
    )
  }

  const stats = [
    {
      label: "上榜家数",
      value: String(total),
      note: hasFilter ? `筛选自 ${items.length} 只` : "今日",
    },
    {
      label: "总净买入",
      value: `${totalNetAmt >= 0 ? "+" : ""}${(totalNetAmt / 1e8).toFixed(2)} 亿`,
      tone: totalNetAmt >= 0 ? "text-up" : "text-down",
      note: "上榜个股合计",
    },
    {
      label: "净买入家数",
      value: String(netBuyCount),
      note: `占比 ${total > 0 ? ((netBuyCount / total) * 100).toFixed(0) : 0}%`,
    },
    {
      label: "净卖出家数",
      value: String(netSellCount),
      note: `占比 ${total > 0 ? ((netSellCount / total) * 100).toFixed(0) : 0}%`,
    },
  ]

  return (
    <>
      <div className="@container mx-auto w-full max-w-5xl px-4 py-5 @2xl:p-6">
        <header className="flex flex-wrap items-center justify-between gap-4 pb-4">
          <p className="text-caption text-tertiary">机构与游资席位</p>
          <Input
            type="date"
            value={activeDate}
            onChange={(e) => handleDateChange(e.target.value)}
            aria-label="选择龙虎榜日期"
            className="bg-surface text-caption h-8 w-36 rounded-md"
          />
        </header>

        <div className="bg-hairline border-hairline grid grid-cols-2 gap-px overflow-hidden rounded-lg border @md:grid-cols-4">
          {stats.map((s) => (
            <div key={s.label} className="bg-surface px-4 py-3.5">
              <p className="text-tertiary text-micro">{s.label}</p>
              <p
                className={`font-number text-h1 mt-1.5 font-semibold tracking-[-0.01em] ${s.tone ?? "text-primary"}`}
              >
                {s.value}
              </p>
              <p className="text-tertiary text-micro mt-1">{s.note}</p>
            </div>
          ))}
        </div>

        <section className="bg-surface border-hairline mt-5 overflow-hidden rounded-lg border">
          <div className="border-hairline flex flex-wrap items-center gap-x-3 gap-y-2 border-b px-4 py-2.5">
            <h2 className="text-caption text-primary font-medium">上榜个股</h2>
            <span className="text-caption text-tertiary">
              {hasFilter ? `${filtered.length} / ${items.length}` : `${items.length} 只`}
            </span>
            {isFetching && <span className="text-caption text-tertiary">更新中…</span>}

            <div className="ml-auto flex flex-wrap items-center justify-end gap-2">
              <div className="relative">
                <Search className="text-tertiary pointer-events-none absolute top-1/2 left-2.5 size-3.5 -translate-y-1/2" />
                <Input
                  type="text"
                  placeholder="名称或代码…"
                  aria-label="搜索个股"
                  value={search}
                  onChange={(e) => setSearch(e.target.value)}
                  className="bg-canvas text-caption h-8 w-36 rounded-md pl-8"
                />
              </div>
              <Popover open={deptOpen} onOpenChange={setDeptOpen}>
                <PopoverTrigger asChild>
                  <Button
                    type="button"
                    variant="outline"
                    aria-haspopup="listbox"
                    className="bg-canvas border-hairline text-caption hover:bg-hover h-8 w-44 justify-between px-2.5 font-normal"
                  >
                    <span className="truncate" title={dept ?? "全部营业部"}>
                      {dept ?? "全部营业部"}
                    </span>
                    <ChevronDownIcon className="size-3.5 shrink-0 opacity-50" />
                  </Button>
                </PopoverTrigger>
                <PopoverContent
                  align="end"
                  className="bg-surface border-hairline shadow-modal w-80 max-w-[calc(100vw-2rem)] gap-0 rounded-lg border p-0 ring-0"
                >
                  <Command className="rounded-lg!">
                    <CommandInput placeholder="搜索营业部…" />
                    <CommandList className="max-h-72">
                      <CommandEmpty className="text-secondary text-caption py-4">
                        无匹配营业部
                      </CommandEmpty>
                      <CommandItem
                        value="全部营业部"
                        data-checked={dept === null}
                        onSelect={() => {
                          handleDeptChange(null)
                          setDeptOpen(false)
                        }}
                        className="text-caption"
                      >
                        <span className="truncate">全部营业部</span>
                      </CommandItem>
                      {(resp?.depts ?? []).map((name) => (
                        <CommandItem
                          key={name}
                          value={name}
                          data-checked={dept === name}
                          onSelect={() => {
                            handleDeptChange(name)
                            setDeptOpen(false)
                          }}
                          className="text-caption"
                        >
                          <span className="truncate">{name}</span>
                        </CommandItem>
                      ))}
                    </CommandList>
                  </Command>
                </PopoverContent>
              </Popover>
              {dept && (
                <Button
                  type="button"
                  variant="ghost"
                  size="icon-xs"
                  onClick={() => handleDeptChange(null)}
                  className="text-tertiary hover:text-primary"
                  aria-label="清除营业部筛选"
                  title="清除筛选"
                >
                  <X />
                </Button>
              )}
            </div>
          </div>

          <div className="overflow-x-auto">
            <table className="data-table">
              <thead>
                <tr>
                  <th>个股</th>
                  <th className="num">净买入</th>
                  <th className="num">涨跌幅</th>
                  <th className="hidden @2xl:table-cell">上榜原因</th>
                </tr>
              </thead>
              <tbody>
                {filtered.length === 0 ? (
                  <tr>
                    <td colSpan={4} className="p-0">
                      <DataEmpty
                        size="table"
                        title="没有符合条件的个股"
                        description={hasFilter ? "调整筛选条件再试" : "当前日期暂无龙虎榜数据"}
                      />
                    </td>
                  </tr>
                ) : (
                  filtered.map((s) => {
                    const up = (s.billboard_net_amt ?? 0) >= 0
                    const dateLabel = (
                      s.trade_dates?.length
                        ? [...s.trade_dates].reverse()
                        : [s.trade_date.slice(0, 10)]
                    )
                      .map((date) => date.slice(5))
                      .join("、")
                    const reasons = s.reasons?.length
                      ? s.reasons
                      : s.explanation
                        ? [s.explanation]
                        : []
                    const isSelected = selected?.code === s.code
                    return (
                      <tr
                        key={s.code}
                        className={`cursor-pointer ${isSelected ? "bg-brand/5" : ""}`}
                        onClick={() => toggleSelected(s)}
                      >
                        <td>
                          <div className="flex min-w-0 flex-col gap-0.5">
                            <button
                              type="button"
                              aria-expanded={isSelected}
                              className="text-primary hover:text-brand text-body cursor-pointer truncate text-left font-medium transition-colors"
                              onClick={(e) => {
                                e.stopPropagation()
                                toggleSelected(s)
                              }}
                            >
                              {s.name}
                            </button>
                            <span className="text-tertiary text-micro font-number whitespace-nowrap">
                              {s.code} · {dateLabel}
                            </span>
                          </div>
                        </td>
                        <td className={`num font-medium ${up ? "text-up" : "text-down"}`}>
                          {s.billboard_net_amt == null ? (
                            <span className="text-tertiary">—</span>
                          ) : (
                            <>
                              {up ? "+" : ""}
                              {(s.billboard_net_amt / 1e8).toFixed(2)} 亿
                            </>
                          )}
                        </td>
                        <td
                          className={`num font-medium ${s.change_rate == null ? "" : s.change_rate >= 0 ? "text-up" : "text-down"}`}
                        >
                          {s.change_rate == null ? (
                            <span className="text-tertiary">—</span>
                          ) : (
                            <>
                              {s.change_rate >= 0 ? "+" : ""}
                              {s.change_rate.toFixed(2)}%
                            </>
                          )}
                        </td>
                        <td className="hidden @2xl:table-cell">
                          {reasons.length === 0 ? (
                            <span className="text-tertiary">—</span>
                          ) : (
                            <div className="text-secondary flex max-w-64 flex-col gap-0.5 leading-snug whitespace-normal">
                              {reasons.map((reason) => (
                                <span key={reason}>{reason}</span>
                              ))}
                            </div>
                          )}
                        </td>
                      </tr>
                    )
                  })
                )}
              </tbody>
            </table>
          </div>
        </section>
      </div>

      {/* Detail Modal */}
      {selected && (
        <Dialog
          open
          onOpenChange={(open) => {
            if (!open) closeSelected()
          }}
        >
          <DialogContent className="bg-surface border-hairline w-modal-lg shadow-modal flex max-h-[90vh] max-w-[95vw] flex-col gap-0 overflow-hidden rounded-lg border p-0 sm:max-w-[95vw]">
            <DialogHeader className="border-hairline flex shrink-0 flex-row items-center border-b px-6 py-4">
              <DialogTitle className="text-h2 text-primary pr-10 font-medium">
                {selected.name}
                <span className="text-tertiary text-caption font-number ml-2 font-normal">
                  {selected.code}
                </span>
                <span className="text-tertiary text-caption font-number ml-3 font-normal">
                  {seatDate?.slice(0, 10)}
                </span>
              </DialogTitle>
              <DialogDescription className="sr-only">龙虎榜个股详情</DialogDescription>
            </DialogHeader>
            <div className="min-h-0 flex-1 overflow-y-auto p-6">
              {(selected.trade_dates?.length ?? 0) > 1 && (
                <div className="mb-5 flex flex-wrap items-center gap-2">
                  <span className="text-tertiary text-caption">席位日期</span>
                  {selected.trade_dates?.map((date) => (
                    <Button
                      key={date}
                      type="button"
                      variant={selectedDate === date ? "secondary" : "ghost"}
                      size="xs"
                      onClick={() => setSelectedDate(date)}
                      className="text-caption font-number h-auto px-2 py-0.5"
                    >
                      {date.slice(5)}
                    </Button>
                  ))}
                </div>
              )}

              <div className="bg-hairline border-hairline grid grid-cols-2 gap-px overflow-hidden rounded-lg border sm:grid-cols-4">
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
                  <div key={m.l} className="bg-canvas px-3 py-3">
                    <p className="text-tertiary text-micro">{m.l}</p>
                    <p
                      className={`font-number text-body mt-1 font-medium ${
                        m.up ? "text-up" : "text-primary"
                      }`}
                    >
                      {m.v}
                    </p>
                  </div>
                ))}
              </div>

              {selectedReasons.length > 0 && (
                <div className="mt-5">
                  <h4 className="text-tertiary text-micro mb-2">上榜原因</h4>
                  <ul className="text-secondary text-caption space-y-1.5 leading-[1.7]">
                    {selectedReasons.map((reason) => (
                      <li key={reason} className="flex gap-2">
                        <span
                          className="bg-brand/50 mt-2 size-1 shrink-0 rounded-full"
                          aria-hidden
                        />
                        {reason}
                      </li>
                    ))}
                  </ul>
                </div>
              )}

              {buyLoading || sellLoading ? (
                <DataLoading label="加载席位数据…" size="compact" />
              ) : (
                <div className="mt-6 space-y-6">
                  {buySeats.length > 0 && (
                    <div>
                      <h4 className="text-up text-caption mb-2 font-medium">
                        买方席位
                        <span className="text-tertiary font-number ml-1.5 font-normal">
                          {buySeats.length}
                        </span>
                      </h4>
                      <SeatTable seats={buySeats} side="buy" />
                    </div>
                  )}
                  {sellSeats.length > 0 && (
                    <div>
                      <h4 className="text-down text-caption mb-2 font-medium">
                        卖方席位
                        <span className="text-tertiary font-number ml-1.5 font-normal">
                          {sellSeats.length}
                        </span>
                      </h4>
                      <SeatTable seats={sellSeats} side="sell" />
                    </div>
                  )}
                </div>
              )}
            </div>
          </DialogContent>
        </Dialog>
      )}
    </>
  )
}

function SeatTable({ seats, side }: { seats: SeatDetail[]; side: "buy" | "sell" }) {
  const isBuy = side === "buy"
  return (
    <div className="border-hairline overflow-hidden rounded-md border">
      <table className="data-table">
        <thead>
          <tr>
            <th>营业部</th>
            <th className="num">{isBuy ? "买入(万)" : "卖出(万)"}</th>
            <th className="num">{isBuy ? "买入占比" : "卖出占比"}</th>
          </tr>
        </thead>
        <tbody>
          {seats.map((s, i) => {
            const amount = isBuy ? s.buy_amt : s.sell_amt
            const ratio = isBuy ? s.buy_ratio : s.sell_ratio
            return (
              <tr key={i}>
                <td>
                  <span className="flex items-center gap-2">
                    {s.dept_name}
                    {s.seat_type === "机构" ? (
                      <span className="bg-hover text-secondary text-micro rounded-sm px-1.5 py-0.5">
                        机构
                      </span>
                    ) : null}
                  </span>
                </td>
                <td className={`num font-medium ${isBuy ? "text-up" : "text-down"}`}>
                  {amount ? (amount / 1e4).toFixed(0) : "—"}
                </td>
                <td className="num text-secondary">
                  {ratio != null ? `${(ratio * 100).toFixed(1)}%` : "—"}
                </td>
              </tr>
            )
          })}
        </tbody>
      </table>
    </div>
  )
}
