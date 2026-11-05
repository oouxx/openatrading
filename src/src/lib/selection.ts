/**
 * Keep the highlighted candidate valid while a filtered list changes.
 *
 * cmdk's own search resets the highlight to the first match, but lists filtered
 * outside `<Command>` (stock search, slash menu) don't go through that path:
 * when the previously highlighted item drops out of the results, the stale
 * value survives and Enter selects nothing. Callers recompute the highlight
 * from the current candidates instead.
 */
export function keepValidSelection(current: string | null, values: string[]): string | null {
  if (current && values.includes(current)) return current
  return values[0] ?? null
}
