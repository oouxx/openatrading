"use client";

import {
  ArrowUpIcon,
  BotIcon,
  DownloadIcon,
  LoaderIcon,
  PencilIcon,
  SquareIcon,
  UserIcon,
} from "lucide-react";
import {
  ActionBarPrimitive,
  AuiIf,
  ComposerPrimitive,
  ErrorPrimitive,
  MessagePrimitive,
  ThreadPrimitive,
} from "@assistant-ui/react";
import type { Unstable_TriggerAdapter, Unstable_TriggerItem } from "@assistant-ui/react";
import "@assistant-ui/react-markdown/styles/dot.css";

import { Button } from "@/components/ui/button";
import { TooltipIconButton } from "@/components/assistant-ui/tooltip-icon-button";
import { MarkdownText } from "@/components/assistant-ui/markdown-text";
import { ToolFallback } from "@/components/assistant-ui/tool-fallback";
import { Reasoning, ReasoningGroup } from "@/components/assistant-ui/reasoning";
import { cn } from "@/lib/utils";

export interface ThreadProps {
  slashAdapter?: Unstable_TriggerAdapter;
  slashAction?: {
    onExecute: (item: Unstable_TriggerItem) => void;
    removeOnExecute?: boolean;
  };
}

export function Thread({ slashAdapter, slashAction }: ThreadProps) {
  return (
    <ThreadPrimitive.Root
      className="flex h-full flex-col bg-background text-sm"
      style={{
    "--thread-max-width": "48rem",
    "--accent-color": "#c96442",
    "--accent-foreground": "#ffffff",
    fontFamily: "Georgia, serif",
      }}
    >
      <ThreadPrimitive.Viewport
        turnAnchor="top"
        className="relative flex flex-1 flex-col overflow-x-auto overflow-y-scroll scroll-smooth px-4 pt-4"
      >


        <ThreadPrimitive.Messages
          components={{
            UserMessage,
            EditComposer,
            AssistantMessage,
          }}
        />

        <ThreadPrimitive.ViewportFooter className="sticky bottom-0 mx-auto mt-auto flex w-full max-w-[var(--thread-max-width)] flex-col gap-4 overflow-visible rounded-t-3xl bg-background pb-4">

          <Composer slashAdapter={slashAdapter} slashAction={slashAction} />
        </ThreadPrimitive.ViewportFooter>
      </ThreadPrimitive.Viewport>
    </ThreadPrimitive.Root>
  );
}

interface ComposerProps {
  slashAdapter?: Unstable_TriggerAdapter;
  slashAction?: { onExecute: (item: Unstable_TriggerItem) => void; removeOnExecute?: boolean };
}

function Composer({ slashAdapter, slashAction }: ComposerProps) {
  const inner = (
    <ComposerPrimitive.Root className="relative flex w-full flex-col">
      <ComposerPrimitive.AttachmentDropzone className="flex w-full flex-col rounded-2xl border border-input bg-background px-1 pt-2 outline-none transition-colors has-[textarea:focus-visible]:border-foreground/60 data-[dragging=true]:border-ring data-[dragging=true]:border-dashed data-[dragging=true]:bg-accent/50">

        <ComposerPrimitive.Input
          placeholder="Send a message..."
          className="mb-1 max-h-32 min-h-14 w-full resize-none bg-transparent px-4 pt-2 pb-3 text-sm outline-none placeholder:text-muted-foreground focus-visible:ring-0"
          rows={1}
          autoFocus
          aria-label="Message input"
        />
        <ComposerAction />
      </ComposerPrimitive.AttachmentDropzone>
    </ComposerPrimitive.Root>
  );

  if (!slashAdapter) return inner;

  return (
    <ComposerPrimitive.Unstable_TriggerPopoverRoot>
      <ComposerPrimitive.Unstable_TriggerPopover char="/" adapter={slashAdapter}>
        {slashAction && (
          <ComposerPrimitive.Unstable_TriggerPopover.Action
            onExecute={slashAction.onExecute}
            removeOnExecute={slashAction.removeOnExecute}
          />
        )}
        <ComposerPrimitive.Unstable_TriggerPopoverItems>
          {(items) => (
            <div className="max-h-48 overflow-y-auto rounded-xl border border-hairline bg-bg-surface shadow-lg p-1 mb-2">
              {items.map((item) => (
                <ComposerPrimitive.Unstable_TriggerPopoverItem
                  key={item.id}
                  item={item}
                  className="w-full text-left px-3 py-2 rounded-lg text-xs text-white hover:bg-ai/20 cursor-pointer transition-colors data-[highlighted]:bg-ai/30"
                >
                  <div className="font-medium text-xs text-white">/{item.label}</div>
                  <div className="text-[11px] text-ink-muted mt-0.5">{item.description}</div>
                </ComposerPrimitive.Unstable_TriggerPopoverItem>
              ))}
            </div>
          )}
        </ComposerPrimitive.Unstable_TriggerPopoverItems>
      </ComposerPrimitive.Unstable_TriggerPopover>

      {inner}
    </ComposerPrimitive.Unstable_TriggerPopoverRoot>
  );
}

function ComposerAction() {
  return (
    <div className="relative mx-2 mb-2 flex items-center justify-between">
      <div />

      <AuiIf condition={(s) => !s.thread.isRunning}>
        <ComposerPrimitive.Send asChild>
          <TooltipIconButton
            tooltip="Send message"
            side="bottom"
            variant="default"
            size="icon"
            className="size-8 rounded-full"
            style={{
              backgroundColor: "var(--accent-color)",
              color: "var(--accent-foreground)",
            }}
            aria-label="Send message"
          >
            <ArrowUpIcon className="size-4" />
          </TooltipIconButton>
        </ComposerPrimitive.Send>
      </AuiIf>

      <AuiIf condition={(s) => s.thread.isRunning}>
        <ComposerPrimitive.Cancel asChild>
          <Button
            type="button"
            variant="default"
            size="icon"
            className="size-8 rounded-full"
            style={{
              backgroundColor: "var(--accent-color)",
              color: "var(--accent-foreground)",
            }}
            aria-label="Stop generating"
          >
            <SquareIcon className="size-3 fill-current" />
          </Button>
        </ComposerPrimitive.Cancel>
      </AuiIf>
    </div>
  );
}

function UserMessage() {
  return (
    <MessagePrimitive.Root
      className="mx-auto grid w-full max-w-[var(--thread-max-width)] auto-rows-auto grid-cols-[minmax(72px,1fr)_auto] content-start gap-y-2 px-2 py-5 fade-in slide-in-from-bottom-1 animate-in duration-150"
      data-role="user"
    >
      

      <div className="relative col-start-2 min-w-0">
        <div className="rounded-2xl bg-muted px-4 py-2.5 break-words text-foreground">
          <MessagePrimitive.Parts />
        </div>
        <div className="absolute top-1/2 left-0 -translate-x-full -translate-y-1/2 pr-2">
          <UserActionBar />
        </div>
      </div>

      
    </MessagePrimitive.Root>
  );
}

function UserActionBar() {
  return (
    <ActionBarPrimitive.Root
      hideWhenRunning
      autohide="not-last"
      className="flex flex-col items-end"
    >
      <ActionBarPrimitive.Edit asChild>
        <TooltipIconButton tooltip="Edit" className="p-4">
          <PencilIcon />
        </TooltipIconButton>
      </ActionBarPrimitive.Edit>
    </ActionBarPrimitive.Root>
  );
}

function EditComposer() {
  return (
    <MessagePrimitive.Root className="mx-auto flex w-full max-w-[var(--thread-max-width)] flex-col px-2 py-3">
      <ComposerPrimitive.Root className="ml-auto flex w-full max-w-[85%] flex-col rounded-2xl bg-muted">
        <ComposerPrimitive.Input
          className="min-h-14 w-full resize-none bg-transparent p-4 text-foreground text-sm outline-none"
          autoFocus
        />
        <div className="mx-3 mb-3 flex items-center gap-2 self-end">
          <ComposerPrimitive.Cancel asChild>
            <Button variant="ghost" size="sm">Cancel</Button>
          </ComposerPrimitive.Cancel>
          <ComposerPrimitive.Send asChild>
            <Button size="sm">Update</Button>
          </ComposerPrimitive.Send>
        </div>
      </ComposerPrimitive.Root>
    </MessagePrimitive.Root>
  );
}

function AssistantMessage() {
  return (
    <MessagePrimitive.Root
      className="relative mx-auto w-full max-w-[var(--thread-max-width)] py-5 fade-in slide-in-from-bottom-1 animate-in duration-150"
      data-role="assistant"
    >
      <div className="flex size-8 shrink-0 items-center justify-center rounded-full bg-primary/10">
        <BotIcon className="size-4" />
      </div>
      <div className="break-words px-2 leading-relaxed text-foreground">
        <MessagePrimitive.Parts
          components={{
            Text: MarkdownText,
            tools: { Fallback: ToolFallback },
            Reasoning,
            ReasoningGroup,
          }} />
        <MessageError />
        <AuiIf condition={(s) => s.thread.isRunning && s.message.content.length === 0}>
          <div className="flex items-center gap-2 text-muted-foreground">
            <LoaderIcon className="size-4 animate-spin" />
            <span className="text-sm">Thinking...</span>
          </div>
        </AuiIf>
      </div>

      <div className="mt-1 ml-2 flex min-h-6 items-center">
        
        <AssistantActionBar />
      </div>
      
      <AuiIf condition={(s) => !s.thread.isRunning}>
        <div className="mt-4 flex flex-wrap gap-2">
          <ThreadPrimitive.Suggestion
            prompt="Tell me more"
            className="rounded-full border bg-background px-3 py-1 text-sm hover:bg-muted"
          >
            Tell me more
          </ThreadPrimitive.Suggestion>
          <ThreadPrimitive.Suggestion
            prompt="Can you explain differently?"
            className="rounded-full border bg-background px-3 py-1 text-sm hover:bg-muted"
          >
            Explain differently
          </ThreadPrimitive.Suggestion>
        </div>
      </AuiIf>
    </MessagePrimitive.Root>
  );
}

function MessageError() {
  return (
    <MessagePrimitive.Error>
      <ErrorPrimitive.Root className="mt-2 rounded-md border border-destructive bg-destructive/10 p-3 text-destructive text-sm dark:bg-destructive/5 dark:text-red-200">
        <ErrorPrimitive.Message className="line-clamp-2" />
      </ErrorPrimitive.Root>
    </MessagePrimitive.Error>
  );
}

function AssistantActionBar() {
  return (
    <ActionBarPrimitive.Root
      hideWhenRunning
      autohide="not-last"
      className="-ml-1 flex gap-1 text-muted-foreground"
    >
      
      <ActionBarPrimitive.ExportMarkdown asChild>
        <TooltipIconButton tooltip="Export as Markdown">
          <DownloadIcon />
        </TooltipIconButton>
      </ActionBarPrimitive.ExportMarkdown>
      
      
    </ActionBarPrimitive.Root>
  );
}