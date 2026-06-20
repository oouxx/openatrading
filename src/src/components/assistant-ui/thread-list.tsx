import type { ChatThread } from "@/hooks/useChatThreads";

interface ThreadListProps {
  threads: ChatThread[];
  activeThreadId: number | null;
  onNewThread: () => void;
  onSelectThread: (id: number) => void;
  onDeleteThread: (id: number) => void;
}

export function ThreadList({
  threads,
  activeThreadId,
  onNewThread,
  onSelectThread,
  onDeleteThread,
}: ThreadListProps) {
  return (
    <div className="w-56 flex-shrink-0 bg-bg-surface border-r border-hairline flex flex-col h-full">
      <div className="p-2.5">
        <button
          onClick={onNewThread}
          className="w-full px-3 py-1.5 bg-ai/15 text-ai rounded-button text-xs font-mono hover:bg-ai/25 transition-colors cursor-pointer text-center block"
        >
          + 新建会话
        </button>
      </div>
      <div className="flex-1 overflow-y-auto px-2 pb-2 space-y-0.5">
        {threads.map((thread) => (
          <div
            key={thread.id}
            data-active={activeThreadId === thread.id}
            className="group flex items-center gap-1 px-2 py-1.5 rounded-md hover:bg-bg-hover cursor-pointer transition-colors data-[active=true]:bg-ai/10"
            onClick={() => onSelectThread(thread.id)}
          >
            <span className="flex-1 text-left min-w-0 text-xs text-ink truncate">
              {thread.title}
            </span>
            <button
              onClick={(e) => {
                e.stopPropagation();
                onDeleteThread(thread.id);
              }}
              className="opacity-0 group-hover:opacity-100 text-ink-muted hover:text-red-400 transition-all shrink-0 p-0.5"
              title="删除会话"
            >
              <svg
                xmlns="http://www.w3.org/2000/svg"
                className="w-3.5 h-3.5"
                viewBox="0 0 24 24"
                fill="none"
                stroke="currentColor"
                strokeWidth="2"
                strokeLinecap="round"
                strokeLinejoin="round"
              >
                <path d="M3 6h18" />
                <path d="M19 6v14c0 1-1 2-2 2H7c-1 0-2-1-2-2V6" />
                <path d="M8 6V4c0-1 1-2 2-2h4c1 0 2 1 2 2v2" />
              </svg>
            </button>
          </div>
        ))}
      </div>
    </div>
  );
}
