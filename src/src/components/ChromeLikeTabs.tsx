import { X } from "lucide-react"

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
    <div className="bg-surface border-hairline flex h-9 shrink-0 items-stretch overflow-x-auto overflow-y-hidden border-b">
      {tabs.map((tab) => {
        const isActive = tab.id === activeTab
        return (
          <div
            key={tab.id}
            onClick={() => onSelect(tab.id)}
            className={`group border-hairline flex shrink-0 cursor-pointer items-center gap-2 border-r px-3 text-caption whitespace-nowrap transition-colors select-none ${
              isActive
                ? "border-b-ai bg-base border-b-2 text-primary"
                : "bg-surface text-secondary hover:bg-hover hover:text-primary"
            }`}
          >
            <span className="max-w-[120px] truncate">{tab.label}</span>
            <button
              onClick={(e) => {
                e.stopPropagation()
                onClose(tab.id)
              }}
              className="hover:bg-hover flex size-4 items-center justify-center rounded-sm opacity-0 transition-opacity group-hover:opacity-100"
            >
              <X className="size-3" />
            </button>
          </div>
        )
      })}
    </div>
  )
}

export type { Tab }
