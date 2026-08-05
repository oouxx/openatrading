import { useState, useEffect } from "react"
import { Input } from "@/components/ui/input"

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
  const [display, setDisplay] = useState<string[]>(() =>
    items.length > 0 ? [...items, ""] : [""]
  )

  // Sync from parent when items change externally (e.g., transport type switch)
  useEffect(() => {
    setDisplay(items.length > 0 ? [...items, ""] : [""])
  }, [items])

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
                className="w-28 font-mono text-caption"
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
              className="flex-1 font-mono text-caption"
            />
            {!isLast && (
              <button
                type="button"
                className="hover:text-down text-secondary shrink-0 self-center px-1 text-caption"
                onClick={() => commit(display.filter((_, j) => j !== i))}
              >
                ✕
              </button>
            )}
          </div>
        )
      })}
      <button
        type="button"
        className="text-secondary text-caption hover:text-primary"
        onClick={() => setDisplay([...display, ""])}
      >
        + 添加
      </button>
    </div>
  )
}
