import { useEffect } from "react"
import { eachLeaf, useLayoutStore } from "@/stores/layoutStore"
import { combosFor, useLayoutShortcutStore } from "@/stores/layoutShortcutStore"
import { pressedCombo } from "@/lib/shortcuts"
import { useRightPanelStore } from "@/stores/rightPanelStore"
import { useUiStore } from "@/stores/ui"

/**
 * 工作区的全局快捷键。
 *
 * - 布局快捷键：默认 `⌘1–9` 按列表位置，可在 设置 · 布局 里逐个改成任意
 *   `⌘/⌃ + 键` 的组合（含 `⌘⇧1`、`⌘⌥K` 这类）
 * - `⌘,` 打开设置（设置里的「布局」页就是布局管理入口）
 * - `⌘⇧F` 把光标所在的格子放大/还原；没有指向时放大第一格
 * - `Esc` 先退子页面，再还原放大的格子，最后退出结构编辑态
 *
 * 几个按下注意：
 * - 数字按 `event.code`（Digit1..Digit9）判断：macOS 的 Option 会改写 `event.key`，
 *   拿 key 比对在真机上会漏（以前用 ⌥⌘数字时踩过）。
 * - 光标在输入框里时不抢键：数字键在文本框里另有语义。
 * - 浏览器把 `⌘1–9` 留给了标签页，web 端这组快捷键**不生效**（产品决定：不为 web 绕行，
 *   切布局走 设置 · 布局 里的「使用」）。桌面端没有标签页，正常工作。
 * - 不用 `⌘⇧1..3`：macOS 把 `⌘⇧3/4` 占给了系统截图。
 */
export function useWorkspaceShortcuts() {
  const setLayout = useLayoutStore((s) => s.setLayout)
  const stopEditing = useLayoutStore((s) => s.stopEditing)

  useEffect(() => {
    /** 光标在输入框里时让位给文本编辑，别把按键抢走。 */
    const inEditable = (event: KeyboardEvent) => {
      // 事件目标可能是 window / document（无焦点时的合成事件），先确认是元素
      const target = event.target
      if (!(target instanceof HTMLElement)) return false
      return (
        target.tagName === "INPUT" ||
        target.tagName === "TEXTAREA" ||
        target.isContentEditable ||
        target.getAttribute("role") === "textbox"
      )
    }

    const handler = (event: KeyboardEvent) => {
      const mod = event.metaKey || event.ctrlKey

      // ⌘, —— 打开设置。已经在设置里时保持在设置（不来回跳，避免误触丢焦点）
      if (mod && !event.altKey && event.key === ",") {
        event.preventDefault()
        useUiStore.getState().setMode("settings")
        return
      }

      // Esc 的优先级：先退"子页面"（K 线 / 板块详情），再还原放大的格子，最后退出编辑态
      if (event.key === "Escape") {
        const state = useLayoutStore.getState()
        const activeId = state.activePaneId
        const detailPane =
          activeId && state.paneDetail[activeId] ? activeId : Object.keys(state.paneDetail)[0]
        if (detailPane) {
          state.closePaneDetail(detailPane)
          return
        }
        const panel = useRightPanelStore.getState()
        if (panel.plateTarget) {
          panel.closePlate()
          return
        }
        if (state.maximizedPaneId) {
          state.setMaximizedPane(null)
          return
        }
        if (state.editingId) stopEditing()
        return
      }

      // ⌘⇧F —— 放大/还原光标所在的那一格
      if (mod && event.shiftKey && !event.altKey && event.key.toLowerCase() === "f") {
        event.preventDefault()
        const state = useLayoutStore.getState()
        const layout = state.layouts.find((item) => item.id === state.activeId)
        if (!layout) return
        if (state.maximizedPaneId) {
          state.setMaximizedPane(null)
          return
        }
        // 没有"当前格"（纯键盘操作）时，放大第一格总比什么都不做更有用
        let target = state.activePaneId
        if (!target) {
          eachLeaf(layout.root, (leaf) => {
            target ??= leaf.id
          })
        }
        if (target) state.setMaximizedPane(target)
        return
      }

      // 布局快捷键（默认 ⌘1–9，可在设置里自定义）
      if (!inEditable(event)) {
        const combo = pressedCombo(event)
        const bindings = combosFor(
          useLayoutStore.getState().layouts,
          useLayoutShortcutStore.getState().combos
        )
        // 默认键写成 ⌘n；Windows/Linux 上按 ⌃n 也要能中（沿用以前 meta||ctrl 的口径）
        const target =
          bindings.find((item) => item.combo === combo) ??
          (combo.startsWith("Ctrl+")
            ? bindings.find((item) => item.combo === combo.replace(/^Ctrl\+/, "Meta+"))
            : undefined)
        if (target) {
          event.preventDefault()
          if (useUiStore.getState().mode !== "chat") useUiStore.getState().setMode("chat")
          setLayout(target.layoutId)
          return
        }
      }
    }
    window.addEventListener("keydown", handler)
    return () => window.removeEventListener("keydown", handler)
  }, [setLayout, stopEditing])
}
