import { useState } from "react"
import { Input } from "@/components/ui/input"
import { Button } from "@/components/ui/button"
import { X } from "lucide-react"

export function KeyValueList({
  items,
  onChange,
  keyPlaceholder,
  valuePlaceholder,
  hideKey,
}: {
  items: string[]
  onChange: (entries: string[]) => void
  keyPlaceholder?: string
  valuePlaceholder?: string
  hideKey?: boolean
}) {
  // Internal display list: always has a trailing empty row for adding new entries
  const [display, setDisplay] = useState<string[]>(() => (items.length > 0 ? [...items, ""] : [""]))
  const [prevItems, setPrevItems] = useState(items)

  // Sync from parent when items change externally (e.g., transport type switch)
  if (prevItems !== items) {
    setPrevItems(items)
    setDisplay(items.length > 0 ? [...items, ""] : [""])
  }

  const commit = (next: string[]) => {
    const nonEmpty = next.filter((e) => e.trim())
    setDisplay([...nonEmpty, ""])
    onChange(nonEmpty)
  }

  return (
    <div className="space-y-1.5">
      {display.map((entry, i) => {
        const eqIdx = entry.indexOf("=")
        const key = eqIdx > 0 ? entry.slice(0, eqIdx) : ""
        const val = eqIdx > 0 ? entry.slice(eqIdx + 1) : entry
        const isLast = i === display.length - 1
        return (
          <div key={i} className="flex gap-2">
            {!hideKey && (
              <Input
                value={key}
                onChange={(e) => {
                  const next = [...display]
                  next[i] = `${e.target.value}=${val}`
                  commit(next)
                }}
                placeholder={keyPlaceholder ?? "KEY"}
                className="font-number text-caption w-28"
              />
            )}
            <Input
              value={val}
              onChange={(e) => {
                const next = [...display]
                next[i] = hideKey ? e.target.value : `${key}=${e.target.value}`
                commit(next)
              }}
              placeholder={valuePlaceholder ?? "VALUE"}
              className="font-number text-caption flex-1"
            />
            {!isLast && (
              <Button
                type="button"
                variant="ghost"
                size="icon-xs"
                className="text-secondary hover:text-down shrink-0 self-center"
                onClick={() => commit(display.filter((_, j) => j !== i))}
                title="删除"
              >
                <X className="size-3" />
              </Button>
            )}
          </div>
        )
      })}
      <Button
        type="button"
        variant="ghost"
        size="xs"
        className="text-secondary hover:text-primary w-fit"
        onClick={() => setDisplay([...display, ""])}
      >
        + 添加
      </Button>
    </div>
  )
}
