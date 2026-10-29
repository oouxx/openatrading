import { describe, expect, test } from "bun:test"

import { appendUserTurn, applyEvent } from "../src/agents/reducer"

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
