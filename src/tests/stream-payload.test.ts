import { describe, expect, test } from "bun:test"
import { pickSymbolPayload } from "../src/lib/stream-payload"

const quote = (symbol: string) => ({ symbol, last_px: 1 })

describe("pickSymbolPayload", () => {
  test("精确命中", () => {
    const payload = { "600519.SS": quote("600519.SS"), "000001.SZ": quote("000001.SZ") }
    expect(pickSymbolPayload(payload, "600519.SS")).toEqual(quote("600519.SS"))
  })

  test("请求不带后缀 / 键不带后缀都能对上", () => {
    expect(pickSymbolPayload({ "600519.SS": quote("600519.SS") }, "600519")).toEqual(
      quote("600519.SS")
    )
    expect(pickSymbolPayload({ "600519": quote("600519") }, "600519.SS")).toEqual(quote("600519"))
  })

  test("并集里有多只票时不乱认：找不到就是 undefined", () => {
    const payload = { "600519.SS": quote("600519.SS"), "000001.SZ": quote("000001.SZ") }
    expect(pickSymbolPayload(payload, "300308.SZ")).toBeUndefined()
  })

  test("只有一条时兜底（单发接口的历史形状）", () => {
    expect(pickSymbolPayload({ "600519.SS": { candles: [] } }, "300308.SZ")).toEqual({
      candles: [],
    })
  })

  test("错误 payload 不当作数据", () => {
    expect(pickSymbolPayload({ error: "boom" }, "600519.SS")).toBeUndefined()
  })
})
