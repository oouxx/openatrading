import { useMemo, type ReactNode } from "react"
import { PaneNavContext, type PaneNav } from "@/components/paneNavContext"
import { useLayoutStore } from "@/stores/layoutStore"

import { useRightPanelStore } from "@/stores/rightPanelStore"

/**
 * 面板内的"下一层页面"导航。
 *
 * 点股票只有一件事：**把工作区当前标的换掉**（`selectStock`）。所有跟随中的看盘格
 * ——个股详情 / 五档 / 下单 / 关键指标 / 所属板块——一起换票；解绑固定的格子不动。
 *
 * 以前这里还要判断"这一格有没有联动组、组里的接收方是谁"，然后派发过去。跟随变成
 * 默认之后那套逻辑没有信息量了：格子要不要跟着走，由它自己的跟随开关决定。
 *
 * 板块 / 龙虎榜这些详情子页面必须开在**点它的那一格**里：
 * - 网格格子 → 状态落在这一格的 leaf id 上，放大/还原、切回布局都还在，换榜时统一清掉；
 * - 右侧行情面板 → 沿用面板自身的覆盖层（老行为）。
 */
export function PaneNavProvider({ leafId, children }: { leafId: string; children: ReactNode }) {
  const selectStock = useLayoutStore((s) => s.selectStock)
  const openPanePlate = useLayoutStore((s) => s.openPanePlate)
  const openPaneDragon = useLayoutStore((s) => s.openPaneDragon)
  const closePaneDetail = useLayoutStore((s) => s.closePaneDetail)
  const paneDetail = useLayoutStore((s) => s.paneDetail[leafId])
  const top = paneDetail?.[paneDetail.length - 1]

  const value = useMemo<PaneNav>(
    () => ({
      openStock: (symbol, name) => selectStock(symbol, name),
      plateDetail: top?.type === "plate" ? top.plate : null,
      openPlate: (plate) => openPanePlate(leafId, plate),
      closePlate: () => closePaneDetail(leafId),
      dragonDetail: top?.type === "dragon" ? top.item : null,
      openDragon: (item) => openPaneDragon(leafId, item),
      closeDragon: () => closePaneDetail(leafId),
    }),
    [leafId, top, selectStock, openPanePlate, openPaneDragon, closePaneDetail]
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
