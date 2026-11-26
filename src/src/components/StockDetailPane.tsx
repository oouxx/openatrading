import { CandlestickChart } from "lucide-react"
import KlineDrawer from "@/components/KlineDrawer"
import { StockSearchBox } from "@/components/StockSearchBox"
import { Button } from "@/components/ui/button"
import { usePaneStock } from "@/hooks/usePaneStock"

/**
 * 个股详情格：表头搜股票，正文是这只票的 K 线。
 *
 * **默认跟随工作区当前标的**——点榜单、在五档格里换票，这一格跟着换。想把它固定在
 * 某一只上就点右上角的图钉（解绑），几格各解绑一只就是最简单的自选盯盘墙。
 *
 * 开关统一挂在格子角落（`PaneFollowToggle`），表头只留搜索框——同一个动作不该在
 * 一格里有第二个入口。
 */
export default function StockDetailPane({
  layoutId,
  leafId,
  symbol,
  symbolName,
}: {
  layoutId: string
  leafId: string
  symbol?: string
  symbolName?: string
}) {
  const { stock, select } = usePaneStock({ layoutId, leafId, symbol, symbolName })

  return (
    <div className="bg-surface flex h-full min-w-0 flex-col overflow-hidden">
      <header className="border-hairline flex h-9 shrink-0 items-center border-b px-2">
        <StockSearchBox
          className="w-full"
          placeholder={stock ? `${stock.name} · 换一只` : "搜索股票…"}
          ariaLabel="搜索要查看的股票"
          onPick={select}
        />
      </header>

      {stock ? (
        <div className="min-h-0 flex-1 overflow-hidden">
          <KlineDrawer symbol={stock.code} name={stock.name} onClose={() => {}} showClose={false} />
        </div>
      ) : (
        <div className="flex min-h-0 flex-1 items-center justify-center px-6">
          <div className="text-center">
            <CandlestickChart className="text-tertiary mx-auto mb-2 size-5" />
            <p className="text-secondary text-caption">在上面搜索一只股票</p>
            <p className="text-tertiary text-micro mt-1">不搜索就跟着工作区当前标的走</p>
            <Button
              variant="outline"
              size="xs"
              className="mt-3"
              onClick={() => select("600519", "贵州茅台")}
            >
              试试 600519 贵州茅台
            </Button>
          </div>
        </div>
      )}
    </div>
  )
}
