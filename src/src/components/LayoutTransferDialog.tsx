import { useMemo, useRef, useState } from "react"
import { Check, ClipboardCopy, Download, Upload } from "lucide-react"
import { Button } from "@/components/ui/button"
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from "@/components/ui/dialog"
import { Textarea } from "@/components/ui/textarea"
import { useAppMode } from "@/contexts/useAppMode"
import { serializeLayouts, useLayoutStore } from "@/stores/layoutStore"
import { cn } from "@/lib/utils"

type Mode = "import" | "export"

/**
 * 布局的搬家通道：导出成 JSON 贴给同事，或从他那儿粘回来。
 *
 * 只搬运"怎么排"——布局 id、面板尺寸存档都不带走，导进来永远是一份新的：
 * 同名布局不会互相覆盖，别人的拖拽结果也不会顶掉你调好的宽度。
 */
export default function LayoutTransferDialog({
  mode,
  onClose,
}: {
  mode: Mode | null
  onClose: () => void
}) {
  return (
    <Dialog open={mode !== null} onOpenChange={(open) => !open && onClose()}>
      <DialogContent className="sm:max-w-xl">
        <DialogHeader>
          <DialogTitle>{mode === "import" ? "导入布局" : "导出布局"}</DialogTitle>
          <DialogDescription className="text-tertiary text-caption">
            {mode === "import"
              ? "粘贴布局 JSON，或选择之前导出的文件。导入后会自动切到新布局。"
              : "复制 JSON 或下载文件，对方在「导入布局」里贴回来即可。"}
          </DialogDescription>
        </DialogHeader>
        {/* 按 mode 重新挂载，切换导入/导出时输入框和报错状态不会串 */}
        {mode === "import" ? <ImportPanel onClose={onClose} /> : <ExportPanel onClose={onClose} />}
      </DialogContent>
    </Dialog>
  )
}

function ImportPanel({ onClose }: { onClose: () => void }) {
  const importLayouts = useLayoutStore((s) => s.importLayouts)
  const { mode, setMode } = useAppMode()
  const [text, setText] = useState("")
  const [error, setError] = useState<string | null>(null)
  const fileRef = useRef<HTMLInputElement | null>(null)

  const submit = () => {
    if (!text.trim()) {
      setError("先粘贴一段布局 JSON")
      return
    }
    const result = importLayouts(text)
    if (!result.ok) {
      setError(result.error ?? "导入失败")
      return
    }
    // 导入后立刻切过去看效果；在设置/技能页导入时也要把主区带回工作台
    if (mode !== "chat") setMode("chat")
    onClose()
  }

  const pickFile = async (file: File | undefined) => {
    if (!file) return
    try {
      const content = await file.text()
      setText(content)
      setError(null)
    } catch {
      setError("读取文件失败")
    }
  }

  return (
    <>
      <Textarea
        autoFocus
        value={text}
        onChange={(e) => {
          setText(e.target.value)
          setError(null)
        }}
        placeholder='{ "layouts": [ { "name": "我的盯盘", "root": { … } } ] }'
        spellCheck={false}
        aria-label="布局 JSON"
        className="font-number text-caption h-56 resize-none"
      />
      {error && (
        <p role="status" className="text-danger text-caption">
          {error}
        </p>
      )}
      <input
        ref={fileRef}
        type="file"
        accept=".json,application/json"
        className="hidden"
        onChange={(e) => {
          void pickFile(e.target.files?.[0])
          // 同一个文件连续选两次也要触发 change
          e.target.value = ""
        }}
      />
      <DialogFooter>
        <Button variant="ghost" size="sm" onClick={() => fileRef.current?.click()}>
          <Upload />
          选择文件
        </Button>
        <Button size="sm" onClick={submit} disabled={!text.trim()}>
          导入
        </Button>
      </DialogFooter>
    </>
  )
}

function ExportPanel({ onClose }: { onClose: () => void }) {
  const layouts = useLayoutStore((s) => s.layouts)
  const activeId = useLayoutStore((s) => s.activeId)
  const customLayouts = useMemo(() => layouts.filter((layout) => !layout.builtin), [layouts])
  const active = layouts.find((layout) => layout.id === activeId)
  const [scope, setScope] = useState<"current" | "all">("current")
  const [copied, setCopied] = useState(false)

  const json = useMemo(() => {
    const targets = scope === "all" ? customLayouts : active ? [active] : []
    return JSON.stringify(serializeLayouts(targets), null, 2)
  }, [scope, customLayouts, active])

  const copy = () => {
    void navigator.clipboard?.writeText(json).then(() => {
      setCopied(true)
      window.setTimeout(() => setCopied(false), 1500)
    })
  }

  const download = () => {
    const blob = new Blob([json], { type: "application/json" })
    const url = URL.createObjectURL(blob)
    const link = document.createElement("a")
    link.href = url
    link.download = `openatrading-layout-${scope === "all" ? "all" : (active?.name ?? "layout")}.json`
    link.click()
    URL.revokeObjectURL(url)
  }

  return (
    <>
      {customLayouts.length > 0 && (
        <div className="bg-canvas border-hairline flex gap-0.5 rounded-md border p-0.5">
          {(
            [
              { id: "current", label: `当前布局（${active?.name ?? "—"}）` },
              { id: "all", label: `全部自定义（${customLayouts.length}）` },
            ] as const
          ).map((option) => (
            <Button
              key={option.id}
              variant="ghost"
              size="xs"
              aria-pressed={scope === option.id}
              onClick={() => setScope(option.id)}
              className={cn(
                "text-caption h-7 flex-1 rounded-sm",
                scope === option.id
                  ? "bg-surface text-primary hover:bg-surface shadow-xs"
                  : "text-tertiary hover:text-primary"
              )}
            >
              {option.label}
            </Button>
          ))}
        </div>
      )}
      <Textarea
        readOnly
        value={json}
        aria-label="布局 JSON"
        spellCheck={false}
        className="font-number text-caption h-56 resize-none"
      />
      <DialogFooter>
        <Button variant="ghost" size="sm" onClick={download}>
          <Download />
          下载 .json
        </Button>
        <Button size="sm" onClick={copy}>
          {copied ? <Check /> : <ClipboardCopy />}
          {copied ? "已复制" : "复制"}
        </Button>
        <Button variant="ghost" size="sm" onClick={onClose}>
          关闭
        </Button>
      </DialogFooter>
    </>
  )
}
