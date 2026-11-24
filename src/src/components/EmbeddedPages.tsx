import { lazy } from "react"
import EmbeddedPage from "@/components/EmbeddedPage"

// 整页视图（自选 / 模拟盘）作为网格格子时的包装。单独一个文件是因为
// paneRegistry 只导出注册表数据，混进组件会让 fast refresh 退化。
const RoleView = lazy(() => import("@/components/RoleView"))
const WatchlistView = lazy(() => import("@/components/WatchlistView"))

export function RolePane() {
  return (
    <EmbeddedPage name="RoleView">
      <RoleView />
    </EmbeddedPage>
  )
}

export function WatchlistPane({ leafId, layoutId }: { leafId: string; layoutId: string }) {
  return (
    <EmbeddedPage name="WatchlistView">
      <WatchlistView leafId={leafId} layoutId={layoutId} />
    </EmbeddedPage>
  )
}
