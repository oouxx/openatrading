import { useState, useEffect } from "react";
import { Input } from "@/components/ui/input";
import { Textarea } from "@/components/ui/textarea";
import { Button } from "@/components/ui/button";
import { ScrollArea } from "@/components/ui/scroll-area";
import { Separator } from "@/components/ui/separator";
import { Checkbox } from "@/components/ui/checkbox";
import { useAppMode } from "@/contexts/AppModeContext";
import { fetchJson } from "@/stores/data";
import ReactMarkdown from "react-markdown";
import remarkGfm from "remark-gfm";

interface ToolItem {
  name: string;
  description: string;
}

const PRESET_SKILLS = [
  { name: "market-analysis", label: "📈 市场分析" },
  { name: "dragon-tiger-tracking", label: "🐉 龙虎榜追踪" },
  { name: "breakout-analysis", label: "🔥 涨停晋级" },
  { name: "market-replay", label: "🎯 行情回放" },
  { name: "strategy-manager", label: "🤖 策略管理" },
  { name: "free-chat", label: "⚡ 自由对话" },
];

export default function SkillEditor() {
  const { setMode } = useAppMode();
  const [selectedSkill, setSelectedSkill] = useState(PRESET_SKILLS[0].name);
  const [name, setName] = useState("dragon-tiger-tracking");
  const [description, setDescription] = useState(
    "龙虎榜分析 — 查询上榜个股、营业部买卖明细",
  );
  const [instructions, setInstructions] = useState(
    "## 角色\n\n你是一个龙虎榜分析专家...\n\n## 工具\n- dragon_tiger_detail\n- dragon_tiger_seat",
  );
  const [tools, setTools] = useState<ToolItem[]>([]);
  const [selectedTools, setSelectedTools] = useState<string[]>([]);

  useEffect(() => {
    fetchJson<ToolItem[]>("/api/tools")
      .then(setTools)
      .catch(() => {});
  }, []);

  const toggleTool = (toolName: string) => {
    setSelectedTools((prev) =>
      prev.includes(toolName)
        ? prev.filter((t) => t !== toolName)
        : [...prev, toolName],
    );
  };

  return (
    <div className="flex flex-col h-full p-6 gap-5 overflow-y-auto">
      <div className="flex items-center gap-2 flex-wrap">
        <span className="text-sm font-medium text-white mr-2">
          已安装 Skill
        </span>
        {PRESET_SKILLS.map((skill) => (
          <Button
            key={skill.name}
            variant={selectedSkill === skill.name ? "default" : "outline"}
            size="sm"
            onClick={() => setSelectedSkill(skill.name)}
          >
            {skill.label}
          </Button>
        ))}
        <Button variant="outline" size="sm">
          + 新建
        </Button>
      </div>

      <Separator />

      <div className="flex-1 grid grid-cols-1 lg:grid-cols-2 gap-5 min-h-0">
        {/* Left: Form */}
        <ScrollArea className="pr-3">
          <div className="space-y-4">
            <div>
              <label className="text-xs text-ink-muted block mb-1">name</label>
              <Input
                value={name}
                onChange={(e) => setName(e.target.value)}
                className="font-mono text-xs"
              />
            </div>
            <div>
              <label className="text-xs text-ink-muted block mb-1">
                description
              </label>
              <Textarea
                value={description}
                onChange={(e) => setDescription(e.target.value)}
                className="text-xs"
                rows={3}
              />
            </div>
            <div>
              <label className="text-xs text-ink-muted block mb-3">
                关联工具
              </label>
              <div className="space-y-2">
                {tools.map((tool) => (
                  <label
                    key={tool.name}
                    className="flex items-center gap-2 cursor-pointer text-xs text-ink-muted hover:text-white"
                  >
                    <Checkbox
                      checked={selectedTools.includes(tool.name)}
                      onCheckedChange={() => toggleTool(tool.name)}
                    />
                    <span className="font-mono">{tool.name}</span>
                    <span className="text-ink-muted">
                      {" "}
                      — {tool.description}
                    </span>
                  </label>
                ))}
                {tools.length === 0 && (
                  <p className="text-xs text-ink-muted">加载中...</p>
                )}
              </div>
            </div>
            <div>
              <label className="text-xs text-ink-muted block mb-1">
                指令（Markdown）
              </label>
              <Textarea
                value={instructions}
                onChange={(e) => setInstructions(e.target.value)}
                className="font-mono text-xs min-h-[300px]"
              />
            </div>
          </div>
        </ScrollArea>
        {/* Right: Preview */}
        <ScrollArea className="border border-hairline rounded-lg p-4 bg-bg">
          <div className="prose prose-invert prose-sm max-w-none">
            <ReactMarkdown remarkPlugins={[remarkGfm]}>
              {instructions}
            </ReactMarkdown>
          </div>
        </ScrollArea>
      </div>

      <div className="flex gap-2 justify-end pt-3 border-t border-hairline">
        <Button variant="outline" onClick={() => setMode("chat")}>
          返回
        </Button>
        <Button>保存 Skill</Button>
      </div>
    </div>
  );
}
