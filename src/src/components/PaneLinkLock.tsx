import { Lock, LockOpen } from "lucide-react"
import { Button } from "@/components/ui/button"
import { usePaneLinkStore, stockTargetLabel } from "@/stores/paneLinkStore"
import { cn } from "@/lib/utils"

/**
 * 联动锁：只有两态。
 *
 * - 锁定（默认）：点这一格里的股票，K 线在关联的看盘格打开（自选/榜单本身不动）
 * - 解锁：点股票就在本格打开子页面，关掉回到榜单
 *
 * 目标不让选（见 `paneLinkStore`），所以这里只给一个开关；悬停提示会说清当前锁到
 * 哪一格，以及解锁后会发生什么。
 */
export default function PaneLinkLock({
  layoutId,
  leafId,
  className,
}: {
  layoutId: string
  leafId: string
  className?: string
}) {
  const locked = usePaneLinkStore((s) => s.isLocked(layoutId, leafId))
  const setLocked = usePaneLinkStore((s) => s.setLocked)
  const target = stockTargetLabel(layoutId)

  const label = locked
    ? target
      ? `已联动到「${target}」格：点股票在那一格打开（点击改为本格打开）`
      : "已联动：点股票在关联格打开（点击改为本格打开）"
    : target
      ? `未联动：点股票在本格打开（点击联动到「${target}」格）`
      : "未联动：点股票在本格打开"
  const Icon = locked ? Lock : LockOpen

  return (
    <Button
      type="button"
      variant="ghost"
      size="icon-xs"
      aria-label={label}
      aria-pressed={locked}
      title={label}
      onClick={() => setLocked(layoutId, leafId, !locked)}
      className={cn(
        "shrink-0",
        locked ? "text-brand hover:text-brand" : "text-tertiary hover:text-secondary",
        className
      )}
    >
      <Icon />
    </Button>
  )
}
