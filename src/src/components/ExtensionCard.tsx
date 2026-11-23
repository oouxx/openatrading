import { useState } from "react"
import { Globe, SlidersHorizontal, Terminal, Users } from "lucide-react"
import { Button } from "@/components/ui/button"
import { Switch } from "@/components/ui/switch"
import { ExtensionSettings } from "./ExtensionSettings"
import { extensionErrorMessage, type BuiltinExtension } from "@/hooks/query/useExtensions"

/** 内置扩展的固定图标（按 pi-rs 扩展 id 映射）。 */
const EXTENSION_ICON = {
  subagent: Users,
  web_search: Globe,
} as const

export function ExtensionCard({
  extension: ext,
  onToggle,
  onSaveSetting,
  disabled,
}: {
  extension: BuiltinExtension
  onToggle: (enabled: boolean) => Promise<unknown>
  onSaveSetting: (key: string, value: string) => Promise<unknown>
  disabled?: boolean
}) {
  const [busy, setBusy] = useState(false)
  const [error, setError] = useState<string | null>(null)
  const [settingsOpen, setSettingsOpen] = useState(false)

  const Icon = EXTENSION_ICON[ext.id as keyof typeof EXTENSION_ICON] ?? Terminal
  const hasSettings = ext.settings.length > 0

  return (
    <div className="bg-surface border-hairline rounded-lg border">
      <div className="flex items-start gap-3 p-4">
        <span
          className={`bg-hover grid size-9 shrink-0 place-items-center rounded-lg ${
            ext.enabled ? "text-secondary" : "text-tertiary"
          }`}
        >
          <Icon className="size-4" />
        </span>

        <div className="min-w-0 flex-1">
          <div className="flex flex-wrap items-center gap-1.5">
            <span
              className={`text-body font-medium ${ext.enabled ? "text-primary" : "text-secondary"}`}
            >
              {ext.name}
            </span>
            {ext.commands.length > 0 && (
              <span className="text-tertiary border-hairline font-number text-caption rounded-md border px-1.5 py-0.5 leading-none">
                /{ext.commands.join(" /")}
              </span>
            )}
          </div>
          <p
            className={`text-caption mt-1 leading-5 ${
              ext.enabled ? "text-secondary" : "text-tertiary"
            }`}
          >
            {ext.description}
          </p>
          {error && <p className="text-danger text-caption mt-1">{error}</p>}
        </div>

        <div className="flex shrink-0 items-center gap-0.5">
          {hasSettings && (
            <Button
              type="button"
              variant="ghost"
              size="icon-sm"
              className={settingsOpen ? "text-primary" : "text-secondary hover:text-primary"}
              aria-expanded={settingsOpen}
              aria-label={`${settingsOpen ? "收起" : "展开"} ${ext.name} 设置`}
              title={settingsOpen ? "收起设置" : "设置"}
              onClick={() => setSettingsOpen((open) => !open)}
            >
              <SlidersHorizontal className="size-3.5" />
            </Button>
          )}
          <Switch
            checked={ext.enabled}
            disabled={disabled || busy}
            onCheckedChange={(next) => {
              setBusy(true)
              setError(null)
              onToggle(next)
                .catch((e: unknown) =>
                  setError(
                    extensionErrorMessage(e instanceof Error ? e.message : "切换失败，请稍后重试")
                  )
                )
                .finally(() => setBusy(false))
            }}
            aria-label={`${ext.enabled ? "停用" : "启用"} ${ext.name}`}
            title={ext.enabled ? "停用" : "启用"}
            className="mt-0.5 shrink-0"
          />
        </div>
      </div>

      {hasSettings && settingsOpen && (
        <ExtensionSettings settings={ext.settings} note={ext.settingsNote} onSave={onSaveSetting} />
      )}
    </div>
  )
}
