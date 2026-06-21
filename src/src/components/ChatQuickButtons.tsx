import { Button } from "@/components/ui/button";

export default function ChatQuickButtons() {
  const openView = (viewId: string) => {
    window.dispatchEvent(
      new CustomEvent("open-right-panel", { detail: viewId }),
    );
  };

  return (
    <div className="flex items-center gap-2 px-4 py-2 border-t border-hairline">
      <Button variant="ghost" size="sm" className="text-xs text-ink-muted hover:text-white h-8 px-3" onClick={() => openView("dragon-tiger")}>
        📈 龙虎榜
      </Button>
      <Button variant="ghost" size="sm" className="text-xs text-ink-muted hover:text-white h-8 px-3" onClick={() => openView("breakout")}>
        🔥 晋级分析
      </Button>
      <Button variant="ghost" size="sm" className="text-xs text-ink-muted hover:text-white h-8 px-3" onClick={() => openView("replay")}>
        🎯 行情回放
      </Button>
      <Button variant="ghost" size="sm" className="text-xs text-ink-muted hover:text-white h-8 px-3" onClick={() => openView("roles")}>
        🤖 角色策略
      </Button>
    </div>
  );
}
