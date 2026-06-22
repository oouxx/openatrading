// Shared stock symbol and chart URL utilities

export function normalizeSymbol(raw: string): string {
  const dot = raw.indexOf('.')
  if (dot !== -1) {
    const code = raw.substring(0, dot)
    let suffix = raw.substring(dot + 1).toLowerCase()
    if (suffix === 'ss') suffix = 'sh'
    return suffix + code
  }
  return (raw.startsWith('6') ? 'sh' : 'sz') + raw
}

export function getKlineImageUrl(symbol: string, period: string): string {
  const s = normalizeSymbol(symbol)
  switch (period) {
    case 'minute': return `https://image.sinajs.cn/newchart/min/n/${s}.gif`
    case 'day': return `https://image.sinajs.cn/newchart/daily/n/${s}.gif`
    case 'week': return `https://image.sinajs.cn/newchart/weekly/n/${s}.gif`
    case 'month': return `https://image.sinajs.cn/newchart/monthly/n/${s}.gif`
    default: return `https://image.sinajs.cn/newchart/min/n/${s}.gif`
  }
}
