/** snake_case / kebab-case → Title Case. `get_kline` → `Get Kline`. */
function toTitleCase(s: string): string {
  return s
    .replace(/[-_]+/g, " ")
    .replace(/\b\w/g, (c) => c.toUpperCase())
    .replace(/\s+/g, " ")
    .trim()
}

/** Parse the serialized tool `args` string into an object (best-effort). */
function parseArgs(args?: string): Record<string, unknown> | null {
  if (!args) return null
  try {
    const p = JSON.parse(args)
    return p && typeof p === "object" && !Array.isArray(p) ? (p as Record<string, unknown>) : null
  } catch {
    return null
  }
}

/** First non-empty string value among candidate arg keys. */
function argString(a: Record<string, unknown> | null, ...keys: string[]): string {
  if (!a) return ""
  for (const k of keys) {
    const v = a[k]
    if (typeof v === "string" && v.trim()) return v
  }
  return ""
}

/**
 * Generic tool-summary generator (Goose-style). One card serves every tool:
 * a few well-known tools get friendly Chinese phrases derived from their args;
 * everything else falls back to a readable name + the argument keys, so an
 * unknown / MCP tool never requires a bespoke card.
 */
export function summarizeTool(name: string, args?: string): string {
  const a = parseArgs(args)
  const p = (...keys: string[]) => argString(a, ...keys)

  if (/^(read|read_file)$/.test(name))
    return p("path", "uri", "file_path") ? `查看 ${p("path", "uri", "file_path")}` : "查看文件"
  if (/^(write|write_file|create_file)$/.test(name))
    return p("path", "file_path") ? `写入 ${p("path", "file_path")}` : "写入文件"
  if (/^(edit|edit_file|file_edit)$/.test(name))
    return p("path", "file_path") ? `编辑 ${p("path", "file_path")}` : "编辑文件"
  if (/^(bash|shell|execute_command|run)$/.test(name))
    return p("command", "cmd") ? `运行 ${p("command", "cmd")}` : "运行命令"
  if (name === "glob") return p("pattern") ? `查找 ${p("pattern")}` : "查找文件"
  if (name === "grep") return p("pattern") ? `搜索 ${p("pattern")}` : "搜索代码"
  if (/^(web_search|search)$/.test(name))
    return p("query", "q") ? `搜索 ${p("query", "q")}` : "网页搜索"
  if (/^(web_fetch|fetch)$/.test(name)) return p("url") ? `抓取 ${p("url")}` : "抓取网页"

  // Generic fallback: readable name + a single concise arg value, or the arg keys.
  const readable = toTitleCase(name)
  if (a) {
    const keys = Object.keys(a)
    if (keys.length === 1) {
      const val = a[keys[0]]
      const short = typeof val === "string" ? val : JSON.stringify(val)
      return short && short !== "{}" ? `${readable} · ${short}` : readable
    }
    if (keys.length > 1) return `${readable} · ${keys.join(", ")}`
  }
  return readable
}
