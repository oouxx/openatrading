import { useMemo } from "react"
import { Link2, Link2Off } from "lucide-react"
import { Button } from "@/components/ui/button"
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from "@/components/ui/select"
import { useLayoutStore } from "@/stores/layoutStore"
import { stockTargetCandidates, usePaneLinkStore } from "@/stores/paneLinkStore"

/**
 * 联动目标选择器：点表格里的股票时，K 线在**哪一格**打开。
 *
 * 一个布局只有一份设置（见 `paneLinkStore`），所以自选股、强势股、个股排行等
 * 表头的这个控件改的是同一个值——在任意一处调好，其它格跟着走。
 *
 * 没有可选目标（布局里没有「个股详情」格）时不渲染：那种情况下点击会退回本格
 * 子页面，控件摆出来只会让人点了没反应。
 */
export default function StockLinkSelect({
  layoutId,
  className,
}: {
  layoutId: string
  className?: string
}) {
  const linked = usePaneLinkStore((s) => s.links[layoutId] ?? null)
  const link = usePaneLinkStore((s) => s.link)
  const unlink = usePaneLinkStore((s) => s.unlink)
  // 目标格被拆掉/删掉后要跟着失效：候选跟布局对象一起变
  const layout = useLayoutStore((s) => s.layouts.find((item) => item.id === layoutId))
  const candidates = useMemo(() => stockTargetCandidates(layout), [layout])
  const target = linked && candidates.some((item) => item.id === linked) ? linked : null

  if (candidates.length === 0) return null

  return (
    <div className={`flex items-center gap-1 ${className ?? ""}`}>
      <Select
        value={target ?? "auto"}
        onValueChange={(next) => (next === "auto" ? unlink(layoutId) : link(layoutId, next))}
      >
        <SelectTrigger
          size="sm"
          aria-label="点击股票时在哪一格打开 K 线"
          title="点击股票时在哪一格打开 K 线"
          className="text-caption text-secondary h-8 w-[150px]"
        >
          <Link2 className={target ? "text-brand" : "text-tertiary"} />
          <SelectValue />
        </SelectTrigger>
        <SelectContent>
          {candidates.map((item) => (
            <SelectItem key={item.id} value={item.id}>
              {item.label}
            </SelectItem>
          ))}
          {/* 自动 = 跟随布局里的第一格「个股详情」，结构变了也不会指空 */}
          <SelectItem value="auto">自动（跟随布局）</SelectItem>
        </SelectContent>
      </Select>
      {target && (
        <Button
          variant="ghost"
          size="icon-sm"
          className="text-tertiary hover:text-primary"
          title="解除关联"
          aria-label="解除关联"
          onClick={() => unlink(layoutId)}
        >
          <Link2Off className="size-3.5" />
        </Button>
      )}
    </div>
  )
}
