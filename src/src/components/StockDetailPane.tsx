import { useCallback, useEffect, useRef, useState } from "react"
import { CandlestickChart, Search } from "lucide-react"
import KlineDrawer from "@/components/KlineDrawer"
import { Button } from "@/components/ui/button"
import { Command, CommandGroup, CommandItem, CommandList } from "@/components/ui/command"
import { Input } from "@/components/ui/input"
import { searchStocks } from "@/lib/stock-utils"
import { useLayoutStore } from "@/stores/layoutStore"

/**
 * 个股详情格：表头搜股票，正文是这只票的 K 线与盘口。
 *
 * 和"行情视图"格的区别是它盯的是**一只票**而不是一张榜——所以钉住的代码存在叶子上
 * （会跟着布局一起同步到后端），多格各钉一只就是最简单的自选盯盘墙。
 */
export default function StockDetailPane({
  leafId,
  layoutId,
  symbol,
  symbolName,
}: {
  leafId: string
  layoutId: string
  symbol?: string
  symbolName?: string
}) {
  const setPaneSymbol = useLayoutStore((s) => s.setPaneSymbol)
  const [query, setQuery] = useState("")
  const [results, setResults] = useState<{ code: string; name: string }[]>([])
  const [loading, setLoading] = useState(false)
  const [open, setOpen] = useState(false)
  const [highlight, setHighlight] = useState(0)
  const containerRef = useRef<HTMLDivElement | null>(null)
  const debounceRef = useRef<ReturnType<typeof setTimeout> | null>(null)
  // 请求序号：慢的那次回包不能盖掉快的那次
  const seqRef = useRef(0)

  const select = useCallback(
    (code: string, name: string) => {
      setPaneSymbol(layoutId, leafId, code, name)
      setQuery("")
      setOpen(false)
    },
    [layoutId, leafId, setPaneSymbol]
  )

  const onQueryChange = (value: string) => {
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
          setResults(items.slice(0, 8))
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
      if (containerRef.current && !containerRef.current.contains(event.target as Node))
        setOpen(false)
    }
    document.addEventListener("mousedown", handler)
    return () => document.removeEventListener("mousedown", handler)
  }, [])

  return (
    <div className="bg-surface flex h-full min-w-0 flex-col overflow-hidden">
      <header className="border-hairline flex h-9 shrink-0 items-center gap-1.5 border-b px-2">
        <div ref={containerRef} className="relative min-w-0 flex-1">
          <Search className="text-tertiary pointer-events-none absolute top-1/2 left-2 size-3.5 -translate-y-1/2" />
          <Input
            value={query}
            onChange={(e) => onQueryChange(e.target.value)}
            placeholder={symbol ? `${symbolName ?? symbol} · 换一只` : "搜索股票…"}
            aria-label="搜索要钉住的股票"
            className="bg-canvas border-hairline focus-visible:border-brand/60 text-caption h-7 rounded-md pl-7"
            onFocus={() => query.trim() && results.length > 0 && setOpen(true)}
            onKeyDown={(e) => {
              if (!open) return
              if (e.key === "ArrowDown" || e.key === "ArrowUp") {
                e.preventDefault()
                const delta = e.key === "ArrowDown" ? 1 : -1
                setHighlight(
                  (prev) => (prev + delta + results.length) % Math.max(results.length, 1)
                )
                return
              }
              if (e.key === "Enter") {
                const picked = results[highlight] ?? results[0]
                if (picked) select(picked.code, picked.name)
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
                          onSelect={() => select(stock.code, stock.name)}
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
      </header>

      {symbol ? (
        <div className="min-h-0 flex-1 overflow-hidden">
          <KlineDrawer
            symbol={symbol}
            name={symbolName ?? symbol}
            onClose={() => {}}
            showClose={false}
          />
        </div>
      ) : (
        <div className="flex min-h-0 flex-1 items-center justify-center px-6">
          <div className="text-center">
            <CandlestickChart className="text-tertiary mx-auto mb-2 size-5" />
            <p className="text-secondary text-caption">在上面搜索一只股票</p>
            <p className="text-tertiary text-micro mt-1">这一格会一直钉住它</p>
            <Button
              variant="outline"
              size="xs"
              className="mt-3"
              onClick={() => {
                // 空态也能一键钉住一个默认标的，省掉一次输入
                select("600519", "贵州茅台")
              }}
            >
              试试 600519 贵州茅台
            </Button>
          </div>
        </div>
      )}
    </div>
  )
}
