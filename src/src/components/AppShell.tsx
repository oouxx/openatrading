// src/components/AppShell.tsx
import { useState, useCallback } from "react";
import { useAppMode } from "@/contexts/AppModeContext";
import AiCopilot from "@/AiCopilot";
import ConversationSidebar from "./ConversationSidebar";
import { useThreads } from "@/hooks/useChatThreads";
import { useStockTabStore } from "@/stores/stockTabStore";
import RightPanel from "./RightPanel";
import SkillEditor from "./SkillEditor";
import SettingsView from "./SettingsView";
import SettingsSidebar from "./SettingsSidebar";
import type { SettingsTab } from "./SettingsSidebar";
import {
  SidebarInset,
  SidebarProvider,
  SidebarTrigger,
} from "@/components/ui/sidebar";
import { TooltipProvider } from "@/components/ui/tooltip";

export default function AppShell() {
  const { mode } = useAppMode();
  const [settingsTab, setSettingsTab] = useState<SettingsTab>("appearance");
  const activeThreadId = useStockTabStore((s) => s.activeThreadId);
  const setActiveThreadId = useStockTabStore((s) => s.setActiveThreadId);
  const { threads, createThread, deleteThread, reload } = useThreads();

  const handleNewThread = async () => {
    const thread = await createThread("新对话");
    setActiveThreadId(thread.id);
  };

  const handleDeleteThread = useCallback(async (id: number) => {
    await deleteThread(id);
    if (activeThreadId === id) setActiveThreadId(null);
    reload();
  }, [activeThreadId, deleteThread, reload]);

  const handleActiveChange = useCallback((id: number | null) => {
    setActiveThreadId(id);
    useStockTabStore.getState().setActiveStockTab(null);
    reload();
  }, [reload]);

  return (
    <TooltipProvider>
      <SidebarProvider defaultOpen>
        {mode === "settings" ? (
          <SettingsSidebar activeTab={settingsTab} onTabChange={setSettingsTab} />
        ) : (
          <ConversationSidebar
            threads={threads}
            activeThreadId={activeThreadId}
            onSelectThread={handleActiveChange}
            onNewThread={handleNewThread}
            onDeleteThread={handleDeleteThread}
          />
        )}
        <SidebarInset className="overflow-hidden">
          {mode === "chat" ? (
            <div className="flex flex-row flex-1 min-w-0 h-full">
              <div className="flex-1 min-w-0 flex flex-col">
                <AiCopilot
                  activeThreadId={activeThreadId}
                  onActiveThreadChange={handleActiveChange}
                  onReload={reload}
                />
              </div>
              <RightPanel />
            </div>
          ) : mode === "skill-editor" ? (
            <div className="flex flex-col flex-1 min-w-0 h-full">
              <div className="flex items-center gap-2 px-3 py-2 border-b border-hairline">
                <SidebarTrigger />
              </div>
              <div className="flex-1 overflow-y-auto">
                <SkillEditor />
              </div>
            </div>
          ) : (
            <div className="flex flex-col flex-1 min-w-0 h-full">
              <div className="flex items-center gap-2 px-3 py-2 border-b border-hairline">
                <SidebarTrigger />
              </div>
              <div className="flex-1 overflow-y-auto">
                <SettingsView activeTab={settingsTab} />
              </div>
            </div>
          )}
        </SidebarInset>
      </SidebarProvider>
    </TooltipProvider>
  );
}
