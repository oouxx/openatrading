import { Pin, PinOff } from "lucide-react"
import { Button } from "@/components/ui/button"
import { usePaneStock } from "@/hooks/usePaneStock"
import { cn } from "@/lib/utils"
import type { PaneLeaf, WorkspaceLayout } from "@/stores/layoutStore"

/**
 * 看盘格右上角的「跟随 / 解绑」开关。
 *
 * 以前这个位置是拖拽联动的把手：拖到另一格上成组，点那边的票就派发过来。跟随变成
 * 默认之后，那套手势只剩"这一格要不要跟着工作区标的走"一个语义——一个开关就够了，
 * 而且每格自己说了算，不需要先画一张关系图。
 *
 * 只挂在看盘格上（`FOLLOW_PANE_KINDS`）：榜单/自选/模拟盘/对话是"点票的地方"，
 * 不是"看票的地方"，没有标的可解绑。
 */
export default function PaneFollowToggle({
  leaf,
  layout,
  alwaysVisible = false,
}: {
  leaf: PaneLeaf
  layout: WorkspaceLayout
  /** 放大态没有稳定的悬停区，开关必须常驻 */
  alwaysVisible?: boolean
}) {
  const { stock, pinned, togglePinned } = usePaneStock({
    layoutId: layout.id,
    leafId: leaf.id,
    symbol: leaf.symbol,
    symbolName: leaf.symbolName,
  })

  const label = pinned
    ? `已解绑：固定看 ${stock?.name ?? "这一只"}（点一下恢复跟随）`
    : `跟随工作区当前标的${stock ? `（${stock.name}）` : ""}，点一下解绑固定`

  return (
    <Button
      variant="ghost"
      size="icon-xs"
      aria-label={label}
      title={label}
      aria-pressed={pinned}
      disabled={!pinned && !stock}
      onClick={togglePinned}
      className={cn(
        "bg-surface/90 border-hairline hover:bg-hover absolute top-1.5 right-19 z-30 rounded-md border transition-opacity",
        pinned ? "text-brand hover:text-brand" : "text-tertiary hover:text-primary",
        alwaysVisible || pinned ? "opacity-100" : "opacity-0 group-hover/pane:opacity-100"
      )}
    >
      {pinned ? <Pin className="size-3" /> : <PinOff className="size-3" />}
    </Button>
  )
}
