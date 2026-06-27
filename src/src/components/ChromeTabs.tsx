import { X } from "lucide-react";

interface Tab {
  id: string;
  label: string;
}

interface ChromeTabsProps {
  tabs: Tab[];
  activeTab: string | null;
  onSelect: (id: string) => void;
  onClose: (id: string) => void;
}

export default function ChromeTabs({ tabs, activeTab, onSelect, onClose }: ChromeTabsProps) {
  if (tabs.length === 0) return null;

  return (
    <div className="flex h-9 shrink-0 items-stretch overflow-x-auto overflow-y-hidden border-b border-hairline bg-bg-surface">
      {tabs.map((tab) => {
        const isActive = tab.id === activeTab;
        return (
          <div
            key={tab.id}
            onClick={() => onSelect(tab.id)}
            className={`group flex shrink-0 cursor-pointer items-center gap-1.5 border-r border-hairline px-3 text-xs whitespace-nowrap transition-colors select-none ${
              isActive
                ? "border-b-2 border-b-ai bg-bg text-white"
                : "bg-bg-surface text-ink-muted hover:bg-bg-hover hover:text-white"
            }`}
          >
            <span className="max-w-[120px] truncate">{tab.label}</span>
            <button
              onClick={(e) => { e.stopPropagation(); onClose(tab.id); }}
              className="flex size-4 items-center justify-center rounded-sm opacity-0 transition-opacity group-hover:opacity-100 hover:bg-bg-hover"
            >
              <X className="size-3" />
            </button>
          </div>
        );
      })}
    </div>
  );
}

export type { Tab };
