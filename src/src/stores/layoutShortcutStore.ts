import { create } from "zustand"
import { defaultCombo } from "@/lib/shortcuts"

/**
 * 布局的自定义快捷键（`布局 id → 组合键`）。
 *
 * `null` = 用户主动清掉（这个布局没有快捷键），和"没配过（用默认 ⌘n）"是两件事，
 * 所以值用 `string | null` 而不是省略键。
 *
 * 和面板宽度一样只存本机：快捷键取决于你这台机器的键盘与系统占位，换设备照搬
 * 很容易撞上那边已经用掉的组合。
 */
const STORAGE_KEY = "openatrading-layout-shortcuts"

type ComboMap = Record<string, string | null>

function readCombos(): ComboMap {
  try {
    const raw: unknown = JSON.parse(localStorage.getItem(STORAGE_KEY) ?? "{}")
    if (!raw || typeof raw !== "object") return {}
    return Object.fromEntries(
      Object.entries(raw as Record<string, unknown>).filter(
        (entry): entry is [string, string | null] =>
          entry[1] === null || typeof entry[1] === "string"
      )
    )
  } catch {
    return {}
  }
}

interface LayoutShortcutState {
  combos: ComboMap
  /** 写入（传 null = 清除这个布局的快捷键）。 */
  set: (layoutId: string, combo: string | null) => void
  /** 恢复默认（按位置给 ⌘n）。 */
  reset: (layoutId: string) => void
}

export const useLayoutShortcutStore = create<LayoutShortcutState>((set, get) => {
  const persist = (combos: ComboMap) => {
    try {
      localStorage.setItem(STORAGE_KEY, JSON.stringify(combos))
    } catch {
      // 隐私模式下写不进去：本次会话内仍然生效
    }
    set({ combos })
  }

  return {
    combos: readCombos(),

    set: (layoutId, combo) => persist({ ...get().combos, [layoutId]: combo }),

    reset: (layoutId) => {
      const combos = { ...get().combos }
      delete combos[layoutId]
      persist(combos)
    },
  }
})

/** 某个布局当前生效的组合键：自定义 > 默认 ⌘n > 无。 */
export function combosFor(
  layouts: { id: string }[],
  combos: ComboMap
): { layoutId: string; combo: string }[] {
  const out: { layoutId: string; combo: string }[] = []
  layouts.forEach((layout, index) => {
    const combo = layout.id in combos ? combos[layout.id] : defaultCombo(index)
    if (combo) out.push({ layoutId: layout.id, combo })
  })
  return out
}

/** 这个组合键已经给了哪个布局（查重用）。 */
export function comboOwner(
  layouts: { id: string; name: string }[],
  combos: ComboMap,
  combo: string,
  exceptLayoutId: string
): string | null {
  const taken = combosFor(layouts, combos).find(
    (item) => item.combo === combo && item.layoutId !== exceptLayoutId
  )
  if (!taken) return null
  return layouts.find((layout) => layout.id === taken.layoutId)?.name ?? taken.layoutId
}
