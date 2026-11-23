import { Columns2, Rows2, Trash2 } from "lucide-react"
import { Button } from "@/components/ui/button"
import {
  Select,
  SelectContent,
  SelectGroup,
  SelectItem,
  SelectLabel,
  SelectTrigger,
  SelectValue,
} from "@/components/ui/select"
import { paneIcon } from "@/lib/paneRegistry"
import {
  GENERAL_PANE_KINDS,
  MARKET_PANE_KINDS,
  PANE_HORIZONTAL_ONLY,
  PANE_KIND_LABELS,
  countLeaves,
  useLayoutStore,
  type PaneKind,
  type PaneLeaf,
  type SplitOrientation,
  type WorkspaceLayout,
} from "@/stores/layoutStore"

/**
 * 结构编辑态下浮在每个面板右下角的工具条。
 *
 * 放右下角而不是标题栏：几乎每个面板都有自己的页头，工具条压在页头上会把标签挡掉。
 * 只改结构的三个动作（拆分左右、拆分上下、删除）都在这，改内容（哪只行情）留在面板
 * 自己的标题里——编辑结构和切换内容是两件事，不该挤在同一个控件里。
 */
/** 下拉项：图标 + 名字（十三个选项只靠文字扫一眼很难区分行情那几个）。 */
function renderKindOption(kind: PaneKind) {
  const Icon = paneIcon(kind)
  return (
    <span className="flex items-center gap-2">
      <Icon className="text-tertiary size-3.5" />
      {PANE_KIND_LABELS[kind]}
    </span>
  )
}

export default function PaneEditOverlay({
  leaf,
  layout,
  parentOrientation,
}: {
  leaf: PaneLeaf
  layout: WorkspaceLayout
  parentOrientation?: SplitOrientation
}) {
  const setPaneKind = useLayoutStore((s) => s.setPaneKind)
  const splitPane = useLayoutStore((s) => s.splitPane)
  const removePane = useLayoutStore((s) => s.removePane)

  // 上下分割里不能放"行情面板"：它折叠时是一条 48px 宽的竖条，横过来会散架
  const generalKinds =
    parentOrientation === "vertical"
      ? GENERAL_PANE_KINDS.filter((kind) => !PANE_HORIZONTAL_ONLY.has(kind))
      : GENERAL_PANE_KINDS
  const isOnlyPane = countLeaves(layout.root) <= 1

  return (
    <div className="bg-surface/95 border-hairline shadow-modal absolute right-2 bottom-2 z-30 flex items-center gap-0.5 rounded-md border p-0.5">
      <Select
        value={leaf.kind}
        onValueChange={(next) => setPaneKind(layout.id, leaf.id, next as PaneKind)}
      >
        <SelectTrigger
          size="sm"
          aria-label="这一格放什么"
          className="text-micro h-6 w-24 border-0 bg-transparent px-1.5 shadow-none"
        >
          <SelectValue />
        </SelectTrigger>
        <SelectContent>
          <SelectGroup>
            <SelectLabel>通用</SelectLabel>
            {generalKinds.map((kind) => (
              <SelectItem key={kind} value={kind}>
                {renderKindOption(kind)}
              </SelectItem>
            ))}
          </SelectGroup>
          <SelectGroup>
            <SelectLabel>行情</SelectLabel>
            {MARKET_PANE_KINDS.map((kind) => (
              <SelectItem key={kind} value={kind}>
                {renderKindOption(kind)}
              </SelectItem>
            ))}
          </SelectGroup>
        </SelectContent>
      </Select>
      <Button
        variant="ghost"
        size="icon-xs"
        aria-label="向右拆分"
        title="向右拆分"
        onClick={() => splitPane(layout.id, leaf.id, "horizontal")}
        className="text-tertiary hover:text-primary"
      >
        <Columns2 />
      </Button>
      <Button
        variant="ghost"
        size="icon-xs"
        aria-label="向下拆分"
        title="向下拆分"
        onClick={() => splitPane(layout.id, leaf.id, "vertical")}
        className="text-tertiary hover:text-primary"
      >
        <Rows2 />
      </Button>
      <Button
        variant="ghost"
        size="icon-xs"
        aria-label="删除这一格"
        title="删除这一格"
        disabled={isOnlyPane}
        onClick={() => removePane(layout.id, leaf.id)}
        className="text-tertiary hover:text-danger"
      >
        <Trash2 />
      </Button>
    </div>
  )
}
