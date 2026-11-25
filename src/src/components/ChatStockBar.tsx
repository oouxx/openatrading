import { X } from "lucide-react"
import { Button } from "@/components/ui/button"
import { useChatStockStore } from "@/stores/chatStockStore"

/**
 * 对话格顶部的股票标签条：被派发过来的票在这里留成标签，点标签切到它的 K 线、
 * × 关掉（关完最后一只就回到对话）。
 *
 * **没有搜索框**：搜股票归「个股详情」格和各榜单，对话格不再重复一个入口；没有标签
 * 时整条不渲染，对话保持无表头的沉浸布局。
 *
 * 这里的 K 线是"临时看一眼"（标签存 localStorage、易失），要长期钉住一只票用
 * 「个股详情」格。
 */
export function ChatStockBar() {
  const tabs = useChatStockStore((s) => s.tabs)
  const activeCode = useChatStockStore((s) => s.activeCode)
  const activate = useChatStockStore((s) => s.activate)
  const close = useChatStockStore((s) => s.close)

  // 没有标签时整条不渲染：否则对话格顶部白留 36px
  if (tabs.length === 0) return null

  return (
    <header className="border-hairline flex h-9 shrink-0 items-center gap-1 overflow-x-auto border-b px-2">
      {tabs.map((tab) => {
        const active = tab.code === activeCode
        return (
          <div
            key={tab.code}
            className={`flex shrink-0 items-center rounded-md ${active ? "bg-brand/10" : ""}`}
          >
            <Button
              type="button"
              variant="ghost"
              size="xs"
              className={`text-caption rounded-r-none ${
                active
                  ? "text-brand hover:bg-brand/10 hover:text-brand"
                  : "text-secondary hover:bg-hover"
              }`}
              title={`${tab.name} ${tab.code}`}
              aria-pressed={active}
              onClick={() => activate(tab.code)}
            >
              {tab.name}
            </Button>
            <Button
              type="button"
              variant="ghost"
              size="icon-xs"
              aria-label={`关闭 ${tab.name}`}
              className="text-tertiary hover:bg-hover hover:text-primary rounded-l-none"
              onClick={() => close(tab.code)}
            >
              <X className="size-3" />
            </Button>
          </div>
        )
      })}
    </header>
  )
}
