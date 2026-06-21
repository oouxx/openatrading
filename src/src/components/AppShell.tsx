// src/components/AppShell.tsx
import { useAppMode } from "@/contexts/AppModeContext";
import AiCopilot from "@/AiCopilot";
import ConversationSidebar from "./ConversationSidebar";
import RightPanel from "./RightPanel";
import SkillEditor from "./SkillEditor";
import SettingsView from "./SettingsView";

export default function AppShell() {
  const { mode } = useAppMode();

  return (
    <div className="flex h-screen w-screen overflow-hidden bg-bg">
      {/* Left sidebar always visible */}
      <ConversationSidebar />

      {/* Center + Right area */}
      <div className="flex flex-1 min-w-0">
        {mode === "chat" ? (
          <>
            <div className="flex-1 min-w-0 flex flex-col">
              <AiCopilot />
            </div>
            <RightPanel />
          </>
        ) : mode === "skill-editor" ? (
          <div className="flex-1 min-w-0">
            <SkillEditor />
          </div>
        ) : (
          <div className="flex-1 min-w-0">
            <SettingsView />
          </div>
        )}
      </div>
    </div>
  );
}
