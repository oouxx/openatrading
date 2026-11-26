import { useUiHintStore } from "@/stores/uiHintStore"

/**
 * 点股票却没地方看时，说清楚一次就够（每种情况各提示一次，之后不再打扰）。
 *
 * - `unlinked`：这一格没进任何联动组
 * - `no-receiver`：进了联动组，但组里没有「个股详情」或对话格
 */
export type PaneLinkHintReason = "unlinked" | "no-receiver"

const HINT_KEY = "openatrading-pane-link-hint"

const TEXT: Record<PaneLinkHintReason, string> = {
  unlinked:
    "这个布局里没有「个股详情」格：点股票只会换工作区标的。加一格「个股详情」，它默认就跟当前标的走",
  "no-receiver": "这个联动组里没有看盘格：把「个股详情」或对话格拖进来，点股票才有地方显示",
}

export function hintPaneLinkOnce(reason: PaneLinkHintReason) {
  const key = `${HINT_KEY}-${reason}`
  try {
    if (localStorage.getItem(key)) return
    localStorage.setItem(key, "1")
  } catch {
    // localStorage 不可用时每次都提示，总比不提示好
  }
  useUiHintStore.getState().show(TEXT[reason])
}
