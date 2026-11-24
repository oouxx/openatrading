import { describe, expect, test } from "bun:test"
import { liveWindow } from "../src/lib/rank-live-window"

describe("liveWindow", () => {
  test("没有可视行时不订阅", () => {
    expect(liveWindow(null, 130)).toEqual({ start: 0, end: 0 })
    expect(liveWindow({ min: 0, max: 5 }, 0)).toEqual({ start: 0, end: 0 })
  })

  test("首屏：从 0 开始，带一块预取", () => {
    expect(liveWindow({ min: 0, max: 9 }, 130)).toEqual({ start: 0, end: 60 })
  })

  test("同一块内滚动不改订阅窗口", () => {
    const first = liveWindow({ min: 0, max: 9 }, 130).start
    const second = liveWindow({ min: 5, max: 14 }, 130).start
    expect(second).toBe(first)
  })

  test("跨块后窗口跟着挪，前后各留一块", () => {
    expect(liveWindow({ min: 60, max: 69 }, 130)).toEqual({ start: 30, end: 120 })
  })

  test("末尾不超过已加载行数", () => {
    expect(liveWindow({ min: 120, max: 129 }, 130)).toEqual({ start: 90, end: 130 })
  })
})
