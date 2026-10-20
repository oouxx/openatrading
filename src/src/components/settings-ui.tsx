import type { ReactNode } from "react"
import { cn } from "@/lib/utils"
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from "@/components/ui/select"

/**
 * Codex 风格设置行：标题 + 描述 在左，控件 在右。
 * 行高与 Codex 设置页一致（min-h-[52px]，py-3），行与行之间由
 * 外层 SettingsSection 的 divide-y hairline 分割。
 */
export function SettingRow({
  label,
  description,
  control,
}: {
  label: string
  description?: string
  control: ReactNode
}) {
  return (
    <div className="flex min-h-[52px] items-center justify-between gap-8 py-3">
      <div className="min-w-0">
        <p className="text-primary text-sm leading-5 font-medium">{label}</p>
        {description ? (
          <p className="text-secondary text-caption mt-0.5 leading-4">{description}</p>
        ) : null}
      </div>
      <div className="shrink-0">{control}</div>
    </div>
  )
}

/** Codex 风格分组：小标题（sentence-case 13px semibold）+ 行列表（hairline 分割，无卡片底） */
export function SettingsSection({ title, children }: { title: string; children: ReactNode }) {
  return (
    <section>
      <h2 className="text-primary mb-3 text-[13px] font-semibold">{title}</h2>
      <div className="divide-hairline divide-y">{children}</div>
    </section>
  )
}

/** Codex 风格下拉：h-8、圆角边框、右侧 chevron，宽度默认 200px */
export function SelectField({
  value,
  onChange,
  options,
  className,
  disabled,
}: {
  value: string
  onChange: (v: string) => void
  options: { value: string; label: string }[]
  className?: string
  disabled?: boolean
}) {
  return (
    <Select value={value} onValueChange={onChange} disabled={disabled}>
      <SelectTrigger
        className={cn("bg-surface hover:bg-hover text-primary h-8 w-[200px] text-xs", className)}
      >
        <SelectValue />
      </SelectTrigger>
      <SelectContent>
        {options.map((o) => (
          <SelectItem key={o.value} value={o.value} className="text-xs">
            {o.label}
          </SelectItem>
        ))}
      </SelectContent>
    </Select>
  )
}
