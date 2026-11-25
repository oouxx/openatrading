import { useMemo, type ReactNode } from "react"
import { PaneNavContext, type PaneNav } from "@/components/paneNavContext"
import { groupOf, linkedReceivers, useLayoutStore } from "@/stores/layoutStore"
import { hintPaneLinkOnce } from "@/lib/pane-link-hint"

import { useRightPanelStore } from "@/stores/rightPanelStore"

/**
 * 面板内的"下一层页面"导航。
 *
 * 点股票要打开 K 线、点板块要打开板块详情：
 * - 股票 K 线：这一格被拖到别的格上关联过就派发过去；没联动就只选中（不盖住榜单），
 *   并提示一次怎么建立联动。
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
  const selectStock = useLayoutStore((s) => s.selectStock)
  // 联动组是拖出来的（`usePaneDragLink`）：组里有接收方就派发过去，没有就在本格开
  // 子页面。订阅一下 groups，拖拽改组后这一格的派发目标要立刻跟着变。
  useLayoutStore((s) => s.groups[layoutId])
  const receivers = linkedReceivers(layoutId, leafId)
  const groupId = groupOf(layoutId, leafId)
  const openPanePlate = useLayoutStore((s) => s.openPanePlate)
  const openPaneDragon = useLayoutStore((s) => s.openPaneDragon)
  const closePaneDetail = useLayoutStore((s) => s.closePaneDetail)
  const paneDetail = useLayoutStore((s) => s.paneDetail[leafId])
  const top = paneDetail?.[paneDetail.length - 1]

  const value = useMemo<PaneNav>(
    () => ({
      openStock: (symbol, name) => {
        if (receivers.length === 0) {
          // 没成组就不开任何页面：只把这只票设为工作区当前标的（五档/下单格会跟着），
          // 免得整格榜单被一层临时 K 线盖住。第一次点顺带教一次怎么建立联动。
          selectStock(symbol, name)
          hintPaneLinkOnce(groupId ? "no-receiver" : "unlinked")
          return
        }
        // 一对多：组里的接收方（个股详情钉住那只票 / 对话格开 K 线标签）都更新
        for (const receiver of receivers) openStockInPane(layoutId, receiver, symbol, name)
      },
      plateDetail: top?.type === "plate" ? top.plate : null,
      openPlate: (plate) => openPanePlate(leafId, plate),
      closePlate: () => closePaneDetail(leafId),
      dragonDetail: top?.type === "dragon" ? top.item : null,
      openDragon: (item) => openPaneDragon(leafId, item),
      closeDragon: () => closePaneDetail(leafId),
    }),
    [
      leafId,
      layoutId,
      receivers,
      groupId,
      top,
      openStockInPane,
      selectStock,
      openPanePlate,
      openPaneDragon,
      closePaneDetail,
    ]
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
