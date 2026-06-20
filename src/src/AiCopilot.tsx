import { useState, useCallback, useEffect } from "react";
import { AssistantRuntimeProvider } from "@assistant-ui/react";
import { useAssistantChat } from "@/hooks/useAssistantChat";
import { useChatThreads } from "@/hooks/useChatThreads";
import { Thread } from "@/components/thread";
import { ThreadList } from "@/components/assistant-ui/thread-list";

export default function AiCopilot() {
  const [activeThreadId, setActiveThreadId] = useState<number | null>(null);
  const { threads, createThread, deleteThread, getMessages, saveMessage } = useChatThreads();

  const onSaveMessage = useCallback(
    (role: string, content: string) => {
      if (activeThreadId) {
        saveMessage(activeThreadId, role, content);
      }
    },
    [activeThreadId, saveMessage],
  );

  const { runtime, slashAdapter, slashAction, loadMessages } = useAssistantChat(
    "",
    activeThreadId,
    onSaveMessage,
  );

  // Load messages when switching threads
  useEffect(() => {
    if (activeThreadId) {
      getMessages(activeThreadId).then((msgs) => {
        loadMessages(msgs);
      });
    }
  }, [activeThreadId, getMessages, loadMessages]);

  const handleNewThread = useCallback(async () => {
    const thread = await createThread();
    if (thread) {
      setActiveThreadId(thread.id);
      loadMessages([]);
    }
  }, [createThread, loadMessages]);

  const handleSelectThread = useCallback(
    (id: number) => {
      setActiveThreadId(id);
    },
    [],
  );

  const handleDeleteThread = useCallback(
    async (id: number) => {
      await deleteThread(id);
      if (activeThreadId === id) {
        setActiveThreadId(null);
        loadMessages([]);
      }
    },
    [activeThreadId, deleteThread, loadMessages],
  );

  return (
    <AssistantRuntimeProvider runtime={runtime}>
      <div
        className="min-h-screen bg-bg pt-11 flex flex-row"
        style={{ height: "calc(100vh - 44px)" }}
      >
        <ThreadList
          threads={threads}
          activeThreadId={activeThreadId}
          onNewThread={handleNewThread}
          onSelectThread={handleSelectThread}
          onDeleteThread={handleDeleteThread}
        />
        <div className="flex-1 flex flex-col">
          <Thread slashAdapter={slashAdapter} slashAction={slashAction} />
        </div>
      </div>
    </AssistantRuntimeProvider>
  );
}
