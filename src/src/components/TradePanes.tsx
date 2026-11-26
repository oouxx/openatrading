import { useState } from "react"
import { ActiveStockHeader } from "@/components/ActiveStockHeader"
import { OrderBookPanel } from "@/components/OrderBookPanel"
import { BuyTicketForm, SellTicketForm } from "@/components/TradeTicket"
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from "@/components/ui/select"
import { useRoleList } from "@/hooks/query/useRoleQueries"
import { usePaneStock } from "@/hooks/usePaneStock"
import { usePositionOf } from "@/hooks/usePositionOf"
import { classifySymbol } from "@/lib/stock-utils"
import { extractStockCode } from "@/lib/trade"

/**
 * 「五档行情」「下单」两个网格格子。
 *
 * 标的来自 `usePaneStock`：默认跟随工作区当前标的，右上角的开关可以解绑固定在
 * 某一只上。所以"在榜单里点一只票 → 这两格跟着换"是零配置的，不需要先建立联动。
 */

export function OrderBookPane({
  layoutId,
  leafId,
  symbol,
  symbolName,
}: {
  layoutId: string
  leafId: string
  symbol?: string
  symbolName?: string
}) {
  const { stock, select } = usePaneStock({ layoutId, leafId, symbol, symbolName })
  // 指数没有盘口：不能把裸代码丢给接口——`000001` 会被查成平安银行
  const isIndex = stock != null && classifySymbol(stock.code) === "index"
  const code = stock && !isIndex ? (extractStockCode(stock.code) ?? stock.code) : ""

  return (
    <div className="bg-surface flex h-full min-w-0 flex-col overflow-hidden">
      <ActiveStockHeader stock={stock} onPick={select} />
      <div className="min-h-0 flex-1 overflow-y-auto p-2">
        <OrderBookPanel
          symbol={code}
          name={stock?.name}
          placeholder={isIndex ? "指数没有盘口" : "在自选/榜单里点一只股票，或从右上角搜索"}
          className="border-0"
        />
      </div>
    </div>
  )
}

export function TradePane({
  layoutId,
  leafId,
  symbol,
  symbolName,
}: {
  layoutId: string
  leafId: string
  symbol?: string
  symbolName?: string
}) {
  const { stock, select } = usePaneStock({ layoutId, leafId, symbol, symbolName })
  const { data: roles = [] } = useRoleList()
  const [roleName, setRoleName] = useState<string | null>(null)
  const [side, setSide] = useState<"buy" | "sell">("buy")

  const activeRole = roleName ?? roles[0]?.name ?? null
  const position = usePositionOf(activeRole ?? "", stock?.code ?? "")

  return (
    <div className="bg-surface flex h-full min-w-0 flex-col overflow-hidden">
      <ActiveStockHeader stock={stock} onPick={select} />
      <div className="border-hairline flex h-9 shrink-0 items-center gap-2 border-b px-2">
        <div className="bg-hover border-hairline inline-flex rounded-md border p-0.5">
          {(["buy", "sell"] as const).map((value) => (
            <button
              key={value}
              type="button"
              aria-pressed={side === value}
              onClick={() => setSide(value)}
              className={`text-caption h-6 rounded-sm px-3 transition-colors ${
                side === value
                  ? "bg-surface text-primary shadow-card"
                  : "text-tertiary hover:text-secondary"
              }`}
            >
              {value === "buy" ? "买入" : "卖出"}
            </button>
          ))}
        </div>
        {roles.length > 1 && (
          <Select value={activeRole ?? undefined} onValueChange={(next) => setRoleName(next)}>
            <SelectTrigger
              size="sm"
              aria-label="记入哪个角色"
              className="text-caption text-secondary h-7 w-[120px]"
            >
              <SelectValue placeholder="选择角色" />
            </SelectTrigger>
            <SelectContent>
              {roles.map((role) => (
                <SelectItem key={role.name} value={role.name}>
                  {role.display_name || role.name}
                </SelectItem>
              ))}
            </SelectContent>
          </Select>
        )}
      </div>

      <div className="min-h-0 flex-1 overflow-y-auto p-3">
        {!activeRole ? (
          <p className="text-tertiary text-caption">先在「模拟盘」里建一个角色。</p>
        ) : !stock ? (
          <p className="text-tertiary text-caption">在自选/榜单里点一只股票，或从右上角搜索。</p>
        ) : classifySymbol(stock.code) === "index" ? (
          <p className="text-tertiary text-caption">指数不可交易。</p>
        ) : side === "buy" ? (
          // 换股票就重挂载：价格、股数、备注都该跟着清空
          <BuyTicketForm key={stock.code} roleName={activeRole} preset={stock} />
        ) : position ? (
          // 可卖股数变了（刚买过/刚卖过）就重挂载，股数框跟着回到新的可卖数量
          <SellTicketForm
            key={`${position.symbol}:${position.available_shares}`}
            roleName={activeRole}
            position={position}
          />
        ) : (
          <p className="text-tertiary text-caption">
            「{roles.find((r) => r.name === activeRole)?.display_name ?? activeRole}
            」当前没有 {stock.name} 的持仓。
          </p>
        )}
      </div>
    </div>
  )
}
