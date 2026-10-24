import type { ReactNode } from "react"
import { AlertTriangle, Info, TriangleAlert } from "lucide-react"
import { cn } from "@/lib/utils"

/**
 * 数据状态条：一处定义「异常/降级/说明」三种强度的视觉表达。
 *
 * 用途约束（对应 CLAUDE.md 的 UI/UX 原则）：
 * - `danger`：本次操作失败，用户必须知道并可能需要重试
 * - `warning`：数据不完整/降级，用户若当成准确值会做错决策
 * - `info`：中性说明，例如缺少某个可选配置
 *
 * 不要用它写普通提示文案——能靠布局表达的就不要加这一条。
 */
const TONES = {
  danger: {
    className: "border-danger/30 bg-danger/10 text-danger",
    Icon: TriangleAlert,
  },
  warning: {
    className: "border-warning/30 bg-warning/10 text-warning",
    Icon: AlertTriangle,
  },
  info: {
    className: "border-hairline bg-surface text-secondary",
    Icon: Info,
  },
} as const

export function StatusBanner({
  tone,
  children,
  action,
  className,
}: {
  tone: keyof typeof TONES
  children: ReactNode
  /** 右侧操作（通常是「重试」），不传则无 */
  action?: ReactNode
  className?: string
}) {
  const { className: toneClass, Icon } = TONES[tone]

  return (
    <div
      className={cn(
        "text-caption flex items-center gap-2 rounded-lg border px-3 py-2",
        toneClass,
        className
      )}
      role={tone === "danger" ? "alert" : "status"}
    >
      <Icon className="size-3.5 shrink-0" />
      <span className="min-w-0 flex-1">{children}</span>
      {action ? <span className="shrink-0">{action}</span> : null}
    </div>
  )
}
