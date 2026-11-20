import { describe, expect, test } from "bun:test"

import { clampThinkingLevel, thinkingLevelOptions } from "../src/lib/thinking-levels"

describe("thinking levels", () => {
  // 各模型档位差异来自 pi-rs `thinkingLevelMap`，这里用线上真实取到的几组
  test("lists only the levels the model declares", () => {
    // kimi-k3 只有 max；glm-5.2 只有 high/max；qwen3.8-max 没有 high 但有 xhigh
    expect(thinkingLevelOptions(["max"]).map((o) => o.value)).toEqual(["max"])
    expect(thinkingLevelOptions(["high", "max"]).map((o) => o.value)).toEqual(["high", "max"])
    expect(thinkingLevelOptions(["low", "medium", "xhigh"]).map((o) => o.value)).toEqual([
      "low",
      "medium",
      "xhigh",
    ])
  })

  test("falls back to the basic five levels when the model is unknown", () => {
    expect(thinkingLevelOptions(undefined).map((o) => o.value)).toEqual([
      "off",
      "minimal",
      "low",
      "medium",
      "high",
    ])
  })

  test("keeps declared levels it does not know about", () => {
    expect(thinkingLevelOptions(["off", "ultra"]).map((o) => o.value)).toEqual(["off", "ultra"])
  })

  // 与 pi-rs `clamp_thinking_level` 一致：先往更高档找，再往低档找，兜底第一个可用档位
  test("clamps upward first, then downward", () => {
    expect(clampThinkingLevel("medium", ["low", "high"])).toBe("high")
    expect(clampThinkingLevel("high", ["low", "medium"])).toBe("medium")
    expect(clampThinkingLevel("off", ["max"])).toBe("max")
    expect(clampThinkingLevel("medium", ["medium"])).toBe("medium")
  })

  test("leaves the level untouched when the model is unknown", () => {
    expect(clampThinkingLevel("xhigh", undefined)).toBe("xhigh")
  })
})
