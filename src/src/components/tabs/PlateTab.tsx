import { useState } from "react";
import { useFieldItems } from "@/hooks/query/useFieldItems";
import { useRightPanelStore } from "@/stores/rightPanelStore";
import type { FieldPlate } from "@/stores/data";

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
          <div className="p-3 grid grid-cols-2 md:grid-cols-3 gap-2">
            {plate.list.map((item) => (
              <div
                key={item.code}
                className="bg-bg rounded-card border border-hairline p-2 cursor-pointer hover:border-ai/30 transition-colors"
                onClick={() => onOpenKline(item.code, item.name)}
              >
                <span className="text-white text-xs font-semibold">
                  {item.name}
                </span>
                <span className="text-caption text-ink-muted font-mono ml-2">
                  {item.code.replace(/^(sh|sz|bj)/, "")}
                </span>
              </div>
            ))}
          </div>
        </div>
      )}
    </div>
  );
}
