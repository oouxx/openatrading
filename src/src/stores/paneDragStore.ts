import { create } from "zustand"

/**
 * 拖拽关联的临时状态（不落盘）。
 *
 * 手势：按住某一格角落的把手拖动 → 悬停到另一格上高亮 → 松手关联；松手在空白处
 * 或自己身上 = 把这一格从原来的关联里拖开（解除）。
 */
interface PaneDragState {
  /** 正在拖的格子。 */
  sourceId: string | null
  /** 光标下方的格子（不含自己）。 */
  hoverId: string | null
  start: (sourceId: string) => void
  hover: (hoverId: string | null) => void
  end: () => void
}

export const usePaneDragStore = create<PaneDragState>((set) => ({
  sourceId: null,
  hoverId: null,
  start: (sourceId) => set({ sourceId, hoverId: null }),
  hover: (hoverId) => set((state) => (state.hoverId === hoverId ? state : { hoverId })),
  end: () => set({ sourceId: null, hoverId: null }),
}))
