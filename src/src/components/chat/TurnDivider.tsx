import { ArrowRight } from "lucide-react"

// ── Types ────────────────────────────────────────────────

interface TurnDividerProps {
  /** Optional label shown on the divider (e.g. "Turn 结束", "Turn 3"). */
  label?: string
  /** Whether this turn ended in an error. */
  isError?: boolean
}

// ── Main Component ───────────────────────────────────────
//
// Minimal visual separator between assistant turns. Renders as a thin
// hairline with an optional text label + arrow indicator. The divider
// is intentionally low-contrast — it provides structural clarity without
// competing with content blocks.

export default function TurnDivider({ label, isError }: TurnDividerProps) {
  return (
    <div className="flex items-center gap-2 px-0 py-1">
      <div
        className={`flex-1 border-t ${
          isError ? "border-danger/20" : "border-hairline"
        }`}
      />
      {label && (
        <span
          className={`shrink-0 font-number text-caption tracking-wider uppercase ${
            isError ? "text-danger/40" : "text-secondary/30"
          }`}
        >
          {label}
        </span>
      )}
      <ArrowRight
        className={`size-2.5 shrink-0 ${
          isError ? "text-danger/30" : "text-secondary/30"
        }`}
      />
    </div>
  )
}
