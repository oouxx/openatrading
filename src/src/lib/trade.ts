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

/** 买入股数快捷比例 —— 按可用资金折算成「几成仓」 */
export const BUY_SHARE_RATIOS: { label: string; ratio: number }[] = [
  { label: "1/3 仓", ratio: 1 / 3 },
  { label: "半仓", ratio: 1 / 2 },
  { label: "全仓", ratio: 1 },
]

/** 卖出股数快捷比例 —— 「全部」取实际持仓，其余按整手折算 */
export function sellShareOptions(held: number): { label: string; shares: number }[] {
  const options = [{ label: "全部", shares: held }]
  for (const ratio of [1 / 2, 1 / 3]) {
    const shares = Math.floor((held * ratio) / BOARD_LOT) * BOARD_LOT
    if (shares >= BOARD_LOT && shares < held) {
      options.push({ label: ratio === 1 / 2 ? "半仓" : "1/3 仓", shares })
    }
  }
  return options
}
