import type { ReactNode } from "react"
import { ChevronDownIcon } from "lucide-react"
import { cn } from "src/lib/utils"

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
    <div className="relative">
      <select
        value={value}
        disabled={disabled}
        onChange={(e) => onChange(e.target.value)}
        className={cn(
          "border-input bg-surface hover:bg-hover text-primary focus:border-ring h-8 w-[200px] cursor-pointer appearance-none rounded-lg border px-2.5 pr-7 text-xs transition-colors focus:outline-none disabled:cursor-not-allowed disabled:opacity-50",
          className
        )}
      >
        {options.map((o) => (
          <option key={o.value} value={o.value}>
            {o.label}
          </option>
        ))}
      </select>
      <ChevronDownIcon className="text-secondary pointer-events-none absolute top-1/2 right-2.5 size-3.5 -translate-y-1/2" />
    </div>
  )
}
