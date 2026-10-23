import { useState, useEffect, useCallback } from "react"
import { ChevronDownIcon } from "lucide-react"
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
import {
  useDragonTigerDetail,
  useDragonTigerStockSeat,
  type DragonTigerDetailItem,
} from "@/hooks/query/useDragonTiger"
import { formatCompactNumber } from "@/lib/format"

export default function DragonTigerView() {
  const [dept, setDept] = useState<string | null>(null)
  const [date, setDate] = useState("")
  const { data: resp, isLoading, isFetching, error } = useDragonTigerDetail(date || undefined)
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
      v: `${(totalNetAmt / 1e8).toFixed(2)}亿`,
      cl: totalNetAmt >= 0 ? "var(--color-up)" : "var(--color-down)",
    },
    {
      l: "净买入家数",
      v: String(netBuyCount),
      s: `占比${total > 0 ? ((netBuyCount / total) * 100).toFixed(0) : 0}%`,
    },
    {
      l: "净卖出家数",
      v: String(netSellCount),
      s: `占比${total > 0 ? ((netSellCount / total) * 100).toFixed(0) : 0}%`,
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

        <div className="bg-surface border-hairline overflow-hidden rounded-lg border">
          <div className="border-hairline flex flex-wrap items-center gap-x-3 gap-y-3 border-b px-4 py-3">
            <h3 className="text-caption text-primary font-medium whitespace-nowrap">
              龙虎榜上榜个股
            </h3>
            <span className="text-secondary text-caption whitespace-nowrap">
              {hasFilter ? `${filtered.length} / ${items.length} 只` : `共 ${items.length} 只`}
            </span>
            {isFetching && (
              <span className="text-secondary text-caption whitespace-nowrap">更新中…</span>
            )}
            <div className="ml-auto flex flex-wrap items-center justify-end gap-2">
              <Input
                type="date"
                value={activeDate}
                onChange={(e) => handleDateChange(e.target.value)}
                aria-label="选择龙虎榜日期"
                className="bg-canvas border-hairline focus-visible:border-brand text-caption h-8 w-36"
              />
              <Input
                type="text"
                placeholder="名称或代码…"
                value={search}
                onChange={(e) => setSearch(e.target.value)}
                className="bg-canvas border-hairline focus-visible:border-brand text-caption h-8 w-40 md:w-48"
              />
              <Popover open={deptOpen} onOpenChange={setDeptOpen}>
                <PopoverTrigger asChild>
                  <Button
                    type="button"
                    variant="outline"
                    aria-haspopup="listbox"
                    className="bg-canvas border-hairline text-caption hover:bg-hover h-8 w-48 justify-between px-2.5 font-normal"
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
                  size="xs"
                  onClick={() => handleDeptChange(null)}
                  className="text-caption text-secondary hover:text-primary h-8 px-2"
                >
                  清除
                </Button>
              )}
            </div>
          </div>
          <div className="overflow-x-auto">
            <table className="w-full border-collapse">
              <thead>
                <tr>
                  <th style={th}>名称</th>
                  <th style={th}>净买入(亿)</th>
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
                    return (
                      <tr
                        key={s.code}
                        className={`[&>td]:hover:bg-hover cursor-pointer transition-colors ${selected?.code === s.code ? "[&>td]:!bg-brand/10" : ""}`}
                        onClick={() => toggleSelected(s)}
                      >
                        <td style={td}>
                          <button
                            type="button"
                            aria-expanded={selected?.code === s.code}
                            className="text-brand cursor-pointer font-medium hover:underline"
                            onClick={(e) => {
                              e.stopPropagation()
                              toggleSelected(s)
                            }}
                          >
                            {s.name}
                          </button>
                          <span className="text-secondary text-caption ml-1">{s.code}</span>
                          <div className="text-secondary text-caption font-number mt-0.5">
                            {dateLabel}
                          </div>
                        </td>
                        <td style={td}>
                          {s.billboard_net_amt == null ? (
                            <span className="text-secondary text-caption">—</span>
                          ) : (
                            <span
                              className={`font-number text-caption font-semibold ${up ? "text-up" : "text-down"}`}
                            >
                              {up ? "+" : ""}
                              {(s.billboard_net_amt / 1e8).toFixed(2)}亿
                            </span>
                          )}
                        </td>
                        <td style={td}>
                          {s.change_rate == null ? (
                            <span className="text-secondary text-caption">—</span>
                          ) : (
                            <span
                              className={`font-number text-caption font-semibold ${s.change_rate >= 0 ? "text-up" : "text-down"}`}
                            >
                              {s.change_rate >= 0 ? "+" : ""}
                              {s.change_rate.toFixed(2)}%
                            </span>
                          )}
                        </td>
                        <td style={td}>
                          {reasons.length === 0 ? (
                            <span className="text-secondary text-caption">—</span>
                          ) : (
                            <div className="text-secondary text-caption flex max-w-[260px] flex-col gap-0.5 leading-snug whitespace-normal">
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
              <DialogHeader className="border-hairline flex shrink-0 flex-row items-center border-b px-5 py-4">
                <DialogTitle className="text-h2 text-primary pr-10 font-medium">
                  {selected.name} ({selected.code})
                  <span className="text-secondary text-caption font-number ml-2 font-normal">
                    {seatDate?.slice(0, 10)}
                  </span>
                </DialogTitle>
                <DialogDescription className="sr-only">龙虎榜个股详情</DialogDescription>
              </DialogHeader>
              <div className="min-h-0 flex-1 overflow-y-auto p-6">
                {(selected.trade_dates?.length ?? 0) > 1 && (
                  <div className="mb-4 flex flex-wrap items-center gap-2">
                    <span className="text-secondary text-caption">席位日期</span>
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
                {selectedReasons.length > 0 && (
                  <div className="mb-4">
                    <h4 className="text-caption text-primary mb-2 font-medium">上榜原因</h4>
                    <ul className="text-body text-secondary space-y-1">
                      {selectedReasons.map((reason) => (
                        <li key={reason}>{reason}</li>
                      ))}
                    </ul>
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
              </div>
            </DialogContent>
          </Dialog>
        )}
      </div>
    </>
  )
}
