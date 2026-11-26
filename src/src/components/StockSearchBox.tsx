import { useCallback, useEffect, useRef, useState } from "react"
import { Search } from "lucide-react"
import { Command, CommandGroup, CommandItem, CommandList } from "@/components/ui/command"
import { Input } from "@/components/ui/input"
import { searchStocks } from "@/lib/stock-utils"
import type { StockSearchItem } from "@/stores/data"

/**
 * 股票搜索框（带下拉候选）。
 *
 * 个股详情格和几个跟随型看盘格都要"搜一只票然后打开它"，各处的占位符和尺寸不同，
 * 逻辑（防抖、请求序号、键盘选择、点外面收起）留在这里一份。
 */
export function StockSearchBox({
  placeholder = "搜索股票…",
  ariaLabel = "搜索股票代码或名称",
  className,
  excludeIndexes = false,
  onPick,
}: {
  placeholder?: string
  ariaLabel?: string
  /** 宽度/尺寸差异（如 `w-52`、`w-full`）给外层容器，输入框自己撑满。 */
  className?: string
  /** 只列可交易标的：五档 / 下单格的换股票框用（指数不能交易，列出来只会白点一下）。 */
  excludeIndexes?: boolean
  onPick: (code: string, name: string) => void
}) {
  const [query, setQuery] = useState("")
  const [results, setResults] = useState<StockSearchItem[]>([])
  const [loading, setLoading] = useState(false)
  const [open, setOpen] = useState(false)
  const [highlight, setHighlight] = useState(0)
  const containerRef = useRef<HTMLDivElement | null>(null)
  const debounceRef = useRef<ReturnType<typeof setTimeout> | null>(null)
  // 请求序号：慢的那次回包不能盖掉快的那次
  const seqRef = useRef(0)

  const pick = useCallback(
    (code: string, name: string) => {
      onPick(code, name)
      setQuery("")
      setResults([])
      setOpen(false)
    },
    [onPick]
  )

  const onChange = (value: string) => {
    setQuery(value)
    const seq = ++seqRef.current
    if (debounceRef.current) clearTimeout(debounceRef.current)
    if (!value.trim()) {
      setResults([])
      setOpen(false)
      setLoading(false)
      return
    }
    setLoading(true)
    debounceRef.current = setTimeout(() => {
      void searchStocks(value)
        .then((items) => {
          if (seq !== seqRef.current) return
          const tradeable = excludeIndexes ? items.filter((item) => item.kind !== "index") : items
          setResults(tradeable.slice(0, 8))
          setHighlight(0)
          setOpen(true)
        })
        .catch(() => {
          if (seq === seqRef.current) setResults([])
        })
        .finally(() => {
          if (seq === seqRef.current) setLoading(false)
        })
    }, 250)
  }

  // 点外面收起下拉
  useEffect(() => {
    const handler = (event: MouseEvent) => {
      if (containerRef.current && !containerRef.current.contains(event.target as Node)) {
        setOpen(false)
      }
    }
    document.addEventListener("mousedown", handler)
    return () => document.removeEventListener("mousedown", handler)
  }, [])

  return (
    <div ref={containerRef} className={`relative min-w-0 ${className ?? ""}`}>
      <Search className="text-tertiary pointer-events-none absolute top-1/2 left-2 size-3.5 -translate-y-1/2" />
      <Input
        value={query}
        onChange={(e) => onChange(e.target.value)}
        placeholder={placeholder}
        aria-label={ariaLabel}
        className="bg-canvas border-hairline focus-visible:border-brand/60 text-caption h-7 w-full rounded-md pl-7"
        onFocus={() => {
          if (query.trim() && results.length > 0) setOpen(true)
        }}
        onKeyDown={(e) => {
          if (!open) return
          if (e.key === "ArrowDown" || e.key === "ArrowUp") {
            e.preventDefault()
            const delta = e.key === "ArrowDown" ? 1 : -1
            setHighlight((prev) => (prev + delta + results.length) % Math.max(results.length, 1))
            return
          }
          if (e.key === "Enter") {
            const picked = results[highlight] ?? results[0]
            // 带后缀的符号才是行情/自选能用的：`000001` 既是上证指数也是平安银行
            if (picked) pick(picked.symbol ?? picked.code, picked.name)
            return
          }
          if (e.key === "Escape") setOpen(false)
        }}
      />
      {open && (
        <div className="absolute top-full right-0 left-0 z-30 mt-1">
          <Command
            shouldFilter={false}
            className="bg-surface border-hairline shadow-modal rounded-lg border"
          >
            <CommandList className="max-h-56">
              {loading && results.length === 0 ? (
                <div className="text-secondary text-caption px-3 py-2">搜索中…</div>
              ) : results.length === 0 ? (
                <div className="text-secondary text-caption px-3 py-2">未找到结果</div>
              ) : (
                <CommandGroup>
                  {results.map((stock, index) => (
                    <CommandItem
                      key={stock.code}
                      value={stock.code}
                      onSelect={() => pick(stock.symbol ?? stock.code, stock.name)}
                      className="text-caption"
                      data-highlighted={index === highlight ? "true" : undefined}
                    >
                      <span className="text-secondary font-number">{stock.code}</span>
                      <span className="text-primary">{stock.name}</span>
                    </CommandItem>
                  ))}
                </CommandGroup>
              )}
            </CommandList>
          </Command>
        </div>
      )}
    </div>
  )
}
