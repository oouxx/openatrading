import { Button } from "@/components/ui/button";
import { ScrollArea } from "@/components/ui/scroll-area";
import { ArrowLeft, Palette, Plug, Info } from "lucide-react";
import { useAppMode } from "@/contexts/AppModeContext";

type SettingsTab = "appearance" | "mcp" | "about";

const TABS: { id: SettingsTab; label: string; icon: typeof Palette }[] = [
  { id: "appearance", label: "外观", icon: Palette },
  { id: "mcp", label: "MCP", icon: Plug },
  { id: "about", label: "关于", icon: Info },
];

interface SettingsSidebarProps {
  activeTab: SettingsTab;
  onTabChange: (tab: SettingsTab) => void;
}

export default function SettingsSidebar({ activeTab, onTabChange }: SettingsSidebarProps) {
  const { setMode } = useAppMode();

  return (
    <div className="flex flex-col w-[180px] border-r border-hairline bg-bg-surface flex-shrink-0 overflow-hidden">
      <div className="px-3 py-2.5 border-b border-hairline">
        <Button
          variant="ghost"
          size="sm"
          className="w-full justify-start gap-2 text-xs text-ink-muted hover:text-white"
          onClick={() => setMode("chat")}
        >
          <ArrowLeft className="w-3.5 h-3.5" />
          返回
        </Button>
      </div>

      <ScrollArea className="flex-1">
        <div className="px-2 py-2 flex flex-col gap-0.5">
          {TABS.map((tab) => (
            <button
              key={tab.id}
              onClick={() => onTabChange(tab.id)}
              className={`flex items-center gap-2 px-3 py-2 text-xs rounded-lg transition-colors text-left w-full ${
                activeTab === tab.id
                  ? "bg-ai/10 text-ai border-r-2 border-ai"
                  : "text-ink-muted hover:text-white hover:bg-bg-hover"
              }`}
            >
              <tab.icon className="w-3.5 h-3.5" />
              {tab.label}
            </button>
          ))}
        </div>
      </ScrollArea>
    </div>
  );
}

export type { SettingsTab };
