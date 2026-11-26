import { useCallback, useMemo } from "react"
import { useActiveStockStore, type ActiveStock } from "@/stores/activeStockStore"
import { useLayoutStore } from "@/stores/layoutStore"

export interface PaneStock {
  /** 这一格此刻在看的票（还没选过标的时为 `null`）。 */
  stock: ActiveStock | null
  /** 解绑过：固定在 `stock` 上，不再跟工作区标的走。 */
  pinned: boolean
  /** 换票：跟随中的格子改工作区标的，已解绑的格子改自己的固定值。 */
  select: (code: string, name: string) => void
  /** 解绑 / 恢复跟随。 */
  togglePinned: () => void
}

/**
 * 看盘格（个股详情 / 五档 / 下单 / 关键指标 / 所属板块）的标的来源。
 *
 * 模型只有两种状态，**默认跟随**：
 *
 * - 跟随：读 `activeStockStore`，榜单点票、任一格里搜索换票，所有跟随中的格子一起换
 * - 解绑：固定在这一只上，值写进布局（自定义布局写叶子，内置布局写 `presetSymbols`
 *   覆盖表），跟布局一起同步。看盘墙就是这么拼的：几格各解绑一只
 *
 * 没有第三种状态，也不再需要"拖拽建立联动"——格子要不要跟着走，它自己说了算。
 */
export function usePaneStock({
  layoutId,
  leafId,
  symbol,
  symbolName,
}: {
  layoutId: string
  leafId: string
  /** 布局里记着的固定值（自定义布局写在叶子上）。 */
  symbol?: string
  symbolName?: string
}): PaneStock {
  const override = useLayoutStore((s) => s.presetSymbols[layoutId]?.[leafId])
  const setPaneSymbol = useLayoutStore((s) => s.setPaneSymbol)
  const followActiveStock = useLayoutStore((s) => s.followActiveStock)
  const active = useActiveStockStore((s) => s.stock)
  const setActive = useActiveStockStore((s) => s.set)

  // 内置布局的树是只读预设，固定值只能落在覆盖表里
  const pinnedCode = symbol ?? override?.code
  const pinnedName = symbol ? symbolName : override?.name
  const pinned = !!pinnedCode

  const stock = useMemo(
    () => (pinnedCode ? { code: pinnedCode, name: pinnedName ?? pinnedCode } : active),
    [pinnedCode, pinnedName, active]
  )

  const select = useCallback(
    (code: string, name: string) => {
      // 已解绑的格子：换票等于改这个固定值；跟随中的格子只是把工作区标的换掉，
      // 它自己会跟着变，不因为"在这格里搜了一下"就悄悄解绑
      if (pinned) setPaneSymbol(layoutId, leafId, code, name)
      setActive({ code, name })
    },
    [pinned, layoutId, leafId, setPaneSymbol, setActive]
  )

  const togglePinned = useCallback(() => {
    if (!pinned) {
      if (stock) setPaneSymbol(layoutId, leafId, stock.code, stock.name)
      return
    }
    followActiveStock(layoutId, leafId)
  }, [pinned, stock, layoutId, leafId, setPaneSymbol, followActiveStock])

  return { stock, pinned, select, togglePinned }
}
