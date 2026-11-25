import { useState } from "react"
import { Check, LayoutGrid } from "lucide-react"
import { Button } from "@/components/ui/button"
import { Popover, PopoverContent, PopoverTrigger } from "@/components/ui/popover"
import { PANE_ICONS, SWITCHABLE_PANE_KINDS } from "@/lib/paneRegistry"
import {
  PANE_HORIZONTAL_ONLY,
  PANE_KIND_LABELS,
  eachLeaf,
  isMarketPaneKind,
  resolvePaneKind,
  useLayoutStore,
  type PaneKind,
} from "@/stores/layoutStore"
import { cn } from "@/lib/utils"

/**
 * 每格右上角的"这一格放什么"：换内容不改结构，所以对话 / 个股详情 / 五档 / 下单 /
 * 自选 / 模拟盘和九个行情榜可以随便换，内置预设也能直接换（记在覆盖表里，预设本身不动）。
 * 例外是对话格：一个布局只能有一条，已有别的对话格时这一项禁用。
 *
 * 和行情格表头那个「换榜」下拉的分工：表头是高频的"换个榜看"；这里是低频的"把这一格
 * 变成别的面板"，所以和放大按钮一样统一浮在角落——十几种面板表头形态各不相同，焊进
 * 某一家的表头就会有的格子进得去出不来。
 *
 * 用 Popover 而不是 Select：触发器要的是一个 24px 的纯图标按钮（和放大按钮并排），
 * Select 的触发器和下拉箭头在这么小的尺寸里没法干净地隐藏。
 */
export default function PaneKindSwitcher({
  layoutId,
  leafId,
  kind,
  alwaysVisible = false,
}: {
  layoutId: string
  leafId: string
  kind: PaneKind
  /** 放大态下常驻（没有稳定的悬停区） */
  alwaysVisible?: boolean
}) {
  const setPaneKind = useLayoutStore((s) => s.setPaneKind)
  const [open, setOpen] = useState(false)
  const Icon = PANE_ICONS[kind] ?? LayoutGrid
  // 一个布局只能有一条对话格：所有对话格读同一个 activeThreadId，多开只会互相镜像，
  // 还各自建一条 WebSocket。已有别的对话格时，把这一项禁掉。
  const layout = useLayoutStore((s) => s.layouts.find((item) => item.id === layoutId))
  const presetKinds = useLayoutStore((s) => s.presetKinds)
  const chatTaken = Boolean(
    layout &&
    (() => {
      let taken = false
      eachLeaf(layout.root, (leaf) => {
        if (leaf.id === leafId) return
        if (resolvePaneKind(layout, leaf, presetKinds) === "chat") taken = true
      })
      return taken
    })()
  )

  const groups: { label: string; kinds: PaneKind[] }[] = [
    {
      label: "通用",
      kinds: SWITCHABLE_PANE_KINDS.filter((item) => !isMarketPaneKind(item)),
    },
    {
      label: "行情",
      kinds: SWITCHABLE_PANE_KINDS.filter((item) => isMarketPaneKind(item)),
    },
  ]

  return (
    <Popover open={open} onOpenChange={setOpen}>
      <PopoverTrigger asChild>
        <Button
          variant="ghost"
          size="icon-xs"
          aria-label="换这一格的内容"
          title="换这一格的内容"
          className={cn(
            "bg-surface/90 border-hairline hover:bg-hover text-tertiary hover:text-primary absolute top-1.5 right-11 z-30 rounded-md border transition-opacity",
            alwaysVisible
              ? "opacity-100"
              : "opacity-0 group-hover/pane:opacity-100 focus-visible:opacity-100"
          )}
        >
          <Icon />
        </Button>
      </PopoverTrigger>
      <PopoverContent align="end" className="max-h-[70vh] w-52 overflow-y-auto p-1">
        {groups.map((group) => (
          <div key={group.label}>
            <p className="text-tertiary text-micro px-2 py-1">{group.label}</p>
            {group.kinds
              .filter((item) => !PANE_HORIZONTAL_ONLY.has(item))
              .map((item) => {
                const ItemIcon = PANE_ICONS[item] ?? LayoutGrid
                const active = item === kind
                const disabled = item === "chat" && !active && chatTaken
                return (
                  <Button
                    key={item}
                    type="button"
                    variant="ghost"
                    size="sm"
                    disabled={disabled}
                    title={disabled ? "布局里已经有一个对话格了" : undefined}
                    onClick={() => {
                      setPaneKind(layoutId, leafId, item)
                      setOpen(false)
                    }}
                    className={cn(
                      "text-caption h-7 w-full justify-start gap-2 px-2 font-normal",
                      active ? "text-primary bg-hover" : "text-secondary hover:text-primary",
                      disabled && "text-tertiary hover:text-tertiary"
                    )}
                  >
                    <ItemIcon className="text-tertiary size-3.5" />
                    {PANE_KIND_LABELS[item] ?? item}
                    {disabled && <span className="text-micro text-tertiary ml-auto">已有一格</span>}
                    {active && <Check className="text-brand ml-auto size-3" />}
                  </Button>
                )
              })}
          </div>
        ))}
      </PopoverContent>
    </Popover>
  )
}
