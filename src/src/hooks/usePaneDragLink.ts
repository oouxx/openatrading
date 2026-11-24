import { useCallback, useEffect, useRef } from "react"
import { usePaneDragStore } from "@/stores/paneDragStore"
import { linkedTarget, usePaneLinkStore } from "@/stores/paneLinkStore"

/** 小于这个位移算"点了一下"，不触发关联/解除。 */
const DRAG_THRESHOLD_PX = 6

/** 命中光标下方的格子（不返回 `exclude`）。 */
function paneUnder(x: number, y: number, exclude?: string | null): string | null {
  const el = document.elementFromPoint(x, y)
  const pane = el?.closest<HTMLElement>("[data-pane-id]")
  const id = pane?.dataset.paneId ?? null
  return id && id !== exclude ? id : null
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
      const wasLinked = linkedTarget(layoutId, sourceId) !== null
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
        if (moved && target) link(layoutId, sourceId, target)
        else if (moved && wasLinked) unlink(layoutId, sourceId)
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
