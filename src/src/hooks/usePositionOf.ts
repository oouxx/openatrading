import { useRolePositions } from "@/hooks/query/useRoleQueries"
import type { RolePosition } from "@/stores/data"

/** 代码归一：`600519.SS` / `sh600519` → `600519`，用来对齐持仓与当前标的。 */
function plainCode(code: string): string {
  return code.replace(/^[a-z]+/i, "").replace(/\..*$/, "")
}

/** 当前角色在某只股票上的持仓；没有持仓时为 null。 */
export function usePositionOf(roleName: string, symbol: string): RolePosition | null {
  const { data } = useRolePositions(roleName)
  const key = plainCode(symbol)
  if (!key) return null
  return (data ?? []).find((p) => plainCode(p.symbol) === key) ?? null
}
