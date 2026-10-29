import { describe, expect, test } from "bun:test"

describe("stock ranking query pagination", () => {
  test("loads 30 rows first and 100 rows on later pages", async () => {
    Object.defineProperty(globalThis, "localStorage", {
      configurable: true,
      value: { getItem: () => null, setItem: () => {}, removeItem: () => {} },
    })
    Object.defineProperty(globalThis, "window", {
      configurable: true,
      value: {},
    })
    const { stockRankingQueryOptions } = await import("../src/hooks/query/useStockRanking")
    const options = stockRankingQueryOptions("gainers", "all", true)
    expect(options.initialPageParam).toEqual({ offset: 0, limit: 30 })

    const next = options.getNextPageParam({
      items: [],
      next_offset: 30,
    })
    expect(next).toEqual({ offset: 30, limit: 100 })
  })

  test("stops when the backend reports no next offset", async () => {
    Object.defineProperty(globalThis, "localStorage", {
      configurable: true,
      value: { getItem: () => null, setItem: () => {}, removeItem: () => {} },
    })
    Object.defineProperty(globalThis, "window", {
      configurable: true,
      value: {},
    })
    const { stockRankingQueryOptions } = await import("../src/hooks/query/useStockRanking")
    const options = stockRankingQueryOptions("gainers", "all", true)
    expect(
      options.getNextPageParam({
        items: [],
        next_offset: null,
      })
    ).toBeUndefined()
  })
})
