import { useMemo, type ReactNode } from "react"
import { PaneNavContext, type PaneNav } from "@/components/paneNavContext"
import { useLayoutStore } from "@/stores/layoutStore"
import { resolveStockTarget, usePaneLinkStore } from "@/stores/paneLinkStore"
import { useRightPanelStore } from "@/stores/rightPanelStore"

/**
 * 面板内的"下一层页面"导航。
 *
 * 点股票要打开 K 线、点板块要打开板块详情：
 * - 股票 K 线默认派发到关联网格（自动跟随布局里第一格「个股详情」/ 对话格）；
 *   表头的锁解开时，就在本格开子页面。
 * - 板块 / 龙虎榜这些详情子页面必须开在**点它的那一格**里：
 * - 网格格子 → 状态落在这一格的 leaf id 上，放大/还原、切回布局都还在，换榜时统一清掉；
 * - 右侧行情面板 → 沿用面板自身的覆盖层（老行为）。
 *
 * 之前这些点击写的是同一个全局 `klineTarget`，只有右侧面板会渲染它，于是网格里
 * 点股票要么没反应，要么在别的面板里冒出来、关都关不掉。
 */

/** 网格格子用：子页面状态落在自己的 leaf id 上。 */
export function PaneNavProvider({
  leafId,
  layoutId,
  children,
}: {
  leafId: string
  layoutId: string
  children: ReactNode
}) {
  const openStockInPane = useLayoutStore((s) => s.openStockInPane)
  const locked = usePaneLinkStore((s) => s.isLocked(layoutId, leafId))
  const openPanePlate = useLayoutStore((s) => s.openPanePlate)
  const openPaneDragon = useLayoutStore((s) => s.openPaneDragon)
  const closePaneDetail = useLayoutStore((s) => s.closePaneDetail)
  const paneDetail = useLayoutStore((s) => s.paneDetail[leafId])
  const top = paneDetail?.[paneDetail.length - 1]

  const value = useMemo<PaneNav>(
    () => ({
      // 锁着派发到关联格，解开就在本格开子页面（PaneStockPage 负责渲染）
      openStock: (symbol, name) =>
        openStockInPane(
          layoutId,
          (locked ? resolveStockTarget(layoutId) : null) ?? leafId,
          symbol,
          name
        ),
      plateDetail: top?.type === "plate" ? top.plate : null,
      openPlate: (plate) => openPanePlate(leafId, plate),
      closePlate: () => closePaneDetail(leafId),
      dragonDetail: top?.type === "dragon" ? top.item : null,
      openDragon: (item) => openPaneDragon(leafId, item),
      closeDragon: () => closePaneDetail(leafId),
    }),
    [leafId, layoutId, locked, top, openStockInPane, openPanePlate, openPaneDragon, closePaneDetail]
  )
  return <PaneNavContext.Provider value={value}>{children}</PaneNavContext.Provider>
}

/** 右侧行情面板用：沿用面板自身的覆盖层。 */
export function RightPanelNavProvider({ children }: { children: ReactNode }) {
  const openKline = useRightPanelStore((s) => s.openKline)
  const plateTarget = useRightPanelStore((s) => s.plateTarget)
  const openPlate = useRightPanelStore((s) => s.openPlate)
  const closePlate = useRightPanelStore((s) => s.closePlate)
  const dragonTarget = useRightPanelStore((s) => s.dragonTarget)
  const openDragon = useRightPanelStore((s) => s.openDragon)
  const closeDragon = useRightPanelStore((s) => s.closeDragon)

  const value = useMemo<PaneNav>(
    () => ({
      openStock: openKline,
      plateDetail: plateTarget,
      openPlate,
      closePlate,
      dragonDetail: dragonTarget,
      openDragon,
      closeDragon,
    }),
    [openKline, plateTarget, openPlate, closePlate, dragonTarget, openDragon, closeDragon]
  )
  return <PaneNavContext.Provider value={value}>{children}</PaneNavContext.Provider>
}
