import type { Transition } from "motion/react"

/**
 * Shared Motion transitions. Keep animation timing in one place so new
 * surfaces reuse the same rhythm instead of inventing one-off durations.
 */
const EASE = [0.32, 0.72, 0, 1] as const

export const motionTransition = {
  fast: { duration: 0.2, ease: EASE },
  normal: { duration: 0.35, ease: EASE },
  slow: { duration: 0.4, ease: EASE },
} satisfies Record<string, Transition>
