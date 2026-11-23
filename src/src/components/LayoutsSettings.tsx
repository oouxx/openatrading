import { useState } from "react"
import {
  Check,
  Download,
  Info,
  Grid2x2,
  LayoutGrid,
  LayoutPanelLeft,
  Pencil,
  Plus,
  RotateCcw,
  Trash2,
  Upload,
  Wallet,
} from "lucide-react"
import { Button } from "@/components/ui/button"
import { Input } from "@/components/ui/input"
import LayoutTransferDialog from "@/components/LayoutTransferDialog"
import { useAppMode } from "@/contexts/useAppMode"
import { useLayoutStore, type WorkspaceLayout } from "@/stores/layoutStore"
import { cn } from "@/lib/utils"

/** 内置布局的固定图标；自定义布局统一用网格图标。 */
const LAYOUT_ICONS: Record<string, typeof Grid2x2> = {
  review: LayoutPanelLeft,
  board: Grid2x2,
  ledger: Wallet,
}
const CUSTOM_ICON = LayoutGrid

/** 行内重命名：回车/失焦提交，Esc 放弃。 */
function RenameInput({ layout, onDone }: { layout: WorkspaceLayout; onDone: () => void }) {
  const renameLayout = useLayoutStore((s) => s.renameLayout)
  const [value, setValue] = useState(layout.name)

  const commit = () => {
    if (value.trim() && value.trim() !== layout.name) renameLayout(layout.id, value)
    onDone()
  }

  return (
    <Input
      autoFocus
      value={value}
      aria-label="布局名称"
      onChange={(e) => setValue(e.target.value)}
      onBlur={commit}
      onKeyDown={(e) => {
        if (e.key === "Enter") e.currentTarget.blur()
        if (e.key === "Escape") onDone()
      }}
      className="text-body h-8 max-w-64 rounded-md"
    />
  )
}

/**
 * 设置 · 布局。
 *
 * 布局是"工作区的形状"，属于配置而不是每小时的按钮——所以它住在设置里，
 * 工作台上不再常年挂一个下拉。切换靠 ⌥⌘1–9，⌘, 随时回到这一页。
 *
 * 结构编辑仍在画布上做（每格右下角的拆分工具条），这里的「编辑结构」只是
 * 打开工作台并进入编辑态——双份编辑器比没有更糟。
 */
export default function LayoutsSettings() {
  const layouts = useLayoutStore((s) => s.layouts)
  const activeId = useLayoutStore((s) => s.activeId)
  const presetKinds = useLayoutStore((s) => s.presetKinds)
  const setLayout = useLayoutStore((s) => s.setLayout)
  const cloneLayout = useLayoutStore((s) => s.cloneLayout)
  const createBlankLayout = useLayoutStore((s) => s.createBlankLayout)
  const deleteLayout = useLayoutStore((s) => s.deleteLayout)
  const resetPreset = useLayoutStore((s) => s.resetPreset)
  const startEditing = useLayoutStore((s) => s.startEditing)
  const { mode, setMode } = useAppMode()
  const [renamingId, setRenamingId] = useState<string | null>(null)
  const [transfer, setTransfer] = useState<"import" | "export" | null>(null)

  /** 任何"去工作台"的动作都要先切回 chat，否则改了也看不见。 */
  const toWorkspace = () => {
    if (mode !== "chat") setMode("chat")
  }

  const openStructure = (layout: WorkspaceLayout) => {
    // 内置布局结构只读：克隆一份再改
    if (layout.builtin) cloneLayout(layout.id)
    else startEditing(layout.id)
    toWorkspace()
  }

  return (
    <div className="space-y-6">
      <section className="bg-surface border-hairline rounded-lg border px-5 py-4">
        <div className="mb-1.5 flex flex-wrap items-baseline justify-between gap-2">
          <h2 className="text-primary text-body font-semibold">工作区布局</h2>
          <span className="text-tertiary text-micro">⌘1–9 切换 · ⌥⌘←/→ 循环 · ⌘, 回到这里</span>
        </div>

        <ul className="divide-hairline divide-y">
          {layouts.map((layout) => {
            const Icon = LAYOUT_ICONS[layout.id] ?? CUSTOM_ICON
            const isActive = layout.id === activeId
            const shortcutIndex = layouts.indexOf(layout)
            const hasOverrides = Object.keys(presetKinds[layout.id] ?? {}).length > 0
            return (
              <li
                key={layout.id}
                className="flex min-h-[52px] flex-wrap items-center justify-between gap-3 py-3"
              >
                <div className="flex min-w-0 items-center gap-2.5">
                  <Icon
                    className={cn("size-4 shrink-0", isActive ? "text-brand" : "text-tertiary")}
                  />
                  {renamingId === layout.id ? (
                    <RenameInput layout={layout} onDone={() => setRenamingId(null)} />
                  ) : (
                    <div className="min-w-0">
                      <p className="text-body text-primary flex items-center gap-1.5 font-medium">
                        <span className="truncate">{layout.name}</span>
                        {isActive && <Check className="text-brand size-3.5 shrink-0" />}
                      </p>
                      <p className="text-tertiary text-caption mt-0.5">
                        {layout.builtin ? "内置" : "自定义"}
                        {shortcutIndex < 9 && ` · ⌘${shortcutIndex + 1}`}
                      </p>
                    </div>
                  )}
                </div>

                <div className="flex shrink-0 items-center gap-1">
                  {!isActive && (
                    <Button
                      variant="ghost"
                      size="xs"
                      className="text-secondary hover:text-primary"
                      onClick={() => {
                        setLayout(layout.id)
                        toWorkspace()
                      }}
                    >
                      使用
                    </Button>
                  )}
                  <Button
                    variant="ghost"
                    size="xs"
                    className="text-secondary hover:text-primary gap-1.5"
                    onClick={() => openStructure(layout)}
                  >
                    <Pencil />
                    {layout.builtin ? "克隆并编辑" : "编辑结构"}
                  </Button>
                  {!layout.builtin && (
                    <>
                      <Button
                        variant="ghost"
                        size="xs"
                        className="text-secondary hover:text-primary"
                        onClick={() => setRenamingId(layout.id)}
                      >
                        重命名
                      </Button>
                      <Button
                        variant="ghost"
                        size="icon-xs"
                        aria-label={`删除 ${layout.name}`}
                        title="删除布局"
                        className="text-tertiary hover:text-danger"
                        onClick={() => deleteLayout(layout.id)}
                      >
                        <Trash2 />
                      </Button>
                    </>
                  )}
                  {layout.builtin && hasOverrides && (
                    <Button
                      variant="ghost"
                      size="icon-xs"
                      aria-label={`恢复 ${layout.name} 的默认内容`}
                      title="恢复默认内容"
                      className="text-tertiary hover:text-primary"
                      onClick={() => resetPreset(layout.id)}
                    >
                      <RotateCcw />
                    </Button>
                  )}
                </div>
              </li>
            )
          })}
        </ul>

        <div className="border-hairline mt-2 flex flex-wrap items-center gap-1 border-t pt-3">
          <Button
            variant="ghost"
            size="xs"
            className="text-secondary hover:text-primary gap-1.5"
            onClick={() => {
              // 白板新建：不继承任何东西，拆几格、放什么，全从零开始选
              createBlankLayout()
              toWorkspace()
            }}
          >
            <Plus />
            新建布局
          </Button>
          <Button
            variant="ghost"
            size="xs"
            className="text-secondary hover:text-primary gap-1.5"
            onClick={() => setTransfer("import")}
          >
            <Upload />
            导入
          </Button>
          <Button
            variant="ghost"
            size="xs"
            className="text-secondary hover:text-primary gap-1.5"
            onClick={() => setTransfer("export")}
          >
            <Download />
            导出
          </Button>
          <span className="text-tertiary text-micro ml-auto hidden items-center gap-1 sm:flex">
            <Info className="size-3" />
            布局存服务端，换设备自动带过来
          </span>
        </div>
      </section>

      <div className="text-tertiary text-caption flex flex-col gap-1 px-1 leading-relaxed">
        <p className="flex items-start gap-1.5">
          <Info className="mt-0.5 size-3 shrink-0" aria-hidden />
          面板宽度属于这台机器，不同步；结构、内容与当前布局会同步。
        </p>
        <p className="flex items-start gap-1.5">
          <Info className="mt-0.5 size-3 shrink-0" aria-hidden />
          工作区里每格右上角可以把这一格放大到全屏；快捷键 ⌘⇧F，Esc 还原。
        </p>
      </div>

      <LayoutTransferDialog mode={transfer} onClose={() => setTransfer(null)} />
    </div>
  )
}
