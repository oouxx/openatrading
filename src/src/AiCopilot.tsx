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
  const threadItems = useAuiState((s) => s.threads.threadItems);

  // Get the real backend ID from thread data
  const remoteId = useMemo(() => {
    if (!mainThreadId) return null;
    const item = threadItems.find((t) => t.id === mainThreadId);
    return item?.remoteId ?? null;
  }, [mainThreadId, threadItems]);

  const remoteIdRef = useRef<string | null>(null);
  remoteIdRef.current = remoteId;

  const onSaveMessage = useCallback((role: string, content: string) => {
    const rid = remoteIdRef.current;
    if (rid && /^\d+$/.test(rid)) {
      saveMessage(Number(rid), role, content);
    }
  }, []);

  const { slashAdapter, slashAction, loadMessages } = useAssistantChat(
    "",
    remoteId && /^\d+$/.test(remoteId) ? Number(remoteId) : null,
    onSaveMessage,
  );

  // Load messages when thread changes
  useEffect(() => {
    if (remoteId && /^\d+$/.test(remoteId)) {
      getMessages(Number(remoteId)).then((msgs) => {
        loadMessages(msgs);
      });
    }
  }, [remoteId, loadMessages]);

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
