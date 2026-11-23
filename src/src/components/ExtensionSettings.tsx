import { useState } from "react"
import { Input } from "@/components/ui/input"
import { SettingRow, SelectField } from "./settings-ui"
import { extensionErrorMessage, type ExtensionSetting } from "@/hooks/query/useExtensions"

/**
 * 扩展设置在卡片内的内嵌面板：每个 `settings.json` 键一行。
 *
 * 行由后端声明（label / hint / placeholder / options），前端不按扩展 id 分支；
 * pi-rs 以后给某个扩展加配置项，只需在后端补一条 descriptor。
 */
export function ExtensionSettings({
  settings,
  note,
  onSave,
}: {
  settings: ExtensionSetting[]
  note: string | null
  onSave: (key: string, value: string) => Promise<unknown>
}) {
  return (
    <div className="border-hairline border-t px-4 py-1">
      <div className="divide-hairline divide-y">
        {settings.map((setting) => (
          <ExtensionSettingRow key={setting.key} setting={setting} onSave={onSave} />
        ))}
      </div>
      {note && <p className="text-tertiary text-caption py-3 leading-5">{note}</p>}
    </div>
  )
}

function ExtensionSettingRow({
  setting,
  onSave,
}: {
  setting: ExtensionSetting
  onSave: (key: string, value: string) => Promise<unknown>
}) {
  const [value, setValue] = useState(setting.value)
  const [busy, setBusy] = useState(false)
  const [error, setError] = useState<string | null>(null)

  const commit = (next: string) => {
    if (next === setting.value) return
    setBusy(true)
    setError(null)
    onSave(setting.key, next)
      .catch((e: unknown) => {
        setError(extensionErrorMessage(e instanceof Error ? e.message : "保存失败"))
        // 失败后回到服务端值：页面不应停在一个没落盘的设置上。
        setValue(setting.value)
      })
      .finally(() => setBusy(false))
  }

  const field =
    setting.options.length > 0 ? (
      <SelectField
        // 未配置时按 pi-rs 默认档展示（第一个候选值），选中它不会写文件。
        value={setting.value || setting.options[0]?.value || ""}
        disabled={busy}
        options={setting.options}
        onChange={(next) => {
          setValue(next)
          commit(next)
        }}
        className="w-[168px]"
      />
    ) : (
      <Input
        value={value}
        disabled={busy}
        placeholder={setting.placeholder ?? ""}
        className="text-caption h-8 w-[240px]"
        onChange={(e) => setValue(e.target.value)}
        onBlur={() => commit(value.trim())}
        onKeyDown={(e) => {
          if (e.key === "Enter") e.currentTarget.blur()
        }}
        aria-label={setting.label}
      />
    )

  return (
    <SettingRow
      label={setting.label}
      hint={setting.hint ?? undefined}
      control={
        <div className="flex flex-col items-end gap-1">
          {field}
          {error && <p className="text-danger text-caption max-w-[240px] text-right">{error}</p>}
        </div>
      }
    />
  )
}
