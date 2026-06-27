import { useState, useEffect } from "react";
import { Input } from "@/components/ui/input";
import { Textarea } from "@/components/ui/textarea";
import { Button } from "@/components/ui/button";
import { ScrollArea } from "@/components/ui/scroll-area";
import { Checkbox } from "@/components/ui/checkbox";
import { useAppMode } from "@/contexts/AppModeContext";
import { fetchJson } from "@/stores/data";
import ReactMarkdown from "react-markdown";
import remarkGfm from "remark-gfm";

interface SkillEntry {
  name: string;
  description: string;
  license?: string | null;
  compatibility?: string | null;
  metadata: Record<string, string>;
  allowed_tools?: string | null;
  file_path: string;
  content: string;
  base_dir: string;
  disable_model_invocation: boolean;
  scope: string;
}

interface ToolItem {
  name: string;
  description: string;
}

type EditorTab = "manifest" | "content";

export default function SkillEditor() {
  const { setMode } = useAppMode();
  const [skills, setSkills] = useState<SkillEntry[]>([]);
  const [tools, setTools] = useState<ToolItem[]>([]);
  const [selectedIdx, setSelectedIdx] = useState<number | null>(null);
  const [activeTab, setActiveTab] = useState<EditorTab>("manifest");
  const [saving, setSaving] = useState(false);

  // Editable fields
  const [editName, setEditName] = useState("");
  const [editDesc, setEditDesc] = useState("");
  const [editContent, setEditContent] = useState("");
  const [editLicense, setEditLicense] = useState("");
  const [editCompat, setEditCompat] = useState("");
  const [editAllowedTools, setEditAllowedTools] = useState("");
  const [isNew, setIsNew] = useState(false);

  const loadSkills = async () => {
    try {
      const data = await fetchJson<SkillEntry[]>("/api/skills");
      setSkills(data);
    } catch {
      setSkills([]);
    }
  };

  const loadTools = async () => {
    try {
      const data = await fetchJson<ToolItem[]>("/api/tools");
      setTools(data);
    } catch {
      setTools([]);
    }
  };

  // Mount-time initialization — data must be fetched before user interacts
  // eslint-disable-next-line react-hooks/set-state-in-effect
  useEffect(() => { loadSkills(); loadTools(); }, []);

  const selectSkill = (idx: number) => {
    setSelectedIdx(idx);
    setIsNew(false);
    const s = skills[idx];
    setEditName(s.name);
    setEditDesc(s.description);
    setEditContent(s.content || "");
    setEditLicense(s.license || "");
    setEditCompat(s.compatibility || "");
    setEditAllowedTools(s.allowed_tools || "");
    setActiveTab("manifest");
  };

  const startNew = () => {
    setSelectedIdx(null);
    setIsNew(true);
    setEditName("");
    setEditDesc("");
    setEditContent("");
    setEditLicense("");
    setEditCompat("");
    setEditAllowedTools("");
    setActiveTab("manifest");
  };

  const saveSkill = async () => {
    if (!editName.trim() || !editDesc.trim()) return;
    setSaving(true);
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
      });
      await loadSkills();
      setIsNew(false);
      // Select the saved skill
      const updated = await fetchJson<SkillEntry[]>("/api/skills");
      const idx = updated.findIndex((s) => s.name === editName.trim());
      if (idx >= 0) {
        setSelectedIdx(idx);
        setSkills(updated);
      }
    } catch {
      // silently fail
    } finally {
      setSaving(false);
    }
  };

  const deleteSkill = async (name: string) => {
    if (!confirm(`确定删除技能「${name}」？`)) return;
    try {
      await fetchJson(`/api/skills/${encodeURIComponent(name)}`, { method: "DELETE" });
      setSelectedIdx(null);
      await loadSkills();
    } catch {
      // silently fail
    }
  };

  const selected = selectedIdx !== null ? skills[selectedIdx] : null;

  return (
    <div className="flex h-full flex-col">
      <div className="flex items-center gap-2 border-b border-hairline px-4 py-2">
        <Button variant="ghost" size="sm" onClick={() => setMode("chat")}>
          ← 返回
        </Button>
        <span className="text-sm font-medium text-white">技能管理</span>
        <span className="text-xs text-ink-muted">{skills.length} 个已安装</span>
      </div>

      <div className="flex min-h-0 flex-1">
        {/* Left: skill list */}
        <div className="flex w-[220px] flex-shrink-0 flex-col border-r border-hairline">
          <div className="border-b border-hairline px-3 py-2">
            <button
              className="text-accent w-full rounded-md border border-hairline px-3 py-1.5 text-left text-xs transition-colors hover:bg-bg-hover"
              onClick={startNew}
            >
              + 新建技能
            </button>
          </div>
          <ScrollArea className="flex-1">
            {skills.length === 0 ? (
              <p className="px-4 py-8 text-center text-xs text-ink-muted">暂无已安装技能</p>
            ) : (
              skills.map((sk, i) => (
                <button
                  key={sk.name}
                  onClick={() => selectSkill(i)}
                  className={`w-full border-l-2 px-3 py-2 text-left transition-colors ${
                    selectedIdx === i
                      ? "border-ai bg-ai/5 text-white"
                      : "border-transparent text-ink-muted hover:bg-bg-hover hover:text-white"
                  }`}
                >
                  <div className="truncate text-xs font-medium">{sk.name}</div>
                  <div className="mt-0.5 truncate text-[10px] text-ink-muted">{sk.description}</div>
                  <div className="mt-0.5 text-[10px] text-ink-muted">
                    {sk.scope === "project" ? "项目" : "用户"}
                  </div>
                </button>
              ))
            )}
          </ScrollArea>
        </div>

        {/* Right: editor */}
        <div className="flex min-w-0 flex-1 flex-col">
          {!selected && !isNew ? (
            <div className="flex flex-1 items-center justify-center text-xs text-ink-muted">
              选择一个技能开始编辑，或新建一个
            </div>
          ) : (
            <>
              <div className="flex items-center justify-between border-b border-hairline px-4 py-2">
                <div className="flex items-center gap-1">
                  {(["manifest", "content"] as EditorTab[]).map((tab) => (
                    <button
                      key={tab}
                      onClick={() => setActiveTab(tab)}
                      className={`border-b-2 px-3 py-1 text-xs transition-colors ${
                        activeTab === tab
                          ? "border-ai text-ai"
                          : "border-transparent text-ink-muted hover:text-white"
                      }`}
                    >
                      {tab === "manifest" ? "配置" : "内容"}
                    </button>
                  ))}
                </div>
                <div className="flex gap-2">
                  {selected && (
                    <Button
                      variant="outline"
                      size="sm"
                      className="border-down/30 text-[10px] text-down hover:bg-down/10"
                      onClick={() => deleteSkill(selected.name)}
                    >
                      删除
                    </Button>
                  )}
                  <Button size="sm" onClick={saveSkill} disabled={saving}>
                    {saving ? "保存中..." : "保存"}
                  </Button>
                </div>
              </div>

              <div className="flex min-h-0 flex-1">
                {activeTab === "manifest" ? (
                  <ScrollArea className="flex-1 p-4">
                    <div className="max-w-lg space-y-4">
                      <div>
                        <label className="mb-1 block text-xs text-ink-muted">name</label>
                        <Input
                          value={editName}
                          onChange={(e) => setEditName(e.target.value)}
                          className="font-mono text-xs"
                          placeholder="my-skill"
                          disabled={!!selected}
                        />
                        {selected && (
                          <p className="mt-0.5 text-[10px] text-ink-muted">
                            技能名称不可修改（如需要，请删除后重新创建）
                          </p>
                        )}
                      </div>
                      <div>
                        <label className="mb-1 block text-xs text-ink-muted">description</label>
                        <Textarea
                          value={editDesc}
                          onChange={(e) => setEditDesc(e.target.value)}
                          className="text-xs"
                          rows={3}
                          placeholder="简要描述技能的功能..."
                        />
                      </div>
                      <div className="grid grid-cols-2 gap-3">
                        <div>
                          <label className="mb-1 block text-xs text-ink-muted">license</label>
                          <Input
                            value={editLicense}
                            onChange={(e) => setEditLicense(e.target.value)}
                            className="font-mono text-xs"
                            placeholder="可选"
                          />
                        </div>
                        <div>
                          <label className="mb-1 block text-xs text-ink-muted">compatibility</label>
                          <Input
                            value={editCompat}
                            onChange={(e) => setEditCompat(e.target.value)}
                            className="font-mono text-xs"
                            placeholder="可选"
                          />
                        </div>
                      </div>
                      <div>
                        <label className="mb-1 block text-xs text-ink-muted">allowed-tools</label>
                        <Input
                          value={editAllowedTools}
                          onChange={(e) => setEditAllowedTools(e.target.value)}
                          className="font-mono text-xs"
                          placeholder="tool1 tool2 (空格分隔)"
                        />
                      </div>
                      {tools.length > 0 && (
                        <div>
                          <label className="mb-2 block text-xs text-ink-muted">可用工具 ({tools.length})</label>
                          <div className="max-h-48 space-y-1.5 overflow-y-auto">
                            {tools.map((tool) => (
                              <label
                                key={tool.name}
                                className="flex cursor-pointer items-center gap-2 text-xs text-ink-muted hover:text-white"
                              >
                                <Checkbox
                                  checked={editAllowedTools.split(/\s+/).includes(tool.name)}
                                  onCheckedChange={(checked) => {
                                    const current = editAllowedTools.split(/\s+/).filter(Boolean);
                                    const updated = checked
                                      ? [...current, tool.name]
                                      : current.filter((t) => t !== tool.name);
                                    setEditAllowedTools(updated.join(" "));
                                  }}
                                />
                                <span className="font-mono">{tool.name}</span>
                                <span className="text-ink-muted">— {tool.description}</span>
                              </label>
                            ))}
                          </div>
                        </div>
                      )}
                    </div>
                  </ScrollArea>
                ) : (
                  <div className="flex min-h-0 flex-1">
                    <div className="flex-1 p-4">
                      <label className="mb-1 block text-xs text-ink-muted">
                        指令（Markdown）— 编辑后点击右侧预览
                      </label>
                      <Textarea
                        value={editContent}
                        onChange={(e) => setEditContent(e.target.value)}
                        className="h-full min-h-[300px] font-mono text-xs"
                        placeholder="## 角色&#10;&#10;你是一个...&#10;&#10;## 工具&#10;..."
                      />
                    </div>
                    <div className="w-[380px] flex-shrink-0 overflow-y-auto border-l border-hairline bg-bg p-4">
                      <p className="mb-2 text-[10px] tracking-wider text-ink-muted uppercase">预览</p>
                      <div className="prose prose-invert prose-sm max-w-none">
                        {editContent ? (
                          <ReactMarkdown remarkPlugins={[remarkGfm]}>
                            {editContent}
                          </ReactMarkdown>
                        ) : (
                          <p className="text-xs text-ink-muted">暂无内容</p>
                        )}
                      </div>
                    </div>
                  </div>
                )}
              </div>
            </>
          )}
        </div>
      </div>
    </div>
  );
}
