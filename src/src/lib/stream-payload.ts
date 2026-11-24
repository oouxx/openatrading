/**
 * 行情流 payload 取数：后端推的是"批量形状" `{ "600519.SS": <数据>, ... }`。
 *
 * 为什么需要这层：payload 是**全站订阅的并集**（见 `crates/web/src/market_stream.rs`），
 * 里面可能有很多只票，所以绝不能"取不到就退回第一条"——第一条可能是别人的股票。
 * 只有 payload 里确实只有一条时，才认为它就是这次请求的那条（单发接口 / 单票订阅
 * 的历史形状，防止符号格式漂移导致整块空白）。
 */
const SUFFIX = /\.(SS|SZ|BJ)$/i

function bare(code: string): string {
  return code.replace(/^[a-z]+/i, "").replace(SUFFIX, "")
}

export function pickSymbolPayload(
  payload: Record<string, unknown>,
  symbol: string
): unknown | undefined {
  if (symbol in payload) return payload[symbol]

  const wanted = bare(symbol)
  for (const [key, value] of Object.entries(payload)) {
    if (bare(key) === wanted) return value
  }

  // 单条 payload：只在"没有 error 字段"时才当作数据
  const keys = Object.keys(payload)
  if (keys.length === 1 && !("error" in payload)) return payload[keys[0]]
  return undefined
}
