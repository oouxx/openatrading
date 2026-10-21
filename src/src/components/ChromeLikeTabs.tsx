import { X } from "lucide-react"
import { Button } from "@/components/ui/button"

interface Tab {
  id: string
  label: string
}

interface ChromeTabsProps {
  tabs: Tab[]
  activeTab: string | null
  onSelect: (id: string) => void
  onClose: (id: string) => void
}

export default function ChromeTabs({ tabs, activeTab, onSelect, onClose }: ChromeTabsProps) {
  if (tabs.length === 0) return null

  return (
    <div
      role="tablist"
      aria-label="股票标签页"
      className="bg-surface border-hairline no-scrollbar flex h-9 shrink-0 items-stretch overflow-x-auto overflow-y-hidden border-b"
    >
      {tabs.map((tab) => {
        const isActive = tab.id === activeTab
        return (
          <div
            key={tab.id}
            role="presentation"
            className={`group border-hairline text-caption flex shrink-0 items-center border-r whitespace-nowrap transition-colors select-none ${
              isActive
                ? "border-b-ai bg-canvas text-primary border-b-2"
                : "bg-surface text-secondary hover:bg-hover hover:text-primary"
            }`}
          >
            <button
              type="button"
              role="tab"
              aria-selected={isActive}
              onClick={() => onSelect(tab.id)}
              className="flex min-w-0 flex-1 cursor-pointer items-center py-2 pr-1 pl-3 text-left"
            >
              <span className="max-w-[120px] truncate">{tab.label}</span>
            </button>
            <Button
              type="button"
              variant="ghost"
              size="icon-xs"
              aria-label={`关闭 ${tab.label}`}
              onClick={() => onClose(tab.id)}
              className="hover:bg-hover mr-2 size-4 rounded-sm p-0 opacity-0 group-focus-within:opacity-100 group-hover:opacity-100"
            >
              <X />
            </Button>
          </div>
        )
      })}
    </div>
  )
}

export type { Tab }
