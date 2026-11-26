/**
 * 模拟盘下单的纯计算：整手、股数上限、快捷仓位。
 *
 * 原来这些都长在 `RoleView` 里，买入/卖出一旦要搬到别处（网格里的「下单」格）
 * 就得复制一遍；统一放这里，弹窗和格子用同一套口径。
 */

/** A 股整手股数。买入必须是它的整数倍（与后端 `Shares::require_board_lot` 一致）。 */
export const BOARD_LOT = 100

/**
 * 只接受不会被后端 `normalize_code` 误解析的输入：
 * 裸 6 位代码，或带 sh/sz/ss 前缀/后缀的 6 位代码。
 * 像 "360" 这种 3 位数字后端会补成 000360，必须在前端直接拒绝。
 */
const SYMBOL_CODE_RE = /^(?:(?:sh|sz|ss)[.\-_]?(\d{6})|(\d{6})(?:\.(?:ss|sz))?)$/i

export function extractStockCode(raw: string): string | null {
  const match = SYMBOL_CODE_RE.exec(raw.trim())
  return match ? (match[1] ?? match[2] ?? null) : null
}

/**
 * 按可用资金与价格折算买入股数，向下取整到整手。
 * 资金/价格不可用时返回 0，调用方据此隐藏快捷仓位。
 */
export function maxBuyableShares(cash: number | null, price: number): number {
  if (cash === null || !Number.isFinite(price) || price <= 0 || cash <= 0) return 0
  return Math.floor(cash / price / BOARD_LOT) * BOARD_LOT
}

/**
 * 快捷仓位比例：买入按可用资金折算，卖出按可卖股数折算。
 *
 * 买/卖共用一张表，标签只有一份——两边各写一遍很容易在改比例时漏改一边。
 */
const POSITION_RATIOS: { label: string; ratio: number }[] = [
  { label: "1/4 仓", ratio: 1 / 4 },
  { label: "1/3 仓", ratio: 1 / 3 },
  { label: "半仓", ratio: 1 / 2 },
  { label: "全仓", ratio: 1 },
]

/** 买入股数快捷比例 —— 按可用资金折算成「几成仓」，从轻仓到满仓排列 */
export function buyShareOptions(maxShares: number): { label: string; shares: number }[] {
  const options: { label: string; shares: number }[] = []
  for (const { label, ratio } of POSITION_RATIOS) {
    const shares = ratio >= 1 ? maxShares : Math.floor((maxShares * ratio) / BOARD_LOT) * BOARD_LOT
    // 资金少时不同比例会落到同一个整手数，重复的按钮没有意义
    if (shares >= BOARD_LOT && !options.some((o) => o.shares === shares)) {
      options.push({ label, shares })
    }
  }
  return options
}

/** 卖出股数快捷比例 —— 「全部」取可卖股数，其余按整手折算，从大仓到小仓排列 */
export function sellShareOptions(sellable: number): { label: string; shares: number }[] {
  const options = [{ label: "全部", shares: sellable }]
  for (const { label, ratio } of [...POSITION_RATIOS].reverse()) {
    // 「全仓」就是「全部」，不重复出两个同义按钮
    if (ratio >= 1) continue
    const shares = Math.floor((sellable * ratio) / BOARD_LOT) * BOARD_LOT
    // 仓位小时不同比例会落到同一个整手数，重复的按钮没有意义
    if (shares >= BOARD_LOT && !options.some((o) => o.shares === shares)) {
      options.push({ label, shares })
    }
  }
  return options
}
