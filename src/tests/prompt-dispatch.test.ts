import { describe, expect, test } from "bun:test"

import { sendWithOptimisticUserTurn } from "../src/agents/promptDispatch"

describe("prompt dispatch ordering", () => {
  test("appends the user turn before dispatching", async () => {
    const calls: string[] = []

    await sendWithOptimisticUserTurn(
      () => {
        calls.push("append")
        return "turn-user"
      },
      () => calls.push("rollback"),
      () => {
        calls.push("dispatch")
      }
    )

    expect(calls).toEqual(["append", "dispatch"])
  })

  test("keeps the turn when dispatch succeeds", async () => {
    const calls: string[] = []

    await sendWithOptimisticUserTurn(
      () => {
        calls.push("append")
        return "turn-user"
      },
      () => calls.push("rollback"),
      async () => {
        calls.push("dispatch")
      }
    )

    expect(calls).not.toContain("rollback")
  })

  test("rolls the turn back and rethrows when dispatch fails", async () => {
    const calls: string[] = []

    await expect(
      sendWithOptimisticUserTurn(
        () => {
          calls.push("append")
          return "turn-user"
        },
        (turnId) => calls.push(`rollback:${turnId}`),
        async () => {
          calls.push("dispatch")
          throw new Error("连接已断开")
        }
      )
    ).rejects.toThrow("连接已断开")

    expect(calls).toEqual(["append", "dispatch", "rollback:turn-user"])
  })

  test("does not call rollback when nothing was appended", async () => {
    const calls: string[] = []

    await expect(
      sendWithOptimisticUserTurn(
        () => null,
        () => calls.push("rollback"),
        async () => {
          throw new Error("连接已断开")
        }
      )
    ).rejects.toThrow("连接已断开")

    expect(calls).toEqual([])
  })
})
