export interface SymbolKeyed {
  symbol: string
}

/** Merge ranked pages without duplicating a stock that moved between page
 *  boundaries while the user was scrolling. The first occurrence keeps its
 *  original rank position. */
export function mergeStockRankingPages<T extends SymbolKeyed>(
  pages: readonly { items: T[] }[]
): T[] {
  const seen = new Set<string>()
  const merged: T[] = []
  for (const page of pages) {
    for (const stock of page.items) {
      if (seen.has(stock.symbol)) continue
      seen.add(stock.symbol)
      merged.push(stock)
    }
  }
  return merged
}
