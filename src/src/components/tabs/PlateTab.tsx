import { useState } from "react";
import { ChevronDown } from "lucide-react";
import { useFieldItems } from "@/hooks/query/useFieldItems";
import { useRightPanelStore } from "@/stores/rightPanelStore";
import {
  Collapsible,
  CollapsibleTrigger,
  CollapsibleContent,
} from "@/components/ui/collapsible";
import type { FieldPlate } from "@/stores/data";

type StockItem = FieldPlate["list"][number];
type ActionInfo = NonNullable<NonNullable<StockItem["article"]>["action_info"]>;

/** action_info.price 是扩大 100 倍的原始值，÷100 得到真实股价 */
function formatPrice(price: number): string {
  return (price / 100).toFixed(2);
}

/** action_info.shares_range 是扩大 100 倍的换手率，÷100 得到百分比 */
function formatTurnover(range: number): string {
  return (range / 100).toFixed(2) + "%";
}

/** 判断 action_info 是否有任何可展示内容 */
function hasActionInfo(ai: ActionInfo | undefined): ai is ActionInfo {
  if (!ai) return false;
  return Boolean(
    ai.num ||
    ai.time ||
    ai.expound ||
    ai.price ||
    ai.day ||
    ai.edition ||
    ai.shares_range,
  );
}

/** 展开区目前只放 expound；无 expound 时不显示展开按钮 */
function hasExpandableDetail(ai: ActionInfo | undefined): ai is ActionInfo {
  return Boolean(ai?.expound);
}

export default function PlateTab() {
  const { data: fieldItems = [] } = useFieldItems();
  const openKline = useRightPanelStore((s) => s.openKline);

  return (
    <div className="space-y-3">
      {fieldItems.map((plate: FieldPlate) => (
        <PlateCard key={plate.name} plate={plate} onOpenKline={openKline} />
      ))}
    </div>
  );
}

function PlateCard({
  plate,
  onOpenKline,
}: {
  plate: FieldPlate;
  onOpenKline: (s: string, n: string) => void;
}) {
  const [expanded, setExpanded] = useState<string | null>(null);
  const isOpen = expanded === plate.name;

  return (
    <div className="bg-bg-surface rounded-card border border-hairline overflow-hidden">
      <div
        className="px-4 py-3 flex items-center justify-between cursor-pointer hover:bg-bg-hover transition-colors"
        onClick={() => setExpanded(isOpen ? null : plate.name)}
      >
        <div className="flex items-center gap-3">
          <span className="text-white text-sm font-semibold">{plate.name}</span>
          <span className="text-caption text-ink-muted">{plate.count}只</span>
        </div>
        <span className="text-caption text-ink-muted">
          {isOpen ? "▲" : "▼"}
        </span>
      </div>
      {plate.reason && (
        <div className="px-4 pb-2">
          <p className="text-caption text-ink-muted leading-relaxed">
            {plate.reason}
          </p>
        </div>
      )}
      {isOpen && plate.list.length > 0 && (
        <div className="border-t border-hairline">
          <div className="p-3 grid grid-cols-1 md:grid-cols-2 gap-2">
            {plate.list.map((item) => (
              <StockCard
                key={item.code}
                item={item}
                onOpenKline={onOpenKline}
              />
            ))}
          </div>
        </div>
      )}
    </div>
  );
}

function StockCard({
  item,
  onOpenKline,
}: {
  item: FieldPlate["list"][number];
  onOpenKline: (s: string, n: string) => void;
}) {
  const ai = hasActionInfo(item.article?.action_info)
    ? item.article!.action_info
    : undefined;
  const expandable = hasExpandableDetail(ai);

  return (
    <Collapsible className="group">
      <div
        className="bg-bg rounded-card border border-hairline cursor-pointer hover:border-ai/30 transition-colors"
        onClick={() => onOpenKline(item.code, item.name)}
      >
        <div className="flex items-center justify-between p-2">
          <div className="flex items-baseline gap-2 min-w-0">
            <span className="text-white text-xs font-semibold truncate">
              {item.name}
            </span>
            <span className="text-caption text-ink-muted font-mono">
              {item.code.replace(/^(sh|sz|bj)/, "")}
            </span>
          </div>
          <div className="flex items-center gap-1 flex-shrink-0">
            {ai?.num && (
              <span className="text-caption text-ai font-semibold">
                {ai.num}
              </span>
            )}
            {expandable && (
              <CollapsibleTrigger asChild>
                <button
                  className="text-caption text-ink-muted hover:text-ai transition-colors"
                  title="异动详情"
                  onClick={(e) => e.stopPropagation()}
                >
                  <ChevronDown className="w-3 h-3 group-data-[state=open]:rotate-180 transition-transform" />
                </button>
              </CollapsibleTrigger>
            )}
          </div>
        </div>
        {ai &&
          (ai.time !== undefined ||
            ai.price !== undefined ||
            ai.shares_range !== undefined) && (
            <div className="px-2 pb-2 -mt-1 flex items-center gap-2 text-caption font-mono flex-wrap">
              {ai.time && <span className="text-ink-muted">{ai.time}</span>}
              {ai.price !== undefined && (
                <span className="text-ink-muted">
                  价格{" "}
                  <span className="text-white">{formatPrice(ai.price)}</span>
                </span>
              )}
              {ai.shares_range !== undefined && (
                <span className="text-ink-muted">
                  涨幅{" "}
                  <span className="text-up">
                    {formatTurnover(ai.shares_range)}
                  </span>
                </span>
              )}
            </div>
          )}
      </div>
      {expandable && (
        <CollapsibleContent>
          <div className="px-2 py-2 bg-bg-hover/30 border-t border-hairline rounded-b-card">
            <p className="text-caption text-ink-muted leading-relaxed whitespace-pre-line">
              {ai!.expound}
            </p>
          </div>
        </CollapsibleContent>
      )}
    </Collapsible>
  );
}
