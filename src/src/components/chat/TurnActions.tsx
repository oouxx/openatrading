import { useCallback, useState } from "react"
import { Check, Copy } from "lucide-react"
import { Button } from "@/components/ui/button"

/**
 * 一条已完成的助手回答的尾部动作：把整段回答原文复制走。
 *
 * 悬停才出现（桌面端），触摸设备上常驻——回答经常要被粘到别处，但常驻的
 * 按钮会在每一轮对话尾部堆一列重复控件。
 */
export function TurnActions({ text }: { text: string }) {
  const [copied, setCopied] = useState(false)

  const copy = useCallback(() => {
    navigator.clipboard.writeText(text).then(() => {
      setCopied(true)
      setTimeout(() => setCopied(false), 2000)
    })
  }, [text])

  if (!text.trim()) return null

  return (
    // 只有能 hover 的设备才隐藏（触摸设备没有 hover，隐藏等于永远看不见）。
    <div className="can-hover:opacity-0 can-hover:group-hover/turn:opacity-100 can-hover:focus-within:opacity-100 mt-1 flex items-center gap-1 transition-opacity">
      <Button
        type="button"
        variant="ghost"
        size="xs"
        onClick={copy}
        className="text-tertiary hover:text-secondary hover:bg-hover"
        aria-label={copied ? "已复制回答" : "复制回答"}
      >
        {copied ? <Check className="text-success" /> : <Copy />}
        <span className={copied ? "text-success" : undefined}>{copied ? "已复制" : "复制"}</span>
      </Button>
    </div>
  )
}
