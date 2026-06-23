import { useState, useEffect, useCallback } from "react";
import { ChevronLeft, ChevronRight } from "lucide-react";
import { Button } from "@/components/ui/button";
import { usePanelWidth } from "@/hooks/usePanelWidth";
import ResizableHandle from "./ResizableHandle";
import DragonTigerView from "@/DragonTigerView";
import BreakoutView from "@/BreakoutView";
import ReplayView from "@/ReplayView";
import RoleView from "@/RoleView";

type ViewId = "dragon-tiger" | "breakout" | "replay" | "roles";

const TABS: { id: ViewId; label: string }[] = [
  { id: "dragon-tiger", label: "龙虎榜" },
  { id: "breakout", label: "晋级分析" },
  { id: "replay", label: "行情回放" },
  { id: "roles", label: "角色策略" },
];

export default function RightPanel() {
  const [activeView, setActiveView] = useState<ViewId>("dragon-tiger");
  const { width, setWidth, collapsed, toggleCollapse } = usePanelWidth(
    "layout:right-width", 400, 300, 700, "layout:right-collapsed"
  );

  useEffect(() => {
    const handler = (e: Event) => {
      const detail = (e as CustomEvent).detail as ViewId;
      if (collapsed) toggleCollapse();
      setActiveView(detail);
    };
    window.addEventListener("open-right-panel", handler);
    return () => window.removeEventListener("open-right-panel", handler);
  }, [collapsed, toggleCollapse]);

  const handleResize = useCallback(
    (delta: number) => setWidth((prev) => prev + delta),
    [setWidth]
  );

  const renderView = () => {
    switch (activeView) {
      case "dragon-tiger": return <DragonTigerView />;
      case "breakout": return <BreakoutView />;
      case "replay": return <ReplayView />;
      case "roles": return <RoleView />;
    }
  };

  return (
    <div className="flex flex-shrink-0">
      {!collapsed && <ResizableHandle onResize={handleResize} />}
      <div
        className="flex flex-col bg-bg-surface border-l border-hairline overflow-hidden transition-[width] duration-150"
        style={{ width: collapsed ? 48 : width }}
      >
        <div className="flex items-center gap-1 px-2 py-2 border-b border-hairline flex-shrink-0">
          <Button
            variant="ghost"
            size="icon"
            className="w-6 h-6"
            onClick={toggleCollapse}
            title={collapsed ? "展开面板" : "折叠面板"}
          >
            {collapsed ? (
              <ChevronLeft className="w-3 h-3" />
            ) : (
              <ChevronRight className="w-3 h-3" />
            )}
          </Button>
          {!collapsed && (
            <span className="text-xs font-medium text-white flex-1 truncate">
              {TABS.find((t) => t.id === activeView)?.label}
            </span>
          )}
        </div>

        {collapsed ? (
          <div className="flex flex-col items-center gap-1 py-2 flex-1 border-t border-hairline overflow-y-auto">
            {TABS.map((tab) => (
              <button
                key={tab.id}
                className="w-10 h-10 flex items-center justify-center text-[10px] rounded-card hover:bg-bg-hover transition-colors data-[active=true]:text-ai"
                data-active={activeView === tab.id}
                title={tab.label}
                onClick={() => {
                  setActiveView(tab.id);
                  toggleCollapse();
                }}
              >
                {tab.label.slice(0, 2)}
              </button>
            ))}
          </div>
        ) : (
          <>
            <div className="flex gap-0 px-3 border-b border-hairline flex-shrink-0">
              {TABS.map((tab) => (
                <button
                  key={tab.id}
                  className="px-3 py-1.5 text-xs transition-colors border-b-2 whitespace-nowrap data-[active=true]:text-ai data-[active=true]:border-ai data-[active=false]:text-ink-muted data-[active=false]:border-transparent data-[active=false]:hover:text-white"
                  data-active={activeView === tab.id}
                  onClick={() => setActiveView(tab.id)}
                >
                  {tab.label}
                </button>
              ))}
            </div>

            <div className="flex-1 overflow-y-auto">
              {renderView()}
            </div>
          </>
        )}
      </div>
    </div>
  );
}
