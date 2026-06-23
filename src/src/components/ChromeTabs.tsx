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
    <div className="flex items-stretch h-9 bg-bg-surface overflow-x-auto overflow-y-hidden shrink-0 border-b border-hairline">
      {tabs.map((tab) => {
        const isActive = tab.id === activeTab;
        return (
          <div
            key={tab.id}
            onClick={() => onSelect(tab.id)}
            className={`group flex items-center gap-1.5 px-3 text-xs cursor-pointer select-none whitespace-nowrap shrink-0 border-r border-hairline transition-colors ${
              isActive
                ? "bg-bg text-white border-b-2 border-b-ai"
                : "bg-bg-surface text-ink-muted hover:text-white hover:bg-bg-hover"
            }`}
          >
            <span className="truncate max-w-[120px]">{tab.label}</span>
            <button
              onClick={(e) => { e.stopPropagation(); onClose(tab.id); }}
              className="flex items-center justify-center w-4 h-4 rounded-sm opacity-0 group-hover:opacity-100 hover:bg-bg-hover transition-opacity"
            >
              <X className="w-3 h-3" />
            </button>
          </div>
        );
      })}
    </div>
  );
}

export type { Tab };
