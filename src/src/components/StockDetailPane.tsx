import { useCallback } from "react"
import { CandlestickChart, Pin, PinOff } from "lucide-react"
import KlineDrawer from "@/components/KlineDrawer"
import { StockSearchBox } from "@/components/StockSearchBox"
import { Button } from "@/components/ui/button"
import { useLayoutStore } from "@/stores/layoutStore"
import { useActiveStockStore } from "@/stores/activeStockStore"

/**
 * 个股详情格：表头搜股票，正文是这只票的 K 线。
 *
 * **默认跟随工作区当前标的**——点榜单、在五档格里换票，这一格跟着换，不需要先
 * 建联动。想把它固定在某一只上就点表头的「解绑」（值写进叶子，跟布局一起同步，
 * 多格各解绑一只就是最简单的自选盯盘墙）；点「跟随」回到跟随状态。
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
  const followActiveStock = useLayoutStore((s) => s.followActiveStock)
  // 内置布局的树是只读预设，解绑后的固定值记在覆盖表里（同 `presetKinds`）
  const override = useLayoutStore((s) => s.presetSymbols[layoutId]?.[leafId])

  const setActiveStock = useActiveStockStore((s) => s.set)
  const active = useActiveStockStore((s) => s.stock)
  const pinnedCode = symbol ?? override?.code
  const pinnedName = symbol ? symbolName : override?.name
  const pinned = !!pinnedCode
  const shownCode = pinnedCode ?? active?.code
  const shownName = pinnedCode ? pinnedName : active?.name

  const select = useCallback(
    (code: string, name: string) => {
      // 已解绑的格子：换票等于改这个钉住值；跟随中的格子只是把工作区标的换掉，
      // 它自己会跟着变，不因为"在这格里搜了一下"就悄悄解绑
      if (pinned) setPaneSymbol(layoutId, leafId, code, name)
      setActiveStock({ code, name })
    },
    [pinned, layoutId, leafId, setPaneSymbol, setActiveStock]
  )

  return (
    <div className="bg-surface flex h-full min-w-0 flex-col overflow-hidden">
      <header className="border-hairline flex h-9 shrink-0 items-center gap-1 border-b px-2">
        <StockSearchBox
          className="min-w-0 flex-1"
          placeholder={shownCode ? `${shownName ?? shownCode} · 换一只` : "搜索股票…"}
          ariaLabel="搜索要查看的股票"
          onPick={select}
        />
        {shownCode && (
          <Button
            variant="ghost"
            size="xs"
            className={`shrink-0 ${pinned ? "text-brand" : "text-tertiary"}`}
            title={
              pinned
                ? "恢复跟随：这一格重新跟工作区当前标的走"
                : `解绑：固定看 ${shownName ?? shownCode}，不再跟随`
            }
            aria-pressed={pinned}
            onClick={() => {
              if (pinned) followActiveStock(layoutId, leafId)
              else setPaneSymbol(layoutId, leafId, shownCode, shownName ?? shownCode)
            }}
          >
            {pinned ? <PinOff className="size-3" /> : <Pin className="size-3" />}
            {pinned ? "跟随" : "解绑"}
          </Button>
        )}
      </header>

      {shownCode ? (
        <div className="min-h-0 flex-1 overflow-hidden">
          <KlineDrawer
            symbol={shownCode}
            name={shownName ?? shownCode}
            onClose={() => {}}
            showClose={false}
          />
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
              onClick={() => {
                // 空态也能一键选一只默认标的，省掉一次输入
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
