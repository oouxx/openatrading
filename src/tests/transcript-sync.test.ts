import { describe, expect, test } from "bun:test"

import { appendUserTurn, applyEvent, canReplaceWithHistory } from "../src/agents/reducer"

describe("optimistic user transcript", () => {
  test("survives the first assistant event", () => {
    const withUser = appendUserTurn([], "用户问题", [])

    const started = applyEvent(withUser, { kind: "session", type: "agent_start" })
    const streaming = applyEvent(started.turns, {
      kind: "session",
      type: "message_update",
      assistant_message_event: { type: "text_delta", delta: "回答" },
    })

    expect(streaming.turns.map((turn) => turn.role)).toEqual(["user", "assistant"])
    expect(streaming.turns[0]?.blocks[0]).toMatchObject({
      type: "text",
      content: "用户问题",
    })
  })
})

describe("history snapshot replacement", () => {
  test("replaces completed turns when the session is idle", () => {
    const turns = appendUserTurn([], "用户问题", [])
    expect(canReplaceWithHistory(turns, false)).toBe(true)
  })

  test("replaces an empty transcript", () => {
    expect(canReplaceWithHistory([], false)).toBe(true)
  })

  test("keeps live turns while a turn is in progress", () => {
    const withUser = appendUserTurn([], "用户问题", [])
    const live = applyEvent(withUser, { kind: "session", type: "agent_start" }).turns
    expect(canReplaceWithHistory(live, false)).toBe(false)
  })

  test("keeps turns for a streaming session even between turns", () => {
    // A streaming session briefly has no in-progress turn (turn_end before
    // the next turn_start); the flag still protects the live transcript.
    const completed = appendUserTurn([], "用户问题", [])
    expect(canReplaceWithHistory(completed, true)).toBe(false)
  })
})
