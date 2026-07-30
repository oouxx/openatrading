import React, { useState, useCallback } from "react"
import * as Collapsible from "@radix-ui/react-collapsible"

// ── ReasoningRoot ─────────────────────────────────────

export interface ReasoningRootProps {
  defaultOpen?: boolean
  open?: boolean
  onOpenChange?: (open: boolean) => void
  className?: string
  children: React.ReactNode
}

export function ReasoningRoot({
  defaultOpen = false,
  open: controlledOpen,
  onOpenChange: controlledOnOpenChange,
  className = "",
  children,
}: ReasoningRootProps) {
  const [uncontrolledOpen, setUncontrolledOpen] = useState(defaultOpen)
  const isControlled = controlledOpen !== undefined
  const isOpen = isControlled ? controlledOpen : uncontrolledOpen
  const handleOpenChange = useCallback(
    (open: boolean) => {
      if (!isControlled) setUncontrolledOpen(open)
      controlledOnOpenChange?.(open)
    },
    [isControlled, controlledOnOpenChange]
  )
  return (
    <Collapsible.Root
      open={isOpen}
      onOpenChange={handleOpenChange}
      className={`my-2 w-full overflow-hidden rounded-xl border transition-all duration-200 ${
        isOpen
          ? "border-ai/15 bg-ai/[0.03]"
          : "border-hairline bg-transparent"
      } ${className}`}
    >
      {children}
    </Collapsible.Root>
  )
}

// ── ReasoningTrigger ──────────────────────────────────

export function ReasoningTrigger({ active = false }: { active?: boolean }) {
  return (
    <Collapsible.Trigger className="group flex w-full cursor-pointer items-center gap-2 px-3 py-2 text-left transition-colors hover:bg-ai/[0.03]">
      {/* Sparkle icon (Claude-style) */}
      <svg
        className="size-3.5 shrink-0 text-ink-muted"
        viewBox="0 0 16 16"
        fill="none"
        stroke="currentColor"
        strokeWidth="1.5"
        strokeLinecap="round"
        strokeLinejoin="round"
      >
        <path d="M8 1C8 1 8.5 3.5 10.5 5.5C12.5 7.5 15 8 15 8C15 8 12.5 8.5 10.5 10.5C8.5 12.5 8 15 8 15C8 15 7.5 12.5 5.5 10.5C3.5 8.5 1 8 1 8C1 8 3.5 7.5 5.5 5.5C7.5 3.5 8 1 8 1Z" />
      </svg>
      <span className="text-ink-muted font-mono text-[11px] font-medium tracking-wide">
        思考过程
      </span>
      {active && (
        <span className="flex items-center gap-1">
          {[0, 1, 2].map((i) => (
            <span
              key={i}
              className="bg-ai inline-block rounded-full"
              style={{
                width: i === 1 ? "5px" : "4px",
                height: i === 1 ? "5px" : "4px",
                animation: "pulsating-dot 1.2s ease-in-out infinite",
                animationDelay: `${i * 0.2}s`,
              }}
            />
          ))}
        </span>
      )}
      <svg
        className="ml-auto size-3 shrink-0 text-ink-muted/50 transition-transform duration-200 group-data-[state=open]:rotate-180"
        viewBox="0 0 24 24"
        fill="none"
        stroke="currentColor"
        strokeWidth="2"
        strokeLinecap="round"
        strokeLinejoin="round"
      >
        <polyline points="6 9 12 15 18 9" />
      </svg>
    </Collapsible.Trigger>
  )
}

// ── ReasoningContent ──────────────────────────────────

export interface ReasoningContentProps {
  className?: string
  children: React.ReactNode
  "aria-busy"?: boolean
}

export function ReasoningContent({ className = "", children, ...props }: ReasoningContentProps) {
  return (
    <Collapsible.Content
      className={`reasoning-content overflow-hidden ${className}`}
      {...props}
    >
      {children}
    </Collapsible.Content>
  )
}

// ── ReasoningText ─────────────────────────────────────

export interface ReasoningTextProps {
  className?: string
  children: React.ReactNode
}

export function ReasoningText({ className = "", children }: ReasoningTextProps) {
  return (
    <div className={`relative ${className}`}>
      <div className="text-ink-muted max-h-48 overflow-y-auto px-3 pb-3 text-[11px] leading-relaxed whitespace-pre-wrap font-[family-name:var(--font-mono)]">
        {children}
      </div>
      <div
        className="pointer-events-none absolute inset-x-0 bottom-0 h-8"
        style={{
          background: "linear-gradient(to bottom, transparent, color-mix(in oklch, var(--color-bg) 90%, transparent))",
        }}
      />
    </div>
  )
}

// ── ReasoningFade (standalone) ────────────────────────

export function ReasoningFade() {
  return (
    <div
      className="pointer-events-none absolute inset-x-0 bottom-0 h-8"
      style={{
        background: "linear-gradient(to bottom, transparent, color-mix(in oklch, var(--color-bg) 90%, transparent))",
      }}
    />
  )
}
