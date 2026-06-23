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

export default function AppShell() {
  const { mode } = useAppMode();
  const [settingsTab, setSettingsTab] = useState<SettingsTab>("appearance");

  const adapter = useMemo(() => createThreadListAdapter(), []);
  const { runtime } = useAssistantChat("");
  const runtimeHook = useCallback(() => runtime, [runtime]);
  const threadListRuntime = useRemoteThreadListRuntime({ adapter, runtimeHook });

  return (
    <AssistantRuntimeProvider runtime={threadListRuntime}>
      <div className="flex h-screen w-screen overflow-hidden bg-bg">
        <ConversationSidebar />

        <div className="flex flex-1 min-w-0">
          {mode === "chat" ? (
            <>
              <div className="flex-1 min-w-0 flex flex-col">
                <AiCopilotContent />
              </div>
              <RightPanel />
            </>
          ) : mode === "skill-editor" ? (
            <div className="flex-1 min-w-0 overflow-y-auto">
              <SkillEditor />
            </div>
          ) : (
            <div className="flex flex-1 min-w-0">
              <SettingsSidebar activeTab={settingsTab} onTabChange={setSettingsTab} />
              <SettingsView activeTab={settingsTab} />
            </div>
          )}
        </div>
      </div>
    </AssistantRuntimeProvider>
  );
}
