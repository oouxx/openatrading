import {
  ThreadListPrimitive,
  ThreadListItemPrimitive,
} from "@assistant-ui/react";

export function ThreadList() {
  return (
    <div className="w-56 flex-shrink-0 bg-bg-surface border-r border-hairline flex flex-col h-full">
      <div className="p-2.5">
        <ThreadListPrimitive.New className="w-full px-3 py-1.5 bg-ai/15 text-ai rounded-button text-xs font-mono hover:bg-ai/25 transition-colors cursor-pointer text-center block">
          + 新建会话
        </ThreadListPrimitive.New>
      </div>
      <ThreadListPrimitive.Items>
        {({ threadListItem }) => (
          <ThreadListItemPrimitive.Root
            key={threadListItem.id}
            className="group flex items-center gap-1 px-2 py-1.5 rounded-md hover:bg-bg-hover cursor-pointer transition-colors data-[active=true]:bg-ai/10"
          >
            <ThreadListItemPrimitive.Trigger className="flex-1 text-left min-w-0">
              <ThreadListItemPrimitive.Title fallback="新对话" />
            </ThreadListItemPrimitive.Trigger>
            <ThreadListItemPrimitive.Delete className="opacity-0 group-hover:opacity-100 text-ink-muted hover:text-red-400 transition-all shrink-0 p-0.5" />
          </ThreadListItemPrimitive.Root>
        )}
      </ThreadListPrimitive.Items>
    </div>
  );
}
