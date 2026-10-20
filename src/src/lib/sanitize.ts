import DOMPurify from "dompurify"

/**
 * Sanitize third-party HTML before injecting it into the DOM.
 *
 * Article bodies come from external sites, so they must never be trusted.
 * We keep semantic markup (paragraphs, lists, links, images) and drop
 * everything that can execute code or leak into the app shell.
 */
const SANITIZE_OPTIONS = {
  USE_PROFILES: { html: true },
  FORBID_TAGS: [
    "style",
    "iframe",
    "object",
    "embed",
    "form",
    "input",
    "button",
    "textarea",
    "select",
  ],
  FORBID_ATTR: ["style", "class", "id"],
}

export function sanitizeHtml(html: string): string {
  return DOMPurify.sanitize(html, SANITIZE_OPTIONS)
}
