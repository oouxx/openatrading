import { useCallback } from "react"
import { CandlestickChart } from "lucide-react"
import KlineDrawer from "@/components/KlineDrawer"
import { StockSearchBox } from "@/components/StockSearchBox"
import { Button } from "@/components/ui/button"
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

  const select = useCallback(
    (code: string, name: string) => {
      setPaneSymbol(layoutId, leafId, code, name)
    },
    [layoutId, leafId, setPaneSymbol]
  )

  return (
    <div className="bg-surface flex h-full min-w-0 flex-col overflow-hidden">
      <header className="border-hairline flex h-9 shrink-0 items-center border-b px-2">
        <StockSearchBox
          className="w-full"
          placeholder={symbol ? `${symbolName ?? symbol} · 换一只` : "搜索股票…"}
          ariaLabel="搜索要钉住的股票"
          onPick={select}
        />
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
