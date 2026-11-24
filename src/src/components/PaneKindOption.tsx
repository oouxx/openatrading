import { PANE_KIND_LABELS, type PaneKind } from "@/stores/layoutStore"
import { PANE_ICONS } from "@/lib/paneRegistry"
import { Terminal } from "lucide-react"

/** 下拉里的一行：图标 + 名字（十几种面板类型只靠文字扫一眼很难区分）。 */
export default function PaneKindOption({ kind }: { kind: PaneKind }) {
  const Icon = PANE_ICONS[kind] ?? Terminal
  return (
    <span className="flex items-center gap-2">
      <Icon className="text-tertiary size-3.5" />
      {PANE_KIND_LABELS[kind] ?? kind}
    </span>
  )
}
