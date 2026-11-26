import { create } from "zustand"

/** 工作区当前"正在看"的那只股票。 */
export interface ActiveStock {
  /** 行情代码（如 `600519.SS`），K 线与盘口接口都吃这个格式 */
  code: string
  name: string
}

interface ActiveStockState {
  stock: ActiveStock | null
  set: (stock: ActiveStock | null) => void
}

/**
 * 工作区的当前标的：点榜单/自选里的股票、或在任意格里搜一只，都会更新它。
 * 「个股详情」（未解绑时）、五档行情、下单框都读同一份。
 *
 * 只存内存：这是"我现在在看哪只"的临时视图状态，和 `paneDetail` 一样不落盘——
 * 重启后回到"没选股票"，比恢复一只早就过期的票更符合预期。
 */
export const useActiveStockStore = create<ActiveStockState>((set) => ({
  stock: null,
  // 同一只票重复设置（一个联动组里有多个接收方时会连续 set）不再触发新快照，
  // 否则 K 线格要跟着白白重渲染一轮。
  set: (stock) => set((state) => (stock && state.stock?.code === stock.code ? state : { stock })),
}))
