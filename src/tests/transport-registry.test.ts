import { describe, expect, test } from "bun:test"

import { TransportRegistry } from "../src/agents/transportRegistry"

function fakeTransport(id: string) {
  return {
    id,
    disposeCount: 0,
    dispose() {
      this.disposeCount++
    },
  }
}

describe("per-session transport registry", () => {
  test("returns the same transport for the same session", () => {
    const registry = new TransportRegistry<ReturnType<typeof fakeTransport>>()
    const first = registry.getOrCreate("a", () => fakeTransport("a"))
    const again = registry.getOrCreate("a", () => fakeTransport("a-other"))

    expect(again).toBe(first)
  })

  test("creating a transport for another session keeps the first alive", () => {
    const registry = new TransportRegistry<ReturnType<typeof fakeTransport>>()
    const a = registry.getOrCreate("a", () => fakeTransport("a"))
    const b = registry.getOrCreate("b", () => fakeTransport("b"))

    expect(a.disposeCount).toBe(0)
    expect(b.disposeCount).toBe(0)
    expect(registry.get("a")).toBe(a)
    expect(registry.get("b")).toBe(b)
  })

  test("releasing one session disposes only that transport", () => {
    const registry = new TransportRegistry<ReturnType<typeof fakeTransport>>()
    const a = registry.getOrCreate("a", () => fakeTransport("a"))
    const b = registry.getOrCreate("b", () => fakeTransport("b"))

    registry.release("a")

    expect(a.disposeCount).toBe(1)
    expect(b.disposeCount).toBe(0)
    expect(registry.get("a")).toBeUndefined()
    expect(registry.get("b")).toBe(b)
  })

  test("releasing an unknown session is a no-op", () => {
    const registry = new TransportRegistry<ReturnType<typeof fakeTransport>>()
    expect(() => registry.release("missing")).not.toThrow()
  })

  test("disposeAll disposes every live transport", () => {
    const registry = new TransportRegistry<ReturnType<typeof fakeTransport>>()
    const a = registry.getOrCreate("a", () => fakeTransport("a"))
    const b = registry.getOrCreate("b", () => fakeTransport("b"))

    registry.disposeAll()

    expect(a.disposeCount).toBe(1)
    expect(b.disposeCount).toBe(1)
    expect(registry.get("a")).toBeUndefined()
    expect(registry.get("b")).toBeUndefined()
  })
})
