// src/components/AppShell.tsx
import { useMemo, useCallback, useState } from "react";
import {
  AssistantRuntimeProvider,
  useRemoteThreadListRuntime,
} from "@assistant-ui/react";
import { useAppMode } from "@/contexts/AppModeContext";
import { useAssistantChat } from "@/hooks/useAssistantChat";
import { createThreadListAdapter } from "@/hooks/useChatThreads";
import AiCopilotContent from "@/AiCopilot";
import ConversationSidebar from "./ConversationSidebar";
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

  const adapter = useMemo(() => createThreadListAdapter(), []);
  const { runtime } = useAssistantChat("");
  const runtimeHook = useCallback(() => runtime, [runtime]);
  const threadListRuntime = useRemoteThreadListRuntime({ adapter, runtimeHook });

  return (
    <AssistantRuntimeProvider runtime={threadListRuntime}>
      <TooltipProvider>
        <SidebarProvider defaultOpen>
          {mode === "settings" ? (
            <SettingsSidebar activeTab={settingsTab} onTabChange={setSettingsTab} />
          ) : (
            <ConversationSidebar />
          )}
          <SidebarInset>
            {mode === "chat" ? (
              <div className="flex flex-1 min-w-0 h-full">
                <div className="flex-1 min-w-0 flex flex-col">
                  <AiCopilotContent />
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
              <div className="flex-1 min-w-0 h-full overflow-y-auto">
                <SettingsView activeTab={settingsTab} />
              </div>
            )}
          </SidebarInset>
        </SidebarProvider>
      </TooltipProvider>
    </AssistantRuntimeProvider>
  );
}
