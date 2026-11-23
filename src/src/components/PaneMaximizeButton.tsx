import { Maximize2, Minimize2 } from "lucide-react"
import { Button } from "@/components/ui/button"
import { useLayoutStore } from "@/stores/layoutStore"
import { cn } from "@/lib/utils"

/**
 * 每格右上角的"放大 / 还原"。
 *
 * 14 种面板的表头形态各不相同（对话格现在干脆没有表头），所以这个按钮不能焊进
 * 某一家的表头里——统一浮在格子右上角，鼠标悬停才出现，键盘聚焦也可见；
 * 放大态没有"悬停区"可言，所以常驻。
 */
export default function PaneMaximizeButton({
  leafId,
  alwaysVisible = false,
}: {
  leafId: string
  /** 放大态下按钮必须常驻，否则鼠标一移开就没法还原 */
  alwaysVisible?: boolean
}) {
  const maximized = useLayoutStore((s) => s.maximizedPaneId === leafId)
  const setMaximizedPane = useLayoutStore((s) => s.setMaximizedPane)
  const Icon = maximized ? Minimize2 : Maximize2
  const label = maximized ? "还原（Esc）" : "放大这一格（⌘⇧F）"

  return (
    <Button
      variant="ghost"
      size="icon-xs"
      aria-label={label}
      title={label}
      aria-pressed={maximized}
      onClick={() => setMaximizedPane(maximized ? null : leafId)}
      className={cn(
        "bg-surface/90 border-hairline hover:bg-hover text-tertiary hover:text-primary absolute top-1.5 right-1.5 z-30 rounded-md border transition-opacity",
        alwaysVisible
          ? "opacity-100"
          : "opacity-0 group-hover/pane:opacity-100 focus-visible:opacity-100"
      )}
    >
      <Icon />
    </Button>
  )
}
