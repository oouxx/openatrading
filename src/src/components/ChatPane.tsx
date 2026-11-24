import { useEffect, useRef, type ReactNode } from "react"
import KlineDrawer from "@/components/KlineDrawer"
import { ChatStockBar } from "@/components/ChatStockBar"
import { useChatStockStore } from "@/stores/chatStockStore"

/**
 * 对话格的外壳：顶部股票条 + 对话正文（需要时盖一层 K 线）。
 *
 * K 线用覆盖层而不是替换正文：`AiCopilot` 卸载会 dispose 掉 WebSocket / Tauri
 * 传输层，回来时流式中的回合就断了。覆盖层和行情格里的 K 线子页面是同一个套路。
 */
export default function ChatPane({
  activeThreadId,
  children,
}: {
  activeThreadId: string | null
  children: ReactNode
}) {
  const tabs = useChatStockStore((s) => s.tabs)
  const activeCode = useChatStockStore((s) => s.activeCode)
  const activate = useChatStockStore((s) => s.activate)
  const close = useChatStockStore((s) => s.close)

  const activeTab = tabs.find((tab) => tab.code === activeCode)

  // 切换会话时收起 K 线：用户点的是"看这个会话"，标签保留，一下就能再点回来。
  const prevThread = useRef(activeThreadId)
  useEffect(() => {
    if (prevThread.current === activeThreadId) return
    prevThread.current = activeThreadId
    activate(null)
  }, [activeThreadId, activate])

  return (
    <div className="flex h-full min-h-0 flex-col">
      <ChatStockBar />
      <div className="relative min-h-0 flex-1 overflow-hidden">
        {children}
        {activeTab && (
          <div className="bg-canvas absolute inset-0 z-20 overflow-y-auto">
            <KlineDrawer
              symbol={activeTab.code}
              name={activeTab.name}
              onClose={() => close(activeTab.code)}
            />
          </div>
        )}
      </div>
    </div>
  )
}
