/**
 * 榜单"实时订阅窗口"：把可视行号区间扩成按块对齐的区间。
 *
 * 为什么按块取整：可视区每次滚动都会变，直接拿"可视行"订阅会让 SSE 连接在滚动
 * 过程中不停重建；对齐到 30 行一块后，只有跨块才换订阅。向前后各留一块，滚起来
 * 不用等新连接。
 */
export function liveWindow(
  range: { min: number; max: number } | null,
  total: number,
  block = 30
): { start: number; end: number } {
  if (!range || total <= 0) return { start: 0, end: 0 }
  const start = Math.max(0, Math.floor(range.min / block) * block - block)
  const end = Math.min(total, Math.ceil((range.max + 1) / block) * block + block)
  return { start, end }
}
