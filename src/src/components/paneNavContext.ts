import { createContext, useContext, useState } from "react"
import type { DragonDetailTarget, PlateDetailTarget } from "@/lib/layoutTypes"
import { useRightPanelStore } from "@/stores/rightPanelStore"

/**
 * 面板内的"下一层页面"导航（见 `PaneNav.tsx` 里的两个 Provider）。
 *
 * 单独放一个 .ts 文件：hook 与 Provider 混在一个文件里会让 fast refresh 退化。
 */
export interface PaneNav {
  openStock: (symbol: string, name: string) => void
  /** 当前这一格是否正停留在某个板块详情里（null 表示在列表页）。 */
  plateDetail: PlateDetailTarget | null
  openPlate: (plate: PlateDetailTarget) => void
  closePlate: () => void
  /** 龙虎榜个股详情。 */
  dragonDetail: DragonDetailTarget | null
  openDragon: (item: DragonDetailTarget) => void
  closeDragon: () => void
}

export const PaneNavContext = createContext<PaneNav | null>(null)

/** 没有 provider 时退化成组件内局部状态，保证老调用点不炸。 */
export function usePaneNav(): PaneNav {
  const ctx = useContext(PaneNavContext)
  const openKline = useRightPanelStore((s) => s.openKline)
  const [localPlate, setLocalPlate] = useState<PlateDetailTarget | null>(null)
  const [localDragon, setLocalDragon] = useState<DragonDetailTarget | null>(null)
  if (ctx) return ctx
  return {
    openStock: openKline,
    plateDetail: localPlate,
    openPlate: setLocalPlate,
    closePlate: () => setLocalPlate(null),
    dragonDetail: localDragon,
    openDragon: setLocalDragon,
    closeDragon: () => setLocalDragon(null),
  }
}
