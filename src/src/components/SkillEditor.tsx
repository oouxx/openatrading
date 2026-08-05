import { useState, useEffect } from "react"
import { Input } from "@/components/ui/input"
import { Textarea } from "@/components/ui/textarea"
import { Button } from "@/components/ui/button"
import { ScrollArea } from "@/components/ui/scroll-area"
import { Card, CardContent, CardHeader, CardTitle } from "@/components/ui/card"
import { fetchJson } from "@/stores/data"

interface SkillEntry {
  name: string
  description: string
  file_path: string
  base_dir: string
  disable_model_invocation: boolean
  /** "user" | "project" | "temporary" */
  scope: string
  /** Markdown body after frontmatter. */
  content: string
}

type EditorTab = "manifest" | "content"

export default function SkillEditor() {
  const [skills, setSkills] = useState<SkillEntry[]>([])
  const [selectedIdx, setSelectedIdx] = useState<number | null>(null)
  const [activeTab, setActiveTab] = useState<EditorTab>("manifest")
  const [saving, setSaving] = useState(false)

  // Editable fields
  const [editName, setEditName] = useState("")
  const [editDesc, setEditDesc] = useState("")
  const [editContent, setEditContent] = useState("")
  const [editDisable, setEditDisable] = useState(false)
  const [isNew, setIsNew] = useState(false)

  const loadSkills = async () => {
    try {
      const data = await fetchJson<SkillEntry[]>("/api/skills")
      setSkills(data)
    } catch {
      setSkills([])
    }
  }

  // Mount-time initialization — data must be fetched before user interacts

  useEffect(() => {
    /* eslint-disable react-hooks/set-state-in-effect */
    loadSkills()
    /* eslint-enable react-hooks/set-state-in-effect */
  }, [])

  const selectSkill = (idx: number) => {
    setSelectedIdx(idx)
    setIsNew(false)
    const s = skills[idx]
    setEditName(s.name)
    setEditDesc(s.description)
    setEditContent(s.content || "")
    setEditDisable(!!s.disable_model_invocation)
    setActiveTab("manifest")
  }

  const startNew = () => {
    setSelectedIdx(null)
    setIsNew(true)
    setEditName("")
    setEditDesc("")
    setEditContent("")
    setEditDisable(false)
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
          "disable-model-invocation": editDisable,
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
          <span className="text-body font-medium text-primary">技能管理</span>
          <span className="text-secondary text-caption">{skills.length} 个已安装</span>
        </div>
        <Button size="sm" variant="outline" onClick={startNew}>
          + 新建
        </Button>
      </div>

      <div className="flex min-h-0 flex-1">
        {/* Left: skill list */}
        <div className="border-hairline hidden w-sidebar-narrow shrink-0 flex-col border-r md:flex">
          <div className="border-hairline border-b px-3 py-2">
            <input
              className="bg-hover text-secondary w-full rounded-md border-0 px-2 py-2 text-caption outline-none placeholder:text-primary/20"
              placeholder="搜索技能..."
              readOnly
            />
          </div>
          <ScrollArea className="flex-1">
            {skills.length === 0 ? (
              <p className="text-secondary px-4 py-8 text-center text-caption">暂无已安装技能</p>
            ) : (
              skills.map((sk, i) => (
                <button
                  key={sk.name}
                  onClick={() => selectSkill(i)}
                  className={`w-full border-l-2 px-3 py-3 text-left transition-colors ${
                    selectedIdx === i
                      ? "border-brand bg-brand/5 text-primary"
                      : "text-secondary hover:bg-hover border-transparent hover:text-primary"
                  }`}
                >
                  <div className="flex items-center justify-between">
                    <span className="truncate text-caption font-medium">{sk.name}</span>
                    <span
                      className={`rounded-full px-2 py-[1px] text-caption ${
                        sk.scope === "project"
                          ? "bg-brand/10 text-brand"
                          : "bg-white/5 text-primary/40"
                      }`}
                    >
                      {sk.scope === "project" ? "项目" : sk.scope === "temporary" ? "临时" : "用户"}
                    </span>
                  </div>
                  <div className="text-secondary mt-0.5 truncate text-caption">{sk.description}</div>
                </button>
              ))
            )}
          </ScrollArea>
        </div>

        {/* Right: editor */}
        <div className="readable-content flex min-w-0 flex-1 flex-col">
          {!selected && !isNew ? (
            <div className="text-secondary flex flex-1 flex-col items-center justify-center gap-2">
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
              <span className="text-caption">选择一个技能开始编辑，或新建一个</span>
            </div>
          ) : (
            <>
              <div className="border-hairline flex items-center justify-between border-b px-4 py-2">
                <div className="flex items-center gap-1">
                  {(["manifest", "content"] as EditorTab[]).map((tab) => (
                    <button
                      key={tab}
                      onClick={() => setActiveTab(tab)}
                      className={`rounded-md px-3 py-2 text-caption font-medium transition-colors ${
                        activeTab === tab ? "bg-brand/10 text-brand" : "text-secondary hover:text-primary"
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
                      className="border-down/30 hover:bg-down/10 text-down h-7 text-caption"
                      onClick={() => deleteSkill(selected.name)}
                    >
                      删除
                    </Button>
                  )}
                  <Button size="sm" className="h-7 text-caption" onClick={saveSkill} disabled={saving}>
                    {saving ? "保存中..." : "保存"}
                  </Button>
                </div>
              </div>

              <div className="flex min-h-0 flex-1">
                {activeTab === "manifest" ? (
                  <ScrollArea className="flex-1 p-4">
                    <Card size="sm" className="border-hairline border ring-0">
                      <CardHeader>
                        <CardTitle className="text-caption font-medium">基本信息</CardTitle>
                      </CardHeader>
                      <CardContent className="space-y-3">
                        <div>
                          <label className="text-secondary mb-1 block text-caption">name</label>
                          <Input
                            value={editName}
                            onChange={(e) => setEditName(e.target.value)}
                            className="h-7 font-number text-caption"
                            placeholder="my-skill"
                            disabled={!!selected}
                          />
                          {selected && (
                            <p className="text-secondary mt-0.5 text-caption">
                              技能名称不可修改（如需要，请删除后重新创建）
                            </p>
                          )}
                        </div>
                        <div>
                          <label className="text-secondary mb-1 block text-caption">description</label>
                          <Textarea
                            value={editDesc}
                            onChange={(e) => setEditDesc(e.target.value)}
                            className="text-caption"
                            rows={2}
                            placeholder="简要描述技能的功能..."
                          />
                        </div>
                        <label className="text-secondary flex cursor-pointer items-center gap-2 text-caption">
                          <input
                            type="checkbox"
                            checked={editDisable}
                            onChange={(e) => setEditDisable(e.target.checked)}
                            className="size-3.5"
                          />
                          disable-model-invocation（禁止模型自动调用，仅 /skill 显式触发）
                        </label>
                      </CardContent>
                    </Card>
                  </ScrollArea>
                ) : (
                  <div className="flex min-h-0 flex-1 p-4">
                    <div className="flex flex-1 flex-col">
                      <label className="text-secondary mb-1 block text-caption">指令（Markdown）</label>
                      <Textarea
                        value={editContent}
                        onChange={(e) => setEditContent(e.target.value)}
                        className="flex-1 resize-none font-number text-caption"
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
