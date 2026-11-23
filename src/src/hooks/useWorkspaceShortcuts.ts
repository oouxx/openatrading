import { useEffect } from "react"
import { eachLeaf, useLayoutStore } from "@/stores/layoutStore"
import { useRightPanelStore } from "@/stores/rightPanelStore"
import { useUiStore } from "@/stores/ui"

/**
 * 工作区的全局快捷键。
 *
 * - `⌘1–9` 直接跳到第 n 个布局（顺序与设置里的列表一致）
 * - `⌥⌘←/→` 按列表顺序循环切换（浏览器把 `⌘1–9` 给了标签页，web 端用这条）
 * - `⌘,` 打开设置（设置里的「布局」页就是布局管理入口）
 * - `⌘⇧F` 把光标所在的格子放大/还原；没有指向时放大第一格
 * - `Esc` 先退子页面，再还原放大的格子，最后退出结构编辑态
 *
 * 两个按下注意：
 * - 数字按 `event.code`（Digit1..Digit9）判断：macOS 的 Option 会改写 `event.key`
 *   （⌥2 得到 "™"），拿 key 比对在真机上永远匹配不到。
 * - 光标在输入框里时不抢键：`⌘1–9` / `⌥⌘←/→` 在文本框里各有别的语义。
 * - 不用 `⌘⇧1..3`：macOS 把 `⌘⇧3/4` 占给了系统截图。
 */
export function useWorkspaceShortcuts() {
  const setLayout = useLayoutStore((s) => s.setLayout)
  const stopEditing = useLayoutStore((s) => s.stopEditing)

  useEffect(() => {
    /** 光标在可编辑区域里时，方向键类快捷键要让位给文本编辑。 */
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

      // ⌘1–9 —— 直接跳第 n 个布局
      if (mod && !event.altKey && !event.shiftKey) {
        const digit =
          /^Digit([1-9])$/.exec(event.code)?.[1] ?? (/^[1-9]$/.test(event.key) ? event.key : "")
        const target = digit ? useLayoutStore.getState().layouts[Number(digit) - 1] : undefined
        if (target && !inEditable(event)) {
          event.preventDefault()
          if (useUiStore.getState().mode !== "chat") useUiStore.getState().setMode("chat")
          setLayout(target.id)
          return
        }
      }

      if (!mod || !event.altKey || event.shiftKey) return

      // ⌥⌘←/→ —— 按列表顺序循环切布局（浏览器把 ⌘1–9 给了标签页，web 端靠这条）
      if (event.key === "ArrowRight" || event.key === "ArrowLeft") {
        if (inEditable(event)) return
        event.preventDefault()
        if (useUiStore.getState().mode !== "chat") useUiStore.getState().setMode("chat")
        useLayoutStore.getState().cycleLayout(event.key === "ArrowRight" ? 1 : -1)
        return
      }
    }
    window.addEventListener("keydown", handler)
    return () => window.removeEventListener("keydown", handler)
  }, [setLayout, stopEditing])
}
