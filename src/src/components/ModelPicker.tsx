import { useEffect, useState } from "react"
import { fetchJson } from "@/stores/data"
import { Popover, PopoverContent, PopoverTrigger } from "@/components/ui/popover"
import {
  Command,
  CommandInput,
  CommandList,
  CommandEmpty,
  CommandItem,
} from "@/components/ui/command"
import { ChevronDownIcon, CheckIcon } from "lucide-react"

export interface ModelOption {
  id: string
  name: string
  reasoning: boolean
  contextWindow: number
  maxTokens: number
  available: boolean
}

export interface ProviderOption {
  name: string
  models: ModelOption[]
}

interface ModelPickerProps {
  provider: string
  model: string
  onChange: (provider: string, model: string) => void
  disabled?: boolean
}

const selectClass =
  "bg-elevated border-hairline text-primary w-full rounded-lg border px-3 py-2 font-number text-caption focus:outline-none"

/**
 * 从 pi-rs 可选模型列表（/api/agent/models）中选择 provider + model。
 *
 * 保存逻辑与 Claude Code `/model`、ChatGPT 模型选择器一致：**选中即保存**，
 * 没有单独的保存按钮。Provider 只是分组导航（切换只更新候选列表，不落盘），
 * 在 Model 搜索框里选中某个模型时才一次性提交 provider + model。
 */
export default function ModelPicker({ provider, model, onChange, disabled }: ModelPickerProps) {
  const [providers, setProviders] = useState<ProviderOption[]>([])
  const [loading, setLoading] = useState(true)
  const [open, setOpen] = useState(false)
  // 切换 Provider 时的草稿状态：未选中 Model 前不提交保存
  const [draftProvider, setDraftProvider] = useState<string | null>(null)

  useEffect(() => {
    let active = true
    fetchJson<{ providers: ProviderOption[] }>("/api/agent/models")
      .then((data) => {
        if (active) setProviders(data.providers ?? [])
      })
      .catch(() => {
        // 列表不可用时保持空，仍可通过保留选项展示当前值
      })
      .finally(() => {
        if (active) setLoading(false)
      })
    return () => {
      active = false
    }
  }, [])

  const activeProvider = draftProvider ?? provider
  const activeModels = providers.find((p) => p.name === activeProvider)?.models ?? []
  const currentModel = activeModels.find((m) => m.id === model)
  const modelInList = activeModels.some((m) => m.id === model)

  const handleProviderChange = (nextProvider: string) => {
    // 只切换候选列表，不保存；等用户选中 Model 时一起提交
    setDraftProvider(nextProvider)
  }

  const handleModelSelect = (modelId: string) => {
    onChange(activeProvider, modelId)
    setDraftProvider(null)
    setOpen(false)
  }

  const triggerLabel = draftProvider
    ? currentModel?.name || "选择模型..."
    : currentModel?.name || model || "选择模型..."

  return (
    <div className="space-y-3">
      <div>
        <label className="text-secondary mb-1 block text-caption">Provider</label>
        <select
          value={activeProvider}
          disabled={disabled || loading}
          onChange={(e) => handleProviderChange(e.target.value)}
          className={selectClass}
        >
          {activeProvider && !providers.some((p) => p.name === activeProvider) && (
            <option value={activeProvider}>{activeProvider}</option>
          )}
          {providers.map((p) => (
            <option key={p.name} value={p.name}>
              {p.name}
            </option>
          ))}
        </select>
      </div>
      <div>
        <label className="text-secondary mb-1 block text-caption">Model</label>
        <Popover open={open} onOpenChange={setOpen}>
          <PopoverTrigger asChild>
            <button
              type="button"
              disabled={disabled || loading || activeModels.length === 0}
              className="bg-elevated border-hairline text-primary flex w-full items-center justify-between gap-2 rounded-lg border px-3 py-2 font-number text-caption focus:outline-none disabled:cursor-not-allowed disabled:opacity-50"
            >
              <span className="truncate">{triggerLabel}</span>
              <ChevronDownIcon className="size-4 shrink-0 opacity-50" />
            </button>
          </PopoverTrigger>
          <PopoverContent
            align="start"
            className="bg-surface border-hairline w-[var(--radix-popover-trigger-width)] gap-0 rounded-lg border p-0 shadow-modal ring-0"
          >
            <Command className="rounded-lg!">
              <CommandInput placeholder="搜索模型名称或 ID..." />
              <CommandList className="max-h-72">
                <CommandEmpty className="text-secondary py-3 text-caption">无匹配模型</CommandEmpty>
                {!draftProvider && model && !modelInList && (
                  <CommandItem
                    value={model}
                    onSelect={() => setOpen(false)}
                    className="hover:bg-hover data-selected:bg-hover data-selected:text-primary flex w-full items-center gap-2 px-3 py-2 text-left text-caption transition-colors"
                  >
                    <span className="min-w-0 flex-1 truncate">{model}</span>
                    <CheckIcon className="text-brand size-3.5 shrink-0" />
                  </CommandItem>
                )}
                {activeModels.map((m) => (
                  <CommandItem
                    key={m.id}
                    value={`${m.name} ${m.id}`}
                    onSelect={() => handleModelSelect(m.id)}
                    className="hover:bg-hover data-selected:bg-hover data-selected:text-primary flex w-full items-center gap-2 px-3 py-2 text-left text-caption transition-colors"
                  >
                    <span className="min-w-0 flex-1 truncate">{m.name || m.id}</span>
                    <span className="text-secondary shrink-0 font-number">{m.id}</span>
                    {m.available ? null : <span className="text-secondary shrink-0">未配置密钥</span>}
                    {m.id === model && <CheckIcon className="text-brand size-3.5 shrink-0" />}
                  </CommandItem>
                ))}
              </CommandList>
            </Command>
          </PopoverContent>
        </Popover>
      </div>
    </div>
  )
}
