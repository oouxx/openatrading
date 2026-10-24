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
 * Codex 风格设置行：标题 + 可选提示 在左，控件 在右。
 * 行高与 Codex 设置页一致（min-h-[52px]，py-3），行与行之间由
 * 外层 SettingsSection 的 divide-y hairline 分割。
 *
 * `hint` 只写 label 表达不了的东西：前置条件、副作用、风险。
 * 禁止写 label 的同义词或翻译（`Bot Token` + “机器人令牌”）——
 * 那只会把每行变成两行字，层级交给分组和控件自己解释。
 */
export function SettingRow({
  label,
  hint,
  control,
}: {
  label: string
  hint?: string
  control: ReactNode
}) {
  return (
    <div className="flex min-h-[52px] items-center justify-between gap-8 py-3">
      <div className="min-w-0">
        <p className="text-primary text-body leading-5 font-medium">{label}</p>
        {hint ? <p className="text-body text-secondary mt-0.5 leading-4">{hint}</p> : null}
      </div>
      <div className="shrink-0">{control}</div>
    </div>
  )
}

/** 设置分组：白底卡片 + 小标题 + 行列表（hairline 分割） */
export function SettingsSection({ title, children }: { title: string; children: ReactNode }) {
  return (
    <section className="bg-surface border-hairline rounded-lg border px-5 py-4">
      <h2 className="text-primary text-body mb-1.5 font-semibold">{title}</h2>
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
        className={cn(
          "bg-surface hover:bg-hover text-primary text-caption h-8 w-[200px]",
          className
        )}
      >
        <SelectValue />
      </SelectTrigger>
      <SelectContent>
        {options.map((o) => (
          <SelectItem key={o.value} value={o.value} className="text-caption">
            {o.label}
          </SelectItem>
        ))}
      </SelectContent>
    </Select>
  )
}
