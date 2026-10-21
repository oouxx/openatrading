import { useState } from "react"
import { Button } from "@/components/ui/button"
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
  const { data: fieldItems = [], isLoading, error } = useFieldItems()
  const openKline = useRightPanelStore((s) => s.openKline)

  // 上游会返回「简图」这类 count=0、list=[] 的占位分组，展开后是空卡片，
  // 没有展示意义。
  const plates = fieldItems.filter((plate) => plate.count > 0 || plate.list.length > 0)

  if (isLoading) {
    return <div className="text-caption text-secondary py-12 text-center">加载板块异动...</div>
  }

  if (error) {
    return (
      <div className="bg-surface border-hairline text-caption text-secondary rounded-lg border px-4 py-3">
        加载失败：{error.message}
      </div>
    )
  }

  if (plates.length === 0) {
    return <div className="text-caption text-secondary py-12 text-center">暂无板块异动数据</div>
  }

  return (
    <div className="space-y-3">
      {plates.map((plate: FieldPlate) => (
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
    <div className="bg-surface border-hairline overflow-hidden rounded-lg border">
      <Button
        type="button"
        variant="ghost"
        aria-expanded={isOpen}
        onClick={() => setExpanded(isOpen ? null : plate.name)}
        className="hover:bg-hover h-auto w-full justify-between rounded-none px-4 py-3"
      >
        <span className="flex items-center gap-3">
          <span className="text-body text-primary font-semibold">{plate.name}</span>
          <span className="text-caption text-secondary">{plate.count}只</span>
        </span>
        <span className="text-caption text-secondary">{isOpen ? "▲" : "▼"}</span>
      </Button>
      {plate.reason && (
        <div className="px-4 pb-2">
          <p className="text-caption text-secondary leading-relaxed">{plate.reason}</p>
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
    <button
      type="button"
      className="bg-canvas border-hairline hover:border-brand/30 w-full cursor-pointer rounded-lg border text-left transition-colors"
      onClick={() => onOpenKline(item.code, item.name)}
    >
      <div className="flex items-center justify-between p-2">
        <div className="flex min-w-0 items-baseline gap-2">
          <span className="text-caption text-primary truncate font-semibold">{item.name}</span>
          <span className="text-caption text-secondary font-number">
            {item.code.replace(/^(sh|sz|bj)/, "")}
          </span>
        </div>
        <div className="flex shrink-0 items-center gap-1">
          {ai?.num && <span className="text-brand text-caption font-semibold">{ai.num}</span>}
        </div>
      </div>
      {ai && (ai.time !== undefined || ai.price !== undefined || ai.shares_range !== undefined) && (
        <div className="text-caption font-number -mt-1 flex flex-wrap items-center gap-2 px-2 pb-2">
          {ai.time && <span className="text-secondary">{ai.time}</span>}
          {ai.price !== undefined && (
            <span className="text-secondary">
              价格 <span className="text-primary">{formatPrice(ai.price)}</span>
            </span>
          )}
          {ai.shares_range !== undefined && (
            <span className="text-secondary">
              涨幅 <span className="text-up">{formatTurnover(ai.shares_range)}</span>
            </span>
          )}
        </div>
      )}
    </button>
  )
}
