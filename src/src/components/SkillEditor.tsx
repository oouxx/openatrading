import { useEffect, useMemo, useState } from "react"
import {
  AlertCircle,
  Check,
  Info,
  Loader2,
  PanelLeft,
  Plus,
  Puzzle,
  RefreshCw,
  Save,
  Search,
  Trash2,
  X,
} from "lucide-react"
import { Button } from "@/components/ui/button"
import { Input } from "@/components/ui/input"
import { Skeleton } from "@/components/ui/skeleton"
import { Switch } from "@/components/ui/switch"
import { Tabs, TabsContent, TabsList, TabsTrigger } from "@/components/ui/tabs"
import { Textarea } from "@/components/ui/textarea"
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from "@/components/ui/dialog"
import {
  Sheet,
  SheetContent,
  SheetDescription,
  SheetHeader,
  SheetTitle,
} from "@/components/ui/sheet"
import { skillErrorMessage, useDeleteSkill, useSaveSkill, useSkills } from "@/hooks/query/useSkills"
import type { SkillEntry, SkillScope } from "@/types/skill"
import { cn } from "@/lib/utils"

type EditorTab = "manifest" | "content"

interface SkillDraft {
  name: string
  description: string
  content: string
  disableModelInvocation: boolean
}

const EMPTY_DRAFT: SkillDraft = {
  name: "",
  description: "",
  content: "",
  disableModelInvocation: false,
}

const NAME_PATTERN = /^[a-z0-9]+(?:-[a-z0-9]+)*$/
const MAX_NAME_LENGTH = 64
const MAX_DESCRIPTION_LENGTH = 1024
/** 稳定空数组，避免 `data ?? []` 每次渲染都让 useMemo 重新计算。 */
const EMPTY_SKILLS: SkillEntry[] = []

const SCOPE_META: Record<string, { label: string; badge: string; hint: string }> = {
  user: {
    label: "用户",
    badge: "bg-hover text-tertiary",
    hint: "保存在用户技能目录，可在这里编辑",
  },
  project: {
    label: "项目",
    badge: "bg-brand/10 text-brand",
    hint: "位于项目目录，请直接编辑 SKILL.md",
  },
  temporary: {
    label: "临时",
    badge: "bg-warning/10 text-warning",
    hint: "临时技能，重启后可能消失",
  },
}

function metaFor(scope: SkillScope) {
  return SCOPE_META[scope] ?? SCOPE_META.user
}

function skillToDraft(skill: SkillEntry): SkillDraft {
  return {
    name: skill.name,
    description: skill.description,
    content: skill.content ?? "",
    disableModelInvocation: !!skill.disable_model_invocation,
  }
}

function draftsEqual(a: SkillDraft, b: SkillDraft): boolean {
  return (
    a.name === b.name &&
    a.description === b.description &&
    a.content === b.content &&
    a.disableModelInvocation === b.disableModelInvocation
  )
}

function errorText(error: unknown): string {
  return error instanceof Error ? error.message : String(error ?? "未知错误")
}

function loadErrorMessage(raw: string): string {
  if (/failed to fetch|networkerror|load failed/i.test(raw)) {
    return "无法连接服务端，请确认 gateway 已启动"
  }
  if (/^5\d{2}$/.test(raw)) return "技能列表加载失败，请检查服务端后重试"
  if (/^\d{3}$/.test(raw)) return `加载失败（HTTP ${raw}）`
  return raw
}

function ScopeBadge({ scope }: { scope: SkillScope }) {
  const meta = metaFor(scope)
  return (
    <span
      className={cn("text-caption shrink-0 rounded-md px-1.5 py-0.5 leading-none", meta.badge)}
      title={meta.hint}
    >
      {meta.label}
    </span>
  )
}

interface SkillListBodyProps {
  skills: SkillEntry[]
  filteredSkills: SkillEntry[]
  selectedName: string | null
  isNew: boolean
  isLoading: boolean
  isFetching: boolean
  error: string | null
  search: string
  onSearchChange: (value: string) => void
  onSelect: (name: string) => void
  onNew: () => void
  onRefresh: () => void
}

/**
 * 侧边栏与移动端 Sheet 共用同一份列表，避免两套搜索 / 空态 / 错误态漂移。
 * 顶部 pr-10 给 Sheet 的关闭按钮留位置，桌面端恢复普通内边距。
 */
function SkillListBody({
  skills,
  filteredSkills,
  selectedName,
  isNew,
  isLoading,
  isFetching,
  error,
  search,
  onSearchChange,
  onSelect,
  onNew,
  onRefresh,
}: SkillListBodyProps) {
  const searching = search.trim().length > 0

  return (
    <div className="flex min-h-0 flex-1 flex-col">
      <div className="border-hairline border-b px-4 py-4 pr-10 md:pr-4">
        <div className="flex items-start justify-between gap-3">
          <div className="min-w-0">
            <h1 className="text-h2 text-primary font-medium">技能</h1>
            <p className="text-tertiary text-caption mt-0.5">
              {searching
                ? `${filteredSkills.length} / ${skills.length} 个技能`
                : `${skills.length} 个已安装`}
            </p>
          </div>
          <div className="flex shrink-0 items-center gap-1">
            <Button
              type="button"
              variant="ghost"
              size="icon-sm"
              className="text-tertiary hover:text-primary"
              onClick={onRefresh}
              disabled={isFetching}
              title="刷新技能列表"
              aria-label="刷新技能列表"
            >
              <RefreshCw className={cn("size-3.5", isFetching && "animate-spin")} />
            </Button>
            <Button type="button" size="sm" onClick={onNew}>
              <Plus className="size-3.5" />
              新建
            </Button>
          </div>
        </div>

        <div className="relative mt-3">
          <Search className="text-tertiary pointer-events-none absolute top-1/2 left-2.5 size-3.5 -translate-y-1/2" />
          <Input
            value={search}
            onChange={(event) => onSearchChange(event.target.value)}
            placeholder="搜索技能名称或描述…"
            className="bg-canvas border-hairline focus-visible:border-brand text-caption h-8 pr-8 pl-8"
            aria-label="搜索技能"
          />
          {search && (
            <Button
              type="button"
              variant="ghost"
              size="icon-xs"
              className="text-tertiary hover:text-primary absolute top-1/2 right-1 -translate-y-1/2"
              onClick={() => onSearchChange("")}
              title="清除搜索"
              aria-label="清除搜索"
            >
              <X className="size-3" />
            </Button>
          )}
        </div>
      </div>

      <div className="min-h-0 flex-1 overflow-y-auto px-2 py-2">
        {isLoading ? (
          <div className="space-y-2 px-2 py-1">
            {Array.from({ length: 5 }).map((_, index) => (
              <div key={index} className="space-y-2 rounded-lg px-2 py-2.5">
                <Skeleton className="h-4 w-2/3" />
                <Skeleton className="h-3 w-full" />
              </div>
            ))}
          </div>
        ) : error && skills.length === 0 ? (
          <div className="flex flex-col items-center gap-2 px-4 py-8 text-center">
            <AlertCircle className="text-danger size-4" />
            <p className="text-danger text-caption">技能列表加载失败</p>
            <p className="text-tertiary text-caption break-all">{error}</p>
            <Button type="button" variant="outline" size="xs" onClick={onRefresh}>
              重试
            </Button>
          </div>
        ) : filteredSkills.length === 0 ? (
          <div className="flex flex-col items-center gap-3 px-4 py-10 text-center">
            <p className="text-secondary text-caption">
              {searching ? "没有匹配的技能" : "还没有安装技能"}
            </p>
            {searching ? (
              <Button type="button" variant="ghost" size="xs" onClick={() => onSearchChange("")}>
                清除搜索
              </Button>
            ) : (
              <Button type="button" variant="outline" size="xs" onClick={onNew}>
                <Plus className="size-3" />
                新建技能
              </Button>
            )}
          </div>
        ) : (
          <div className="space-y-1">
            {error && <p className="text-danger text-caption px-2 py-1.5">刷新失败：{error}</p>}
            {filteredSkills.map((skill) => {
              const selected = !isNew && selectedName === skill.name
              return (
                <Button
                  key={skill.name}
                  type="button"
                  variant="ghost"
                  aria-pressed={selected}
                  onClick={() => onSelect(skill.name)}
                  className={cn(
                    "flex h-auto w-full flex-col items-stretch justify-start gap-0 rounded-none border-l-2 px-3 py-2.5 text-left font-normal whitespace-normal",
                    selected
                      ? "border-l-brand bg-brand/5 hover:bg-brand/5 hover:text-primary"
                      : "text-secondary hover:bg-hover hover:text-primary border-l-transparent"
                  )}
                >
                  <span className="flex min-w-0 items-center gap-1.5">
                    <span className="text-body text-primary min-w-0 truncate font-medium">
                      {skill.name}
                    </span>
                    <ScopeBadge scope={skill.scope} />
                  </span>
                  <span
                    className="text-secondary text-caption mt-1 line-clamp-2 leading-4"
                    title={skill.description}
                  >
                    {skill.description || "暂无描述"}
                  </span>
                </Button>
              )
            })}
          </div>
        )}
      </div>
    </div>
  )
}

function EmptyEditor({
  loading,
  error,
  onRetry,
  onNew,
  onOpenList,
}: {
  loading: boolean
  error: string | null
  onRetry: () => void
  onNew: () => void
  onOpenList: () => void
}) {
  if (loading) {
    return (
      <div className="text-secondary flex flex-1 flex-col items-center justify-center gap-2">
        <Loader2 className="text-tertiary size-4 animate-spin" />
        <span className="text-caption">正在加载技能…</span>
      </div>
    )
  }

  if (error) {
    return (
      <div className="flex flex-1 flex-col items-center justify-center gap-3 px-6 text-center">
        <AlertCircle className="text-danger size-6" />
        <div>
          <p className="text-primary text-body font-medium">无法加载技能列表</p>
          <p className="text-tertiary text-caption mt-1 break-all">{error}</p>
        </div>
        <Button type="button" variant="outline" size="sm" onClick={onRetry}>
          重试
        </Button>
      </div>
    )
  }

  return (
    <div className="flex flex-1 flex-col items-center justify-center gap-4 px-6 text-center">
      <div className="bg-hover text-secondary grid size-12 place-items-center rounded-xl">
        <Puzzle className="size-5" />
      </div>
      <div className="max-w-sm">
        <h2 className="text-body text-primary font-medium">技能</h2>
        <p className="text-secondary text-caption mt-1.5 leading-5">
          技能让 Agent 掌握可复用的工作流。选择一个技能开始编辑，或新建一个从零开始。
        </p>
      </div>
      <div className="flex items-center gap-2">
        <Button type="button" size="sm" onClick={onNew}>
          <Plus className="size-3.5" />
          新建技能
        </Button>
        <Button
          type="button"
          variant="outline"
          size="sm"
          className="md:hidden"
          onClick={onOpenList}
        >
          <PanelLeft className="size-3.5" />
          技能列表
        </Button>
      </div>
    </div>
  )
}

export default function SkillEditor() {
  const skillsQuery = useSkills()
  const saveMutation = useSaveSkill()
  const deleteMutation = useDeleteSkill()

  const skills = skillsQuery.data ?? EMPTY_SKILLS
  const [selectedName, setSelectedName] = useState<string | null>(null)
  const [isNew, setIsNew] = useState(false)
  const [activeTab, setActiveTab] = useState<EditorTab>("manifest")
  const [search, setSearch] = useState("")
  const [draft, setDraft] = useState<SkillDraft>(EMPTY_DRAFT)
  const [baseline, setBaseline] = useState<SkillDraft>(EMPTY_DRAFT)
  const [nameTouched, setNameTouched] = useState(false)
  const [descTouched, setDescTouched] = useState(false)
  const [showValidation, setShowValidation] = useState(false)
  const [saveNotice, setSaveNotice] = useState<{
    kind: "success" | "error"
    message: string
  } | null>(null)
  const [pendingAction, setPendingAction] = useState<
    { kind: "select"; name: string } | { kind: "new" } | null
  >(null)
  const [deleteOpen, setDeleteOpen] = useState(false)
  const [deleteError, setDeleteError] = useState<string | null>(null)
  const [mobileListOpen, setMobileListOpen] = useState(false)

  const selected = selectedName
    ? (skills.find((skill) => skill.name === selectedName) ?? null)
    : null
  const hasEditor = isNew || selected !== null
  const readOnly = selected !== null && selected.scope !== "user"
  const dirty = !draftsEqual(draft, baseline)
  const saving = saveMutation.isPending
  const deleting = deleteMutation.isPending
  const skillsLoading = skillsQuery.isPending
  const listError = skillsQuery.error ? loadErrorMessage(errorText(skillsQuery.error)) : null

  const filteredSkills = useMemo(() => {
    const query = search.trim().toLowerCase()
    const rank: Record<SkillScope, number> = { user: 0, project: 1, temporary: 2 }
    return skills
      .filter((skill) => {
        if (!query) return true
        return (
          skill.name.toLowerCase().includes(query) ||
          skill.description.toLowerCase().includes(query) ||
          skill.file_path.toLowerCase().includes(query)
        )
      })
      .sort((a, b) => (rank[a.scope] ?? 9) - (rank[b.scope] ?? 9) || a.name.localeCompare(b.name))
  }, [skills, search])

  const trimmedName = draft.name.trim()
  const normalizedDescription = draft.description.replace(/\s+/g, " ").trim()

  const nameError = useMemo(() => {
    if (!trimmedName) return "请输入技能名称"
    if (isNew && trimmedName.length > MAX_NAME_LENGTH) {
      return `技能名称不能超过 ${MAX_NAME_LENGTH} 个字符`
    }
    if (isNew && !NAME_PATTERN.test(trimmedName)) {
      return "只能使用小写字母、数字和连字符，且不能以连字符开头或结尾"
    }
    if (isNew && skills.some((skill) => skill.name === trimmedName)) {
      return "已存在同名技能，请换一个名称"
    }
    return null
  }, [isNew, skills, trimmedName])

  const descriptionError = useMemo(() => {
    if (!normalizedDescription) return "请输入技能描述"
    if (normalizedDescription.length > MAX_DESCRIPTION_LENGTH) {
      return `技能描述不能超过 ${MAX_DESCRIPTION_LENGTH} 个字符`
    }
    return null
  }, [normalizedDescription])

  const showNameError = (nameTouched || showValidation) && nameError
  const showDescriptionError = (descTouched || showValidation) && descriptionError
  const canSave = hasEditor && !readOnly && dirty && !saving
  const lineCount = draft.content ? draft.content.split("\n").length : 0
  const pathLabel = isNew
    ? "保存后写入用户技能目录"
    : (selected?.file_path ?? "技能不存在或已被删除")

  const resetEditorState = () => {
    setShowValidation(false)
    setNameTouched(false)
    setDescTouched(false)
    setSaveNotice(null)
  }

  const applySkill = (skill: SkillEntry) => {
    const next = skillToDraft(skill)
    setSelectedName(skill.name)
    setIsNew(false)
    setDraft(next)
    setBaseline(next)
    setActiveTab("manifest")
    resetEditorState()
    setMobileListOpen(false)
  }

  const startNew = () => {
    setSelectedName(null)
    setIsNew(true)
    setDraft(EMPTY_DRAFT)
    setBaseline(EMPTY_DRAFT)
    setActiveTab("manifest")
    resetEditorState()
    setMobileListOpen(false)
  }

  const requestSelect = (name: string) => {
    if (!isNew && name === selectedName) {
      setMobileListOpen(false)
      return
    }
    const skill = skills.find((item) => item.name === name)
    if (!skill) return
    if (dirty) {
      setMobileListOpen(false)
      setPendingAction({ kind: "select", name })
      return
    }
    applySkill(skill)
  }

  const requestNew = () => {
    if (isNew) return
    if (dirty) {
      setMobileListOpen(false)
      setPendingAction({ kind: "new" })
      return
    }
    startNew()
  }

  const confirmDiscard = () => {
    const action = pendingAction
    setPendingAction(null)
    if (!action) return
    if (action.kind === "new") {
      startNew()
      return
    }
    const skill = skills.find((item) => item.name === action.name)
    if (skill) applySkill(skill)
  }

  const updateDraft = (patch: Partial<SkillDraft>) => {
    setDraft((current) => ({ ...current, ...patch }))
    setSaveNotice(null)
  }

  const handleSave = async () => {
    setShowValidation(true)
    if (!hasEditor || readOnly || !dirty || saving) return
    if (nameError || descriptionError) return

    const payload = {
      name: trimmedName,
      description: normalizedDescription,
      content: draft.content,
      disableModelInvocation: draft.disableModelInvocation,
    }

    try {
      const saved = await saveMutation.mutateAsync(payload)
      const next: SkillDraft = saved
        ? skillToDraft(saved)
        : {
            name: payload.name,
            description: payload.description,
            content: payload.content,
            disableModelInvocation: payload.disableModelInvocation,
          }
      setDraft(next)
      setBaseline(next)
      setSelectedName(next.name)
      setIsNew(false)
      setNameTouched(false)
      setDescTouched(false)
      setShowValidation(false)
      setSaveNotice({ kind: "success", message: "已保存" })
    } catch (error) {
      setSaveNotice({
        kind: "error",
        message: skillErrorMessage(errorText(error)),
      })
    }
  }

  const handleDelete = async () => {
    if (!selected) return
    setDeleteError(null)
    try {
      await deleteMutation.mutateAsync(selected.name)
      setDeleteOpen(false)
      setSelectedName(null)
      setIsNew(false)
      setDraft(EMPTY_DRAFT)
      setBaseline(EMPTY_DRAFT)
      resetEditorState()
    } catch (error) {
      setDeleteError(skillErrorMessage(errorText(error)))
    }
  }

  useEffect(() => {
    if (!dirty) return
    const handleBeforeUnload = (event: BeforeUnloadEvent) => {
      event.preventDefault()
      event.returnValue = ""
    }
    window.addEventListener("beforeunload", handleBeforeUnload)
    return () => window.removeEventListener("beforeunload", handleBeforeUnload)
  }, [dirty])

  useEffect(() => {
    const handleKeyDown = (event: KeyboardEvent) => {
      if (!hasEditor) return
      if (!(event.metaKey || event.ctrlKey) || event.key.toLowerCase() !== "s") return
      event.preventDefault()
      if (canSave) void handleSave()
    }
    window.addEventListener("keydown", handleKeyDown)
    return () => window.removeEventListener("keydown", handleKeyDown)
  })

  return (
    <>
      <div className="flex h-full min-h-0">
        <aside className="bg-surface border-hairline hidden w-72 shrink-0 flex-col border-r md:flex">
          <SkillListBody
            skills={skills}
            filteredSkills={filteredSkills}
            selectedName={selectedName}
            isNew={isNew}
            isLoading={skillsLoading}
            isFetching={skillsQuery.isFetching}
            error={listError}
            search={search}
            onSearchChange={setSearch}
            onSelect={requestSelect}
            onNew={requestNew}
            onRefresh={() => void skillsQuery.refetch()}
          />
        </aside>

        <main className="flex min-h-0 min-w-0 flex-1 flex-col">
          {!hasEditor ? (
            <EmptyEditor
              loading={skillsLoading}
              error={listError}
              onRetry={() => void skillsQuery.refetch()}
              onNew={requestNew}
              onOpenList={() => setMobileListOpen(true)}
            />
          ) : (
            <Tabs
              value={activeTab}
              onValueChange={(value) => setActiveTab(value as EditorTab)}
              className="flex min-h-0 flex-1 flex-col gap-0"
            >
              <header className="border-hairline border-b">
                <div className="flex flex-wrap items-center justify-between gap-3 px-5 py-3">
                  <div className="flex min-w-0 items-center gap-2">
                    <Button
                      type="button"
                      variant="ghost"
                      size="icon-sm"
                      className="text-secondary md:hidden"
                      onClick={() => setMobileListOpen(true)}
                      title="打开技能列表"
                      aria-label="打开技能列表"
                    >
                      <PanelLeft className="size-4" />
                    </Button>
                    <div className="min-w-0">
                      <div className="flex flex-wrap items-center gap-2">
                        <h2 className="text-body text-primary min-w-0 truncate font-semibold">
                          {isNew ? "新建技能" : (selected?.name ?? draft.name)}
                        </h2>
                        {selected && <ScopeBadge scope={selected.scope} />}
                        {readOnly && <span className="text-tertiary text-caption">只读</span>}
                        {dirty && (
                          <span className="text-warning text-caption flex items-center gap-1">
                            <span className="bg-warning size-1.5 rounded-full" />
                            未保存
                          </span>
                        )}
                        {!dirty && saveNotice?.kind === "success" && (
                          <span className="text-success text-caption flex items-center gap-1">
                            <Check className="size-3" />
                            {saveNotice.message}
                          </span>
                        )}
                      </div>
                      <p
                        className="text-tertiary font-number text-caption mt-0.5 truncate"
                        title={pathLabel}
                      >
                        {pathLabel}
                      </p>
                    </div>
                  </div>

                  <div className="flex shrink-0 items-center gap-1.5">
                    {selected?.scope === "user" && (
                      <Button
                        type="button"
                        variant="ghost"
                        size="icon-sm"
                        className="text-tertiary hover:text-danger"
                        onClick={() => {
                          setDeleteError(null)
                          setDeleteOpen(true)
                        }}
                        title="删除技能"
                        aria-label={`删除技能 ${selected.name}`}
                      >
                        <Trash2 className="size-3.5" />
                      </Button>
                    )}
                    {!readOnly && (
                      <Button
                        type="button"
                        size="sm"
                        onClick={() => void handleSave()}
                        disabled={!canSave}
                        title={!dirty ? "没有未保存的更改" : "保存（⌘S）"}
                      >
                        {saving ? (
                          <Loader2 className="size-3.5 animate-spin" />
                        ) : (
                          <Save className="size-3.5" />
                        )}
                        {saving ? "保存中…" : "保存"}
                      </Button>
                    )}
                  </div>
                </div>

                {saveNotice?.kind === "error" && (
                  <div className="border-hairline bg-danger/5 flex items-start gap-2 border-t px-5 py-2.5">
                    <AlertCircle className="text-danger mt-0.5 size-3.5 shrink-0" />
                    <p className="text-danger text-caption">{saveNotice.message}</p>
                  </div>
                )}

                {readOnly && selected && (
                  <div className="border-hairline bg-hover/40 flex items-start gap-2 border-t px-5 py-2.5">
                    <Info className="text-tertiary mt-0.5 size-3.5 shrink-0" />
                    <p className="text-secondary text-caption">
                      {metaFor(selected.scope).label}技能为只读。请直接编辑{" "}
                      <span className="text-primary font-number break-all">
                        {selected.file_path}
                      </span>{" "}
                      后点击刷新按钮。
                    </p>
                  </div>
                )}

                <div className="flex items-center justify-between gap-3 px-5 py-2">
                  <TabsList className="bg-hover h-8 p-1">
                    <TabsTrigger
                      value="manifest"
                      className="text-caption data-active:bg-surface data-active:text-primary flex-none px-3"
                    >
                      配置
                    </TabsTrigger>
                    <TabsTrigger
                      value="content"
                      className="text-caption data-active:bg-surface data-active:text-primary flex-none px-3"
                    >
                      内容
                    </TabsTrigger>
                  </TabsList>
                  <span className="text-tertiary text-caption">
                    {activeTab === "content"
                      ? `${lineCount} 行 · ${draft.content.length} 字符`
                      : "SKILL.md"}
                  </span>
                </div>
              </header>

              <TabsContent value="manifest" className="min-h-0 flex-1 overflow-y-auto">
                <div className="mx-auto w-full max-w-2xl space-y-4 px-5 py-5">
                  <section className="bg-surface border-hairline rounded-lg border p-5">
                    <div>
                      <h3 className="text-body text-primary font-semibold">基本信息</h3>
                      <p className="text-tertiary text-caption mt-1">
                        告诉 Agent 这个技能做什么、什么时候应该使用。
                      </p>
                    </div>

                    <div className="mt-5 space-y-5">
                      <div>
                        <label
                          htmlFor="skill-name"
                          className="text-secondary text-caption mb-1.5 flex items-center gap-1 font-medium"
                        >
                          名称
                          <span className="text-danger">*</span>
                        </label>
                        <Input
                          id="skill-name"
                          value={draft.name}
                          onChange={(event) => updateDraft({ name: event.target.value })}
                          onBlur={() => setNameTouched(true)}
                          readOnly={!isNew}
                          aria-invalid={!!showNameError}
                          maxLength={isNew ? MAX_NAME_LENGTH : undefined}
                          placeholder="market-review"
                          className="font-number text-caption"
                        />
                        {showNameError ? (
                          <p className="text-danger text-caption mt-1">{nameError}</p>
                        ) : (
                          <p className="text-tertiary text-caption mt-1">
                            {isNew
                              ? "小写字母、数字和连字符，例如 market-review"
                              : "名称创建后不可修改"}
                          </p>
                        )}
                      </div>

                      <div>
                        <label
                          htmlFor="skill-description"
                          className="text-secondary text-caption mb-1.5 flex items-center gap-1 font-medium"
                        >
                          描述
                          <span className="text-danger">*</span>
                        </label>
                        <Textarea
                          id="skill-description"
                          value={draft.description}
                          onChange={(event) => updateDraft({ description: event.target.value })}
                          onBlur={() => setDescTouched(true)}
                          readOnly={readOnly}
                          rows={3}
                          aria-invalid={!!showDescriptionError}
                          placeholder="当用户需要复盘当日行情时使用…"
                          className="text-caption min-h-20 resize-none"
                        />
                        <div className="mt-1 flex items-start justify-between gap-3">
                          {showDescriptionError ? (
                            <p className="text-danger text-caption">{descriptionError}</p>
                          ) : (
                            <p className="text-tertiary text-caption">
                              单行描述，保存时会自动合并换行
                            </p>
                          )}
                          <span className="text-tertiary font-number text-caption shrink-0">
                            {draft.description.length} / {MAX_DESCRIPTION_LENGTH}
                          </span>
                        </div>
                      </div>
                    </div>
                  </section>

                  <section className="bg-surface border-hairline rounded-lg border p-5">
                    <h3 className="text-body text-primary font-semibold">调用行为</h3>
                    <div className="mt-3 flex items-center justify-between gap-6">
                      <span className="text-body text-secondary">允许模型自动调用</span>
                      <Switch
                        checked={!draft.disableModelInvocation}
                        onCheckedChange={(checked) =>
                          updateDraft({ disableModelInvocation: !checked })
                        }
                        disabled={readOnly}
                        aria-label="允许模型自动调用"
                      />
                    </div>
                    {/* 开关自身的状态由 Switch 表达；只在关闭时补充「关闭后怎么触发」这个副作用 */}
                    {draft.disableModelInvocation && (
                      <p className="text-secondary text-caption bg-canvas mt-3 rounded-lg px-3 py-2.5 leading-5">
                        已关闭自动调用，只能用{" "}
                        <span className="font-number text-primary">
                          /skill:{trimmedName || "skill-name"}
                        </span>{" "}
                        手动触发。
                      </p>
                    )}
                  </section>
                </div>
              </TabsContent>

              <TabsContent value="content" className="flex min-h-0 flex-1 flex-col overflow-y-auto">
                <div className="mx-auto flex min-h-0 w-full max-w-4xl flex-1 flex-col gap-3 px-5 py-5">
                  <div className="flex items-end justify-between gap-3">
                    <div>
                      <h3 className="text-body text-primary font-semibold">指令内容</h3>
                      <p className="text-tertiary text-caption mt-0.5">
                        Markdown 正文；保存时会自动生成 frontmatter。
                      </p>
                    </div>
                  </div>
                  <div className="bg-surface border-hairline flex min-h-0 flex-1 flex-col rounded-lg border p-4">
                    <Textarea
                      value={draft.content}
                      onChange={(event) => updateDraft({ content: event.target.value })}
                      readOnly={readOnly}
                      spellCheck={false}
                      placeholder={"## 角色\n\n你是一个…"}
                      aria-label="技能指令内容"
                      className="font-number text-caption field-sizing-fixed min-h-80 flex-1 resize-none border-0 bg-transparent p-0 leading-6 shadow-none focus-visible:ring-0"
                    />
                  </div>
                </div>
              </TabsContent>
            </Tabs>
          )}
        </main>
      </div>

      <Sheet open={mobileListOpen} onOpenChange={setMobileListOpen}>
        <SheetContent side="left" className="bg-surface text-primary w-80 max-w-[85vw] gap-0 p-0">
          <SheetHeader className="sr-only">
            <SheetTitle>技能列表</SheetTitle>
            <SheetDescription>搜索、选择或新建技能</SheetDescription>
          </SheetHeader>
          <SkillListBody
            skills={skills}
            filteredSkills={filteredSkills}
            selectedName={selectedName}
            isNew={isNew}
            isLoading={skillsLoading}
            isFetching={skillsQuery.isFetching}
            error={listError}
            search={search}
            onSearchChange={setSearch}
            onSelect={requestSelect}
            onNew={requestNew}
            onRefresh={() => void skillsQuery.refetch()}
          />
        </SheetContent>
      </Sheet>

      <Dialog
        open={pendingAction !== null}
        onOpenChange={(open) => {
          if (!open) setPendingAction(null)
        }}
      >
        <DialogContent className="sm:max-w-md">
          <DialogHeader>
            <DialogTitle>放弃未保存的更改？</DialogTitle>
            <DialogDescription>当前技能有未保存的修改，切换后会丢失这些内容。</DialogDescription>
          </DialogHeader>
          <DialogFooter>
            <Button type="button" variant="ghost" onClick={() => setPendingAction(null)}>
              继续编辑
            </Button>
            <Button type="button" variant="destructive" onClick={confirmDiscard}>
              放弃更改
            </Button>
          </DialogFooter>
        </DialogContent>
      </Dialog>

      <Dialog
        open={deleteOpen}
        onOpenChange={(open) => {
          setDeleteOpen(open)
          if (!open) setDeleteError(null)
        }}
      >
        <DialogContent className="sm:max-w-md">
          <DialogHeader>
            <DialogTitle>删除技能「{selected?.name}」？</DialogTitle>
            <DialogDescription>
              删除会连同技能目录一起移除，其中的 SKILL.md 与附带文件全部清空，无法恢复。
            </DialogDescription>
          </DialogHeader>
          {deleteError && <p className="text-danger text-caption">{deleteError}</p>}
          <DialogFooter>
            <Button type="button" variant="ghost" onClick={() => setDeleteOpen(false)}>
              取消
            </Button>
            <Button
              type="button"
              variant="destructive"
              disabled={deleting}
              onClick={() => void handleDelete()}
            >
              {deleting ? "删除中…" : "删除"}
            </Button>
          </DialogFooter>
        </DialogContent>
      </Dialog>
    </>
  )
}
