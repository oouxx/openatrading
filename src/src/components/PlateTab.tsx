import { useState } from "react"
import { useFieldItems } from "@/hooks/query/useFieldItems"
import { useRightPanelStore } from "@/stores/rightPanelStore"
import type { FieldPlate } from "@/stores/data"

type StockItem = FieldPlate["list"][number]
type ActionInfo = NonNullable<NonNullable<StockItem["article"]>["action_info"]>

/** action_info.price 是扩大 100 倍的原始值，÷100 得到真实股价 */
function formatPrice(price: number): string {
  return (price / 100).toFixed(2)
}

/** action_info.shares_range 是扩大 100 倍的换手率，÷100 得到百分比 */
function formatTurnover(range: number): string {
  return (range / 100).toFixed(2) + "%"
}

/** 判断 action_info 是否有任何可展示内容 */
function hasActionInfo(ai: ActionInfo | undefined): ai is ActionInfo {
  if (!ai) return false
  return Boolean(ai.num || ai.time || ai.price || ai.day || ai.edition || ai.shares_range)
}

export default function PlateTab() {
  const { data: fieldItems = [] } = useFieldItems()
  const openKline = useRightPanelStore((s) => s.openKline)

  return (
    <div className="space-y-3">
      {fieldItems.map((plate: FieldPlate) => (
        <PlateCard key={plate.name} plate={plate} onOpenKline={openKline} />
      ))}
    </div>
  )
}

function PlateCard({
  plate,
  onOpenKline,
}: {
  plate: FieldPlate
  onOpenKline: (s: string, n: string) => void
}) {
  const [expanded, setExpanded] = useState<string | null>(null)
  const isOpen = expanded === plate.name

  return (
    <div className="bg-bg-surface border-hairline rounded-lg overflow-hidden border">
      <div
        className="hover:bg-bg-hover flex cursor-pointer items-center justify-between px-4 py-3 transition-colors"
        onClick={() => setExpanded(isOpen ? null : plate.name)}
      >
        <div className="flex items-center gap-3">
          <span className="text-sm font-semibold text-foreground">{plate.name}</span>
          <span className="text-caption text-ink-muted">{plate.count}只</span>
        </div>
        <span className="text-caption text-ink-muted">{isOpen ? "▲" : "▼"}</span>
      </div>
      {plate.reason && (
        <div className="px-4 pb-2">
          <p className="text-caption text-ink-muted leading-relaxed">{plate.reason}</p>
        </div>
      )}
      {isOpen && plate.list.length > 0 && (
        <div className="border-hairline border-t">
          <div className="grid grid-cols-1 gap-2 p-3 md:grid-cols-2">
            {plate.list.map((item) => (
              <StockCard key={item.code} item={item} onOpenKline={onOpenKline} />
            ))}
          </div>
        </div>
      )}
    </div>
  )
}

function StockCard({
  item,
  onOpenKline,
}: {
  item: FieldPlate["list"][number]
  onOpenKline: (s: string, n: string) => void
}) {
  const ai = hasActionInfo(item.article?.action_info) ? item.article!.action_info : undefined

  return (
    <div
      className="bg-bg border-hairline hover:border-ai/30 rounded-lg cursor-pointer border transition-colors"
      onClick={() => onOpenKline(item.code, item.name)}
    >
      <div className="flex items-center justify-between p-2">
        <div className="flex min-w-0 items-baseline gap-2">
          <span className="truncate text-xs font-semibold text-foreground">{item.name}</span>
          <span className="text-caption text-ink-muted font-mono">
            {item.code.replace(/^(sh|sz|bj)/, "")}
          </span>
        </div>
        <div className="flex shrink-0 items-center gap-1">
          {ai?.num && <span className="text-ai text-caption font-semibold">{ai.num}</span>}
        </div>
      </div>
      {ai && (ai.time !== undefined || ai.price !== undefined || ai.shares_range !== undefined) && (
        <div className="text-caption -mt-1 flex flex-wrap items-center gap-2 px-2 pb-2 font-mono">
          {ai.time && <span className="text-ink-muted">{ai.time}</span>}
          {ai.price !== undefined && (
            <span className="text-ink-muted">
              价格 <span className="text-foreground">{formatPrice(ai.price)}</span>
            </span>
          )}
          {ai.shares_range !== undefined && (
            <span className="text-ink-muted">
              涨幅 <span className="text-up">{formatTurnover(ai.shares_range)}</span>
            </span>
          )}
        </div>
      )}
    </div>
  )
}
