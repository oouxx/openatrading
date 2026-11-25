import { create } from "zustand"

/**
 * 一次性的顶部提示（比 toast 更轻：不排队、不叠层，新的直接顶掉旧的）。
 *
 * 用在"用户做了个动作但结果不符合预期"的场合——比如点股票却没有可联动的看盘格。
 * 和 `LayoutToast`（切布局/进出编辑态）共用同一个浮层位置。
 */
interface UiHintState {
  message: string | null
  show: (message: string, ttlMs?: number) => void
  clear: () => void
}

let clearTimer: ReturnType<typeof setTimeout> | null = null

export const useUiHintStore = create<UiHintState>((set) => ({
  message: null,
  show: (message, ttlMs = 3600) => {
    if (clearTimer) clearTimeout(clearTimer)
    set({ message })
    clearTimer = setTimeout(() => set({ message: null }), ttlMs)
  },
  clear: () => {
    if (clearTimer) clearTimeout(clearTimer)
    set({ message: null })
  },
}))
