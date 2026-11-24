import KlineDrawer from "@/components/KlineDrawer"
import { useLayoutStore } from "@/stores/layoutStore"

/**
 * 面板内的 K 线子页面：盖住这一格的正文，关掉就回到原来的表。
 *
 * 挂在**这一格自己的 id** 上（`paneDetail` 栈），所以放大/还原、切布局都还在。
 * 以前只有行情格渲染这层，自选股点行只能开在右侧那条固定栏里；现在每一格都挂，
 * 联动派发到哪一格就在哪一格开。
 */
export default function PaneStockPage({ leafId }: { leafId: string }) {
  const stack = useLayoutStore((s) => s.paneDetail[leafId])
  const closePaneDetail = useLayoutStore((s) => s.closePaneDetail)
  const detail = stack?.[stack.length - 1]

  if (detail?.type !== "stock") return null

  return (
    <div className="bg-canvas absolute inset-0 z-20 overflow-y-auto">
      <KlineDrawer
        symbol={detail.symbol}
        name={detail.name}
        onClose={() => closePaneDetail(leafId)}
      />
    </div>
  )
}
