/**
 * 布局快捷键的组合键记法与显示。
 *
 * 组合键存成 `"Meta+Shift+Digit3"` 这种**按物理键位**（`event.code`）的字符串：
 * - 用 code 而不是 key：macOS 的 Option 会改写 `key`（⌥1 → `¡`），拿 key 比对在真机上会漏；
 * - 修饰键固定顺序 Ctrl/Meta/Alt/Shift，同一个键只会有一个写法，便于查重与比较。
 */

/** 修饰键顺序：规范化后同一个组合只有一种字符串。 */
const MODIFIER_ORDER = ["Ctrl", "Meta", "Alt", "Shift"] as const

const DISPLAY_MODIFIERS: Record<string, string> = {
  Ctrl: "⌃",
  Meta: "⌘",
  Alt: "⌥",
  Shift: "⇧",
}

/** 只按了修饰键时不让它成为快捷键。 */
const MODIFIER_CODES = new Set([
  "MetaLeft",
  "MetaRight",
  "ControlLeft",
  "ControlRight",
  "AltLeft",
  "AltRight",
  "ShiftLeft",
  "ShiftRight",
  "CapsLock",
])

/** 结果用 `in` 判别（这个项目的 tsconfig 没开 strict，字面量判别式不可靠）。 */
export type ComboResult = { combo: string } | { reason: string }

/**
 * 已经被工作区占用的组合键：`⌘,` 打开设置、`⌘↵` 放大当前格。
 * 布局快捷键不能抢这两个，否则用户会突然打不开设置。
 */
const RESERVED_COMBOS: Record<string, string> = {
  "Meta+Comma": "⌘, 是打开设置的快捷键",
  "Ctrl+Comma": "⌘, 是打开设置的快捷键",
  "Meta+KeyE": "⌘E 是编辑当前布局的快捷键",
  "Ctrl+KeyE": "⌘E 是编辑当前布局的快捷键",
  "Meta+KeyS": "⌘S 是保存布局的快捷键",
  "Ctrl+KeyS": "⌘S 是保存布局的快捷键",
  "Meta+Enter": "⌘↵ 是放大当前格的快捷键",
  "Ctrl+Enter": "⌘↵ 是放大当前格的快捷键",
}

/** 把一次 keydown 转成规范化的组合键；不带 ⌘/⌃ 的按下直接拒绝（否则会抢打字）。 */
export function comboFromEvent(event: KeyboardEvent): ComboResult {
  if (MODIFIER_CODES.has(event.code)) {
    return { reason: "需要再按一个字母或数字" }
  }
  if (!event.metaKey && !event.ctrlKey) {
    return { reason: "快捷键需要带 ⌘ 或 ⌃" }
  }
  const pressed = new Set<string>()
  if (event.ctrlKey) pressed.add("Ctrl")
  if (event.metaKey) pressed.add("Meta")
  if (event.altKey) pressed.add("Alt")
  if (event.shiftKey) pressed.add("Shift")
  const modifiers = MODIFIER_ORDER.filter((name) => pressed.has(name))
  const combo = [...modifiers, event.code].join("+")
  const reserved = RESERVED_COMBOS[combo]
  if (reserved) return { reason: reserved }
  return { combo }
}

/** 当前按下的组合键（不校验修饰键），用于在录制过程中与已保存的键比较。 */
export function pressedCombo(event: KeyboardEvent): string {
  const pressed = new Set<string>()
  if (event.ctrlKey) pressed.add("Ctrl")
  if (event.metaKey) pressed.add("Meta")
  if (event.altKey) pressed.add("Alt")
  if (event.shiftKey) pressed.add("Shift")
  return [...MODIFIER_ORDER.filter((name) => pressed.has(name)), event.code].join("+")
}

/** 键盘码 → 显示用的字符：`KeyK` → `K`，`Digit1` → `1`，`BracketLeft` → `[`。 */
const CODE_LABELS: Record<string, string> = {
  Digit0: "0",
  Digit1: "1",
  Digit2: "2",
  Digit3: "3",
  Digit4: "4",
  Digit5: "5",
  Digit6: "6",
  Digit7: "7",
  Digit8: "8",
  Digit9: "9",
  Minus: "-",
  Equal: "=",
  BracketLeft: "[",
  BracketRight: "]",
  Semicolon: ";",
  Quote: "'",
  Comma: ",",
  Period: ".",
  Slash: "/",
  Backslash: "\\",
  Backquote: "`",
  Space: "Space",
  Enter: "↵",
  Tab: "⇥",
  ArrowUp: "↑",
  ArrowDown: "↓",
  ArrowLeft: "←",
  ArrowRight: "→",
}

function codeLabel(code: string): string {
  return CODE_LABELS[code] ?? code.replace(/^Key/, "")
}

/** `"Meta+Shift+Digit3"` → `"⌘⇧3"`。 */
export function formatCombo(combo: string): string {
  const parts = combo.split("+")
  const code = parts.pop() ?? ""
  const modifiers = parts.map((name) => DISPLAY_MODIFIERS[name] ?? name).join("")
  return `${modifiers}${codeLabel(code)}`
}

/**
 * 默认快捷键：第 n 个布局给 `⌘n`（前 9 个）。
 *
 * 用户没自定义过的布局沿用这个位置约定，所以"删除自定义快捷键"= 回到默认位置，
 * 而不是变成一个没标签的布局。
 */
export function defaultCombo(index: number): string | null {
  return index >= 0 && index < 9 ? `Meta+Digit${index + 1}` : null
}
