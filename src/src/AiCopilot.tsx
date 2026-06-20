import { useCallback, useMemo, useEffect, useRef } from "react";
import {
  AssistantRuntimeProvider,
  useRemoteThreadListRuntime,
  useAuiState,
} from "@assistant-ui/react";
import { useAssistantChat } from "@/hooks/useAssistantChat";
import { createThreadListAdapter, getMessages, saveMessage } from "@/hooks/useChatThreads";
import { Thread } from "@/components/assistant-ui/thread";
import { ThreadList } from "@/components/assistant-ui/thread-list";

function AiCopilotContent() {
  const mainThreadId = useAuiState((s) => s.threads.mainThreadId);
  const threadIdRef = useRef<string | null>(null);
  threadIdRef.current = mainThreadId ?? null;

  const onSaveMessage = useCallback((role: string, content: string) => {
    const tid = threadIdRef.current;
    if (tid) {
      saveMessage(Number(tid), role, content);
    }
  }, []);

  const { slashAdapter, slashAction, loadMessages } = useAssistantChat(
    "",
    mainThreadId ? Number(mainThreadId) : null,
    onSaveMessage,
  );

  // Load messages when thread changes
  useEffect(() => {
    if (mainThreadId) {
      getMessages(Number(mainThreadId)).then((msgs) => {
        loadMessages(msgs);
      });
    }
  }, [mainThreadId, loadMessages]);

  return (
    <div
      className="min-h-screen bg-bg pt-11 flex flex-row"
      style={{ height: "calc(100vh - 44px)" }}
    >
      <div className="w-56 flex-shrink-0 bg-bg-surface border-r border-hairline flex flex-col h-full overflow-y-auto">
        <ThreadList />
      </div>
      <div className="flex-1 flex flex-col">
        <Thread slashAdapter={slashAdapter} slashAction={slashAction} />
      </div>
    </div>
  );
}

export default function AiCopilot() {
  const adapter = useMemo(() => createThreadListAdapter(), []);

  const { runtime } = useAssistantChat("");

  const runtimeHook = useCallback(() => runtime, [runtime]);

  const threadListRuntime = useRemoteThreadListRuntime({
    adapter,
    runtimeHook,
  });

  return (
    <AssistantRuntimeProvider runtime={threadListRuntime}>
      <AiCopilotContent />
    </AssistantRuntimeProvider>
  );
}
