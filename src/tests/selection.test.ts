import { describe, expect, test } from "bun:test"

import { keepValidSelection } from "../src/lib/selection"

describe("filtered-list highlight", () => {
  test("keeps a highlighted value that is still in the list", () => {
    expect(keepValidSelection("600519", ["000001", "600519"])).toBe("600519")
  })

  test("falls back to the first item when the highlight was filtered out", () => {
    expect(keepValidSelection("600519", ["000001", "300750"])).toBe("000001")
  })

  test("selects the first item when nothing was highlighted", () => {
    expect(keepValidSelection(null, ["000001"])).toBe("000001")
  })

  test("clears the highlight when the list is empty", () => {
    expect(keepValidSelection("600519", [])).toBeNull()
  })
})
