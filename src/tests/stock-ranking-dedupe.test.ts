import { describe, expect, test } from "bun:test"

import { mergeStockRankingPages } from "../src/lib/stock-ranking"

describe("stock ranking pagination", () => {
  test("deduplicates stocks repeated across page boundaries", () => {
    const stock = (symbol: string, name: string) => ({ symbol, name })
    const merged = mergeStockRankingPages([
      { items: [stock("000893.SZ", "亚钾国际"), stock("600000.SS", "浦发银行")] },
      { items: [stock("000893.SZ", "亚钾国际"), stock("300750.SZ", "宁德时代")] },
    ])

    expect(merged.map((item) => item.symbol)).toEqual([
      "000893.SZ",
      "600000.SS",
      "300750.SZ",
    ])
  })
})
