import { useCallback, useEffect, useRef } from "react"
import { usePaneDragStore } from "@/stores/paneDragStore"
import { paneHasLink, usePaneLinkStore } from "@/stores/paneLinkStore"
import { findLeaf, isMarketPaneKind, resolvePaneKind, useLayoutStore } from "@/stores/layoutStore"

/** 小于这个位移算"点了一下"，不触发关联/解除。 */
const DRAG_THRESHOLD_PX = 6

/** 命中光标下方的格子（不返回 `exclude`）。 */
function paneUnder(x: number, y: number, exclude?: string | null): string | null {
  const el = document.elementFromPoint(x, y)
  const pane = el?.closest<HTMLElement>("[data-pane-id]")
  const id = pane?.dataset.paneId ?? null
  return id && id !== exclude ? id : null
}

/** 这一格是不是"点股票会派发"的派发方（行情榜 / 自选）。 */
function isDispatcher(layoutId: string, leafId: string): boolean {
  const state = useLayoutStore.getState()
  const layout = state.layouts.find((item) => item.id === layoutId)
  const leaf = layout ? findLeaf(layout.root, leafId) : null
  if (!layout || !leaf) return false
  const kind = resolvePaneKind(layout, leaf, state.presetKinds)
  return isMarketPaneKind(kind) || kind === "watchlist"
}

/**
 * 把一格拖到另一格上 = 关联，拖开 = 解除。
 *
 * 用 Pointer Events 而不是 HTML5 drag：这里要的是"光标下是哪一格"的命中测试 + 高亮，
 * 不需要拖影；HTML5 拖拽在 Tauri 里还要额外处理 dragover/dragImage。
 *
 * 返回值里的 `begin` 挂到格子角落的把手上：`onPointerDown={(e) => begin(leafId, e)}`。
 */
export function usePaneDragLink(layoutId: string) {
  const link = usePaneLinkStore((s) => s.link)
  const unlink = usePaneLinkStore((s) => s.unlink)
  const cleanupRef = useRef<(() => void) | null>(null)

  useEffect(() => () => cleanupRef.current?.(), [])

  const begin = useCallback(
    (sourceId: string, event: React.PointerEvent) => {
      if (event.button !== 0) return
      event.preventDefault()
      const startX = event.clientX
      const startY = event.clientY
      const wasLinked = paneHasLink(layoutId, sourceId)
      let moved = false
      usePaneDragStore.getState().start(sourceId)

      const onMove = (e: PointerEvent) => {
        if (!moved && Math.hypot(e.clientX - startX, e.clientY - startY) < DRAG_THRESHOLD_PX) {
          return
        }
        moved = true
        usePaneDragStore.getState().hover(paneUnder(e.clientX, e.clientY, sourceId))
      }

      const onUp = (e: PointerEvent) => {
        cleanup()
        const target = paneUnder(e.clientX, e.clientY, sourceId)
        if (moved && target) {
          // 两个方向都认：把接收方拖到榜单上也等于"榜单派发到接收方"。
          // 关联的方向永远是 派发方 → 接收方，跟谁被拖动无关。
          const sourceIsDispatcher = isDispatcher(layoutId, sourceId)
          const targetIsDispatcher = isDispatcher(layoutId, target)
          if (sourceIsDispatcher) link(layoutId, sourceId, target)
          else if (targetIsDispatcher) link(layoutId, target, sourceId)
          // 两边都不是派发方：没有"点股票"这回事，不建立无意义关联
        } else if (moved && wasLinked) {
          unlink(layoutId, sourceId)
        }
      }

      const cleanup = () => {
        window.removeEventListener("pointermove", onMove)
        window.removeEventListener("pointerup", onUp)
        window.removeEventListener("pointercancel", cleanup)
        usePaneDragStore.getState().end()
        cleanupRef.current = null
      }

      cleanupRef.current?.()
      cleanupRef.current = cleanup
      window.addEventListener("pointermove", onMove)
      window.addEventListener("pointerup", onUp)
      window.addEventListener("pointercancel", cleanup)
    },
    [layoutId, link, unlink]
  )

  return begin
}
