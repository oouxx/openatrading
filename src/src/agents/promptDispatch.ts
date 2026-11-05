// ── Prompt dispatch ordering ─────────────────────────────────────────────
//
// The local transcript must contain the user's turn before the transport is
// asked to run the agent. Desktop `send_prompt` resolves only after the turn
// has finished, while its events stream into the reducer during the turn; a
// user turn appended after the await therefore landed *after* the assistant
// reply. Both transports go through this helper so the order is structural
// rather than timing-dependent.

/**
 * Append the optimistic user turn, then dispatch the prompt.
 *
 * @param appendUserTurn Appends the turn and returns its id (or `null` when
 *   nothing was appended). Runs before `dispatch`.
 * @param rollbackUserTurn Removes the turn when `dispatch` fails, so a failed
 *   send leaves no phantom question in the transcript.
 * @param dispatch Starts the turn on the transport.
 */
export async function sendWithOptimisticUserTurn(
  appendUserTurn: () => string | null,
  rollbackUserTurn: (turnId: string) => void,
  dispatch: () => Promise<void> | void
): Promise<void> {
  const turnId = appendUserTurn()
  try {
    await dispatch()
  } catch (error) {
    if (turnId) rollbackUserTurn(turnId)
    throw error
  }
}
