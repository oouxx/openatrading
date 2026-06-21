import { useState } from "react";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { ScrollArea } from "@/components/ui/scroll-area";
import { Separator } from "@/components/ui/separator";
import { Search, Plus, Settings, Puzzle } from "lucide-react";
import { useAppMode } from "@/contexts/AppModeContext";
import { usePanelWidth } from "@/hooks/usePanelWidth";
import { useConversations } from "@/hooks/useConversations";

const PRESET_SKILLS = [
  { name: "market-analysis", label: "市场分析" },
  { name: "dragon-tiger-tracking", label: "龙虎榜追踪" },
  { name: "breakout-analysis", label: "涨停晋级" },
  { name: "market-replay", label: "行情回放" },
  { name: "strategy-manager", label: "策略管理" },
  { name: "free-chat", label: "自由对话" },
];

export default function ConversationSidebar() {
  const { mode, setMode } = useAppMode();
  const { displayWidth, collapsed, toggleCollapse } = usePanelWidth(
    "layout:left-width",
    240,
    180,
    360,
    "layout:left-collapsed",
  );
  const { conversations, selectedId, selectConversation, search, setSearch } =
    useConversations();
  const [showSkills, setShowSkills] = useState(false);

  if (collapsed) {
    return (
      <div className="flex flex-col items-center py-3 gap-4 w-12 border-r border-hairline bg-bg-surface flex-shrink-0">
        <Button variant="ghost" size="icon" onClick={toggleCollapse} className="w-8 h-8">
          <Plus className="w-4 h-4" />
        </Button>
        <Button variant="ghost" size="icon" onClick={() => setMode("skill-editor")} className="w-8 h-8">
          <Puzzle className="w-4 h-4" />
        </Button>
        <div className="flex-1" />
        <Button variant="ghost" size="icon" onClick={() => setMode("settings")} className="w-8 h-8">
          <Settings className="w-4 h-4" />
        </Button>
      </div>
    );
  }

  return (
    <div
      className="flex flex-col border-r border-hairline bg-bg-surface flex-shrink-0 overflow-hidden"
      style={{ width: displayWidth || 240, minWidth: 0 }}
    >
      {/* New conversation + collapse toggle */}
      <div className="flex items-center gap-1 px-3 py-2.5 border-b border-hairline">
        <div className="relative flex-1">
          <Button
            variant="default"
            size="sm"
            className="w-full justify-start gap-2 text-sm h-9"
            onClick={() => setShowSkills(!showSkills)}
          >
            <Plus className="w-4 h-4" />
            新建会话
          </Button>
          {showSkills && (
            <div className="absolute top-full left-0 right-0 mt-1 z-50 bg-bg-surface border border-hairline rounded-lg shadow-lg py-1">
              {PRESET_SKILLS.map((skill) => (
                <button
                  key={skill.name}
                  className="w-full text-left px-4 py-2 text-sm text-white hover:bg-bg-hover transition-colors"
                  onClick={() => setShowSkills(false)}
                >
                  {skill.label}
                </button>
              ))}
            </div>
          )}
        </div>
        <Button variant="ghost" size="icon" onClick={toggleCollapse} className="w-8 h-8 flex-shrink-0">
          <span className="block w-1 h-1 rounded-full bg-ink-muted shadow-[0_4px_0_0_var(--color-muted),0_8px_0_0_var(--color-muted)]" />
        </Button>
      </div>

      {/* Skills entry */}
      <button
        onClick={() => setMode(mode === "skill-editor" ? "chat" : "skill-editor")}
        className="flex items-center gap-2 px-4 py-2 text-sm text-ink-muted hover:text-white hover:bg-bg-hover transition-colors border-b border-hairline"
      >
        <Puzzle className="w-4 h-4" />
        技能
      </button>

      {/* Search */}
      <div className="px-3 py-2">
        <div className="relative">
          <Search className="absolute left-2.5 top-1/2 -translate-y-1/2 w-3.5 h-3.5 text-ink-muted" />
          <Input
            placeholder="搜索会话..."
            value={search}
            onChange={(e) => setSearch(e.target.value)}
            className="pl-8 h-8 text-xs"
          />
        </div>
      </div>

      <Separator />

      {/* Session List */}
      <ScrollArea className="flex-1 px-2 py-1">
        {conversations.length === 0 ? (
          <div className="py-8 text-center text-xs text-ink-muted">暂无会话</div>
        ) : (
          conversations.map((c) => (
            <button
              key={c.id}
              onClick={() => selectConversation(c.id)}
              className={`w-full text-left px-3 py-2 rounded-lg text-sm transition-colors ${
                selectedId === c.id
                  ? "bg-ai/10 text-white"
                  : "text-ink-muted hover:text-white hover:bg-bg-hover"
              }`}
            >
              <div className="truncate">{c.title}</div>
            </button>
          ))
        )}
      </ScrollArea>

      {/* Settings at bottom */}
      <Separator />
      <button
        onClick={() => setMode(mode === "settings" ? "chat" : "settings")}
        className="flex items-center gap-2 px-4 py-2.5 text-sm text-ink-muted hover:text-white hover:bg-bg-hover transition-colors"
      >
        <Settings className="w-4 h-4" />
        设置
      </button>
    </div>
  );
}
