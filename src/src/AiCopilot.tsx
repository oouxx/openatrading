import { AssistantRuntimeProvider } from "@assistant-ui/react";
import { useAssistantChat } from "@/hooks/useAssistantChat";
import { Thread } from "@/components/thread";

export default function AiCopilot() {
  const { runtime, slashAdapter, slashAction } = useAssistantChat("");

  return (
    <AssistantRuntimeProvider runtime={runtime}>
      <div
        className="min-h-screen bg-bg pt-11 flex flex-col"
        style={{ height: "calc(100vh - 44px)" }}
      >
        <Thread slashAdapter={slashAdapter} slashAction={slashAction} />
      </div>
    </AssistantRuntimeProvider>
  );
}
