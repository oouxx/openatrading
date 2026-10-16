import { useState } from "react"
import { useQuery } from "@tanstack/react-query"
import { fetchJson } from "@/stores/data"
import { Popover, PopoverContent, PopoverTrigger } from "@/components/ui/popover"
import {
  Command,
  CommandInput,
  CommandList,
  CommandEmpty,
  CommandItem,
} from "@/components/ui/command"
import { Button } from "@/components/ui/button"
import { Skeleton } from "@/components/ui/skeleton"
import { CheckIcon, ChevronDownIcon, LockKeyholeIcon } from "lucide-react"
import { SettingRow, SelectField } from "./settings-ui"
import { agentModelsKeys } from "@/hooks/query/queryKeys"

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

/** Model 触发按钮：Codex 风格下拉（右对齐、固定宽度 220px） */
const triggerClass =
  "bg-surface border-input hover:bg-hover text-primary flex h-8 w-[220px] items-center justify-between gap-2 rounded-lg border px-2.5 text-xs transition-colors focus:border-ring focus:outline-none disabled:cursor-not-allowed disabled:opacity-50"

/** 上下文窗口格式化：128000 → "128K"、1000000 → "1M" */
function formatContext(n: number): string {
  if (!n) return ""
  if (n >= 1_000_000) {
    const m = n / 1_000_000
    return `${m >= 10 ? Math.round(m) : m.toFixed(1).replace(/\.0$/, "")}M`
  }
  return `${(n / 1000).toFixed(0)}K`
}

/**
 * 模型下拉项：Codex 模型选择器风格 — 第一行名称 + 能力徽标，
 * 第二行 ID / 上下文窗口 / 密钥状态；不可用模型置灰并加锁。
 */
function ModelItem({
  option,
  selected,
  onSelect,
}: {
  option: ModelOption
  selected: boolean
  onSelect: () => void
}) {
  const { id, name, reasoning, contextWindow, available } = option
  const context = formatContext(contextWindow)
  return (
    <CommandItem
      value={`${name} ${id}`}
      disabled={!available}
      onSelect={onSelect}
      className="hover:bg-hover data-selected:bg-hover data-selected:text-primary text-caption flex w-full items-center gap-2 px-3 py-2 text-left transition-colors"
    >
      <div className="min-w-0 flex-1">
        <div className="flex items-center gap-1.5">
          <span className="min-w-0 truncate text-[13px] font-medium">{name || id}</span>
          {reasoning && (
            <span className="text-secondary border-hairline shrink-0 rounded border px-1 py-px text-[10px] leading-3">
              推理
            </span>
          )}
          {!available && <LockKeyholeIcon className="text-secondary size-3 shrink-0" />}
        </div>
        <p className="text-secondary font-number mt-0.5 truncate text-[11px]">
          {id}
          {context ? ` · ${context} 上下文` : ""}
          {!available ? " · 未配置密钥" : ""}
        </p>
      </div>
      {selected && <CheckIcon className="text-brand size-3.5 shrink-0" />}
    </CommandItem>
  )
}

/**
 * 从 pi-rs 可选模型列表（/api/agent/models）中选择 provider + model。
 *
 * 保存逻辑与 Claude Code `/model`、ChatGPT 模型选择器一致：**选中即保存**，
 * 没有单独的保存按钮。Provider 只是分组导航（切换只更新候选列表，不落盘），
 * 在 Model 搜索框里选中某个模型时才一次性提交 provider + model。
 */
export default function ModelPicker({ provider, model, onChange, disabled }: ModelPickerProps) {
  const [open, setOpen] = useState(false)
  // 切换 Provider 时的草稿状态：未选中 Model 前不提交保存
  const [draftProvider, setDraftProvider] = useState<string | null>(null)

  // 模型列表走 react-query 缓存：staleTime 内重复进入设置页不重新请求
  // key 取自 queryKeys——保存 API Key 后要按同一 key 失效，才能立刻刷新置灰状态
  const modelsQuery = useQuery({
    queryKey: agentModelsKeys.all,
    queryFn: () => fetchJson<{ providers: ProviderOption[] }>("/api/agent/models"),
    staleTime: 5 * 60 * 1000,
    retry: 1,
  })
  const providers = modelsQuery.data?.providers ?? []
  const loading = modelsQuery.isPending

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

  const loadFailed = modelsQuery.isError && providers.length === 0
  const noModels = !loadFailed && activeModels.length === 0
  const triggerLabel =
    currentModel?.name ||
    model ||
    (loadFailed ? "加载失败" : noModels ? "暂无可用模型" : "选择模型...")

  const providerOptions = [
    // 当前值不在候选列表时保留展示
    ...(activeProvider && !providers.some((p) => p.name === activeProvider)
      ? [{ value: activeProvider, label: activeProvider }]
      : []),
    ...providers.map((p) => ({ value: p.name, label: p.name })),
  ]

  return (
    <>
      <SettingRow
        label="Provider"
        description="模型供应商"
        control={
          loading ? (
            <Skeleton className="h-8 w-[200px] rounded-lg" />
          ) : (
            <SelectField
              value={activeProvider}
              disabled={disabled}
              onChange={handleProviderChange}
              options={providerOptions}
              className="w-[200px]"
            />
          )
        }
      />
      <SettingRow
        label="Model"
        description="默认使用的模型"
        control={
          loading ? (
            <Skeleton className="h-8 w-[220px] rounded-lg" />
          ) : (
            <Popover open={open} onOpenChange={setOpen}>
              <PopoverTrigger asChild>
                <button
                  type="button"
                  aria-haspopup="listbox"
                  disabled={disabled || noModels}
                  className={triggerClass}
                >
                  <span className="truncate">{triggerLabel}</span>
                  <ChevronDownIcon className="size-3.5 shrink-0 opacity-50" />
                </button>
              </PopoverTrigger>
              <PopoverContent
                align="start"
                className="bg-surface border-hairline shadow-modal w-80 gap-0 rounded-lg border p-0 ring-0"
              >
                {loadFailed ? (
                  <div className="flex flex-col items-center gap-2 py-6 text-center">
                    <p className="text-secondary text-caption">无法加载模型列表</p>
                    <Button size="xs" variant="outline" onClick={() => modelsQuery.refetch()}>
                      重试
                    </Button>
                  </div>
                ) : (
                  <Command className="rounded-lg!">
                    <CommandInput placeholder="搜索模型名称或 ID..." />
                    <CommandList className="max-h-80">
                      <CommandEmpty className="text-secondary text-caption py-4">
                        无匹配模型
                      </CommandEmpty>
                      {!draftProvider && model && !modelInList && (
                        <CommandItem
                          value={model}
                          onSelect={() => setOpen(false)}
                          className="hover:bg-hover data-selected:bg-hover data-selected:text-primary text-caption flex w-full items-center gap-2 px-3 py-2 text-left transition-colors"
                        >
                          <span className="min-w-0 flex-1 truncate">{model}</span>
                          <CheckIcon className="text-brand size-3.5 shrink-0" />
                        </CommandItem>
                      )}
                      {activeModels.map((m) => (
                        <ModelItem
                          key={m.id}
                          option={m}
                          selected={m.id === model}
                          onSelect={() => handleModelSelect(m.id)}
                        />
                      ))}
                    </CommandList>
                  </Command>
                )}
              </PopoverContent>
            </Popover>
          )
        }
      />
    </>
  )
}
