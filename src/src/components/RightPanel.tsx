import { useState, useEffect, useCallback } from "react";
import { Button } from "@/components/ui/button";
import { X } from "lucide-react";
import { usePanelWidth } from "@/hooks/usePanelWidth";
import ResizableHandle from "./ResizableHandle";
import DragonTigerView from "@/DragonTigerView";
import BreakoutView from "@/BreakoutView";
import ReplayView from "@/ReplayView";
import RoleView from "@/RoleView";

type ViewId = "dragon-tiger" | "breakout" | "replay" | "roles";

const VIEW_TITLES: Record<string, string> = {
  "dragon-tiger": "龙虎榜",
  breakout: "晋级分析",
  replay: "行情回放",
  roles: "角色策略",
};

export default function RightPanel() {
  const [activeView, setActiveView] = useState<ViewId | null>(null);
  const { displayWidth, setWidth } =
    usePanelWidth("layout:right-width", 420, 320, 600, "layout:right-collapsed");

  useEffect(() => {
    const handler = (e: Event) => {
      const detail = (e as CustomEvent).detail as ViewId;
      setActiveView((prev) => (prev === detail ? null : detail));
    };
    window.addEventListener("open-right-panel", handler);
    return () => window.removeEventListener("open-right-panel", handler);
  }, []);

  const onResize = useCallback((delta: number) => {
    setWidth((w) => w + delta);
  }, [setWidth]);

  if (!activeView) return null;

  const renderView = () => {
    switch (activeView) {
      case "dragon-tiger": return <DragonTigerView />;
      case "breakout": return <BreakoutView />;
      case "replay": return <ReplayView />;
      case "roles": return <RoleView />;
    }
  };

  return (
    <>
      <ResizableHandle onResize={onResize} />
      <div
        className="flex flex-col bg-bg-surface border-l border-hairline overflow-hidden flex-shrink-0"
        style={{ width: displayWidth || 420 }}
      >
        <div className="flex items-center justify-between px-4 py-2.5 border-b border-hairline flex-shrink-0">
          <span className="text-sm font-medium text-white">{VIEW_TITLES[activeView]}</span>
          <Button variant="ghost" size="icon" className="w-7 h-7" onClick={() => setActiveView(null)}>
            <X className="w-4 h-4" />
          </Button>
        </div>
        <div className="flex-1 overflow-y-auto">{renderView()}</div>
      </div>
    </>
  );
}
