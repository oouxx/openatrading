import { useState, useCallback } from "react"
import { ChevronDownIcon, Search, X } from "lucide-react"
import { Button } from "@/components/ui/button"
import {
  Command,
  CommandEmpty,
  CommandInput,
  CommandItem,
  CommandList,
} from "@/components/ui/command"
import DragonTigerDetailView from "@/components/DragonTigerDetailView"
import { usePaneNav } from "@/components/paneNavContext"
import { Input } from "@/components/ui/input"
import { Popover, PopoverContent, PopoverTrigger } from "@/components/ui/popover"
import { DataEmpty, DataError, DataLoading } from "@/components/data-state"
import { useDragonTigerDetail, type DragonTigerDetailItem } from "@/hooks/query/useDragonTiger"

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
  // 详情是"这一格的下一层页面"，状态挂在格子 id 上（见 paneNavContext）
  const { dragonDetail, openDragon, closeDragon } = usePaneNav()
  const activeDate = date || resp?.date || ""

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

  // 打开详情即进入子页面；返回由子页面自己的返回按钮 / Esc 负责
  const openDetail = useCallback(
    (item: DragonTigerDetailItem) => {
      openDragon(item)
    },
    [openDragon]
  )

  const handleDeptChange = useCallback(
    (next: string | null) => {
      setDept(next)
      closeDragon()
    },
    [closeDragon]
  )

  const handleDateChange = useCallback(
    (next: string) => {
      setDate(next)
      closeDragon()
    },
    [closeDragon]
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

  // 子页面覆盖整格：详情里要点席位、看指标，列表留着只会挤在一起
  if (dragonDetail) {
    return <DragonTigerDetailView item={dragonDetail} onBack={closeDragon} />
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
                    const isSelected = false
                    return (
                      <tr
                        key={s.code}
                        className={`cursor-pointer ${isSelected ? "bg-brand/5" : ""}`}
                        onClick={() => openDetail(s)}
                      >
                        <td>
                          <div className="flex min-w-0 flex-col gap-0.5">
                            <button
                              type="button"
                              aria-expanded={isSelected}
                              className="text-primary hover:text-brand text-body cursor-pointer truncate text-left font-medium transition-colors"
                              onClick={(e) => {
                                e.stopPropagation()
                                openDetail(s)
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
    </>
  )
}
