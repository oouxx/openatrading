import { useMemo } from "react"
import { sanitizeHtml } from "@/lib/sanitize"

interface SafeHtmlProps {
  html: string
  className?: string
}

/**
 * Render external HTML after DOMPurify sanitization.
 *
 * Use this instead of `dangerouslySetInnerHTML` for any content that did not
 * originate in this codebase (news articles, forum posts, agent output).
 */
export function SafeHtml({ html, className }: SafeHtmlProps) {
  const safeHtml = useMemo(() => sanitizeHtml(html), [html])
  return <div className={className} dangerouslySetInnerHTML={{ __html: safeHtml }} />
}
