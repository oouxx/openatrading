import { useEffect } from "react"
import { eachLeaf, useLayoutStore } from "@/stores/layoutStore"
import { useRightPanelStore } from "@/stores/rightPanelStore"
import { useUiStore } from "@/stores/ui"

/**
 * 工作区的全局快捷键。
 *
 * - `⌘,` 打开设置（macOS 惯例，设置里的「布局」页就是布局管理入口）
 * - `⌥⌘1–9` 切换布局，顺序与设置里的列表一致（前三档是内置预设）。
 *   注意 macOS 的 Option 会把数字键变成符号（⌥2 = "™"），所以按 `event.code` 判断。
 * - `⌘⇧F` 把光标所在的格子放大/还原；没有指向时放大第一格
 * - `Esc` 先还原放大的格子，再退出布局结构编辑态
 *
 * 不用 `⌘⇧1..3`：macOS 把 `⌘⇧3/4` 占给了系统截图，按下时事件根本到不了页面。
 */
export function useWorkspaceShortcuts() {
  const setLayout = useLayoutStore((s) => s.setLayout)
  const stopEditing = useLayoutStore((s) => s.stopEditing)

  useEffect(() => {
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

      if (!mod || !event.altKey || event.shiftKey) return
      // macOS 上 Option 会改写 key：⌥1 得到的是 "¡"、⌥2 是 "™"、⌥3 是 "£"……
      // 所以数字必须读 event.code（Digit1..Digit9），不能用 event.key 比对。
      const digit =
        /^Digit([1-9])$/.exec(event.code)?.[1] ?? (/^[1-9]$/.test(event.key) ? event.key : "")
      if (!digit) return
      const index = Number(digit) - 1

      const layout = useLayoutStore.getState().layouts[index]
      if (!layout) return
      event.preventDefault()
      // 布局只在对话态可见，从设置页按快捷键要顺带回到工作台
      if (useUiStore.getState().mode !== "chat") useUiStore.getState().setMode("chat")
      setLayout(layout.id)
    }
    window.addEventListener("keydown", handler)
    return () => window.removeEventListener("keydown", handler)
  }, [setLayout, stopEditing])
}
