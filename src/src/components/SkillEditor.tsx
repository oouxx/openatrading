import { useState, useEffect } from "react"
import { Input } from "@/components/ui/input"
import { Textarea } from "@/components/ui/textarea"
import { Button } from "@/components/ui/button"
import { ScrollArea } from "@/components/ui/scroll-area"
import { Checkbox } from "@/components/ui/checkbox"
import { Card, CardContent, CardHeader, CardTitle } from "@/components/ui/card"
import { fetchJson } from "@/stores/data"

interface SkillEntry {
  name: string
  description: string
  license?: string | null
  compatibility?: string | null
  metadata: Record<string, string>
  allowed_tools?: string | null
  file_path: string
  content: string
  base_dir: string
  disable_model_invocation: boolean
  scope: string
}

interface ToolItem {
  name: string
  description: string
}

type EditorTab = "manifest" | "content"

export default function SkillEditor() {
  const [skills, setSkills] = useState<SkillEntry[]>([])
  const [tools, setTools] = useState<ToolItem[]>([])
  const [selectedIdx, setSelectedIdx] = useState<number | null>(null)
  const [activeTab, setActiveTab] = useState<EditorTab>("manifest")
  const [saving, setSaving] = useState(false)

  // Editable fields
  const [editName, setEditName] = useState("")
  const [editDesc, setEditDesc] = useState("")
  const [editContent, setEditContent] = useState("")
  const [editLicense, setEditLicense] = useState("")
  const [editCompat, setEditCompat] = useState("")
  const [editAllowedTools, setEditAllowedTools] = useState("")
  const [isNew, setIsNew] = useState(false)

  const loadSkills = async () => {
    try {
      const data = await fetchJson<SkillEntry[]>("/api/skills")
      setSkills(data)
    } catch {
      setSkills([])
    }
  }

  const loadTools = async () => {
    try {
      const data = await fetchJson<ToolItem[]>("/api/tools")
      setTools(data)
    } catch {
      setTools([])
    }
  }

  // Mount-time initialization — data must be fetched before user interacts

  useEffect(() => {
    loadSkills()
    loadTools()
  }, [])

  const selectSkill = (idx: number) => {
    setSelectedIdx(idx)
    setIsNew(false)
    const s = skills[idx]
    setEditName(s.name)
    setEditDesc(s.description)
    setEditContent(s.content || "")
    setEditLicense(s.license || "")
    setEditCompat(s.compatibility || "")
    setEditAllowedTools(s.allowed_tools || "")
    setActiveTab("manifest")
  }

  const startNew = () => {
    setSelectedIdx(null)
    setIsNew(true)
    setEditName("")
    setEditDesc("")
    setEditContent("")
    setEditLicense("")
    setEditCompat("")
    setEditAllowedTools("")
    setActiveTab("manifest")
  }

  const saveSkill = async () => {
    if (!editName.trim() || !editDesc.trim()) return
    setSaving(true)
    try {
      await fetchJson("/api/skills", {
        method: "POST",
        body: JSON.stringify({
          name: editName.trim(),
          description: editDesc.trim(),
          content: editContent,
          license: editLicense.trim() || null,
          compatibility: editCompat.trim() || null,
          "allowed-tools": editAllowedTools.trim() || null,
          metadata: {},
        }),
      })
      await loadSkills()
      setIsNew(false)
      // Select the saved skill
      const updated = await fetchJson<SkillEntry[]>("/api/skills")
      const idx = updated.findIndex((s) => s.name === editName.trim())
      if (idx >= 0) {
        setSelectedIdx(idx)
        setSkills(updated)
      }
    } catch {
      // silently fail
    } finally {
      setSaving(false)
    }
  }

  const deleteSkill = async (name: string) => {
    if (!confirm(`确定删除技能「${name}」？`)) return
    try {
      await fetchJson(`/api/skills/${encodeURIComponent(name)}`, { method: "DELETE" })
      setSelectedIdx(null)
      await loadSkills()
    } catch {
      // silently fail
    }
  }

  const selected = selectedIdx !== null ? skills[selectedIdx] : null

  return (
    <div className="flex h-full flex-col">
      {/* Header */}
      <div className="border-hairline flex items-center justify-between border-b px-4 py-2">
        <div className="flex items-center gap-2">
          <span className="text-sm font-medium text-white">技能管理</span>
          <span className="text-ink-muted text-xs">{skills.length} 个已安装</span>
        </div>
        <Button size="sm" variant="outline" onClick={startNew}>
          + 新建
        </Button>
      </div>

      <div className="flex min-h-0 flex-1">
        {/* Left: skill list */}
        <div className="border-hairline flex w-[220px] shrink-0 flex-col border-r">
          <div className="border-hairline border-b px-3 py-2">
            <input
              className="bg-bg-hover text-ink-muted w-full rounded-md border-0 px-2 py-1.5 text-xs outline-none placeholder:text-white/20"
              placeholder="搜索技能..."
              readOnly
            />
          </div>
          <ScrollArea className="flex-1">
            {skills.length === 0 ? (
              <p className="text-ink-muted px-4 py-8 text-center text-xs">暂无已安装技能</p>
            ) : (
              skills.map((sk, i) => (
                <button
                  key={sk.name}
                  onClick={() => selectSkill(i)}
                  className={`w-full border-l-2 px-3 py-2.5 text-left transition-colors ${
                    selectedIdx === i
                      ? "border-ai bg-ai/5 text-white"
                      : "text-ink-muted hover:bg-bg-hover border-transparent hover:text-white"
                  }`}
                >
                  <div className="flex items-center justify-between">
                    <span className="truncate text-xs font-medium">{sk.name}</span>
                    <span
                      className={`rounded-full px-1.5 py-[1px] text-[9px] ${
                        sk.scope === "project" ? "bg-ai/10 text-ai" : "bg-white/5 text-white/40"
                      }`}
                    >
                      {sk.scope === "project" ? "项目" : "用户"}
                    </span>
                  </div>
                  <div className="text-ink-muted mt-0.5 truncate text-[10px]">{sk.description}</div>
                </button>
              ))
            )}
          </ScrollArea>
        </div>

        {/* Right: editor */}
        <div className="flex min-w-0 flex-1 flex-col">
          {!selected && !isNew ? (
            <div className="text-ink-muted flex flex-1 flex-col items-center justify-center gap-2">
              <svg
                className="size-8 opacity-20"
                fill="none"
                stroke="currentColor"
                strokeWidth={1.5}
                viewBox="0 0 24 24"
              >
                <path
                  strokeLinecap="round"
                  strokeLinejoin="round"
                  d="m16.862 4.487 1.687-1.688a1.875 1.875 0 1 1 2.652 2.652L10.582 16.07a4.5 4.5 0 0 1-1.897 1.13L6 18l.8-2.685a4.5 4.5 0 0 1 1.13-1.897l8.932-8.931Zm0 0L19.5 7.125M18 14v4.75A2.25 2.25 0 0 1 15.75 21H5.25A2.25 2.25 0 0 1 3 18.75V8.25A2.25 2.25 0 0 1 5.25 6H10"
                />
              </svg>
              <span className="text-xs">选择一个技能开始编辑，或新建一个</span>
            </div>
          ) : (
            <>
              <div className="border-hairline flex items-center justify-between border-b px-4 py-2">
                <div className="flex items-center gap-1">
                  {(["manifest", "content"] as EditorTab[]).map((tab) => (
                    <button
                      key={tab}
                      onClick={() => setActiveTab(tab)}
                      className={`rounded-md px-3 py-1.5 text-xs font-medium transition-colors ${
                        activeTab === tab ? "bg-ai/10 text-ai" : "text-ink-muted hover:text-white"
                      }`}
                    >
                      {tab === "manifest" ? "配置" : "内容"}
                    </button>
                  ))}
                </div>
                <div className="flex items-center gap-2">
                  {selected && (
                    <Button
                      variant="outline"
                      size="sm"
                      className="border-down/30 hover:bg-down/10 text-down h-7 text-[10px]"
                      onClick={() => deleteSkill(selected.name)}
                    >
                      删除
                    </Button>
                  )}
                  <Button size="sm" className="h-7 text-xs" onClick={saveSkill} disabled={saving}>
                    {saving ? "保存中..." : "保存"}
                  </Button>
                </div>
              </div>

              <div className="flex min-h-0 flex-1">
                {activeTab === "manifest" ? (
                  <ScrollArea className="flex-1 p-4">
                    <Card size="sm" className="border-hairline border ring-0">
                      <CardHeader>
                        <CardTitle className="text-xs font-medium">基本信息</CardTitle>
                      </CardHeader>
                      <CardContent className="space-y-3">
                        <div>
                          <label className="text-ink-muted mb-1 block text-xs">name</label>
                          <Input
                            value={editName}
                            onChange={(e) => setEditName(e.target.value)}
                            className="h-7 font-mono text-xs"
                            placeholder="my-skill"
                            disabled={!!selected}
                          />
                          {selected && (
                            <p className="text-ink-muted mt-0.5 text-[10px]">
                              技能名称不可修改（如需要，请删除后重新创建）
                            </p>
                          )}
                        </div>
                        <div>
                          <label className="text-ink-muted mb-1 block text-xs">description</label>
                          <Textarea
                            value={editDesc}
                            onChange={(e) => setEditDesc(e.target.value)}
                            className="text-xs"
                            rows={2}
                            placeholder="简要描述技能的功能..."
                          />
                        </div>
                      </CardContent>
                    </Card>

                    <Card size="sm" className="border-hairline mt-3 border ring-0">
                      <CardHeader>
                        <CardTitle className="text-xs font-medium">元数据</CardTitle>
                      </CardHeader>
                      <CardContent className="space-y-3">
                        <div className="grid grid-cols-2 gap-3">
                          <div>
                            <label className="text-ink-muted mb-1 block text-xs">license</label>
                            <Input
                              value={editLicense}
                              onChange={(e) => setEditLicense(e.target.value)}
                              className="h-7 font-mono text-xs"
                              placeholder="可选"
                            />
                          </div>
                          <div>
                            <label className="text-ink-muted mb-1 block text-xs">
                              compatibility
                            </label>
                            <Input
                              value={editCompat}
                              onChange={(e) => setEditCompat(e.target.value)}
                              className="h-7 font-mono text-xs"
                              placeholder="可选"
                            />
                          </div>
                        </div>
                      </CardContent>
                    </Card>

                    <Card size="sm" className="border-hairline mt-3 border ring-0">
                      <CardHeader>
                        <CardTitle className="text-xs font-medium">工具权限</CardTitle>
                      </CardHeader>
                      <CardContent className="space-y-3">
                        <div>
                          <label className="text-ink-muted mb-1 block text-xs">allowed-tools</label>
                          <Input
                            value={editAllowedTools}
                            onChange={(e) => setEditAllowedTools(e.target.value)}
                            className="h-7 font-mono text-xs"
                            placeholder="tool1 tool2 (空格分隔)"
                          />
                        </div>
                        {tools.length > 0 && (
                          <div>
                            <label className="text-ink-muted mb-1 block text-xs">
                              可用工具 ({tools.length})
                            </label>
                            <div className="max-h-40 space-y-1 overflow-y-auto rounded-md border border-white/5 p-2">
                              {tools.map((tool) => (
                                <label
                                  key={tool.name}
                                  className="text-ink-muted flex cursor-pointer items-center gap-2 rounded px-2 py-1 text-xs hover:bg-white/5 hover:text-white"
                                >
                                  <Checkbox
                                    checked={editAllowedTools.split(/\s+/).includes(tool.name)}
                                    onCheckedChange={(checked) => {
                                      const current = editAllowedTools.split(/\s+/).filter(Boolean)
                                      const updated = checked
                                        ? [...current, tool.name]
                                        : current.filter((t) => t !== tool.name)
                                      setEditAllowedTools(updated.join(" "))
                                    }}
                                  />
                                  <span className="font-mono">{tool.name}</span>
                                  <span className="text-ink-muted">— {tool.description}</span>
                                </label>
                              ))}
                            </div>
                          </div>
                        )}
                      </CardContent>
                    </Card>
                  </ScrollArea>
                ) : (
                  <div className="flex min-h-0 flex-1 p-4">
                    <div className="flex flex-1 flex-col">
                      <label className="text-ink-muted mb-1 block text-xs">指令（Markdown）</label>
                      <Textarea
                        value={editContent}
                        onChange={(e) => setEditContent(e.target.value)}
                        className="flex-1 resize-none font-mono text-xs"
                        placeholder="## 角色&#10;&#10;你是一个...&#10;"
                      />
                    </div>
                  </div>
                )}
              </div>
            </>
          )}
        </div>
      </div>
    </div>
  )
}
