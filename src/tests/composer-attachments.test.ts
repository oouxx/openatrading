import { describe, expect, test } from "bun:test"

import {
  attachmentLimitMessage,
  formatFileSize,
  isImageFile,
  mergeComposerAttachments,
  readComposerAttachments,
  type ComposerImageAttachment,
} from "../src/lib/composer-attachments"

function composerImage(id: string): ComposerImageAttachment {
  return {
    kind: "image",
    id,
    name: `${id}.png`,
    size: 10,
    data: id,
    mimeType: "image/png",
  }
}

describe("composer attachment merging", () => {
  test("trims incoming attachments against the latest attachment count", () => {
    const current = ["1", "2", "3", "4", "5"].map(composerImage)
    const incoming = ["6", "7"].map(composerImage)

    const merged = mergeComposerAttachments(current, incoming, 6)

    expect(merged.attachments.map((attachment) => attachment.id)).toEqual([
      "1",
      "2",
      "3",
      "4",
      "5",
      "6",
    ])
    expect(merged.added).toBe(1)
    expect(merged.rejected).toEqual([attachmentLimitMessage(6)])
  })

  test("rejects every attachment when the latest list is already full", () => {
    const current = ["1", "2", "3", "4", "5", "6"].map(composerImage)

    const merged = mergeComposerAttachments(current, [composerImage("7")], 6)

    expect(merged.attachments).toHaveLength(6)
    expect(merged.added).toBe(0)
    expect(merged.rejected).toEqual([attachmentLimitMessage(6)])
  })
})

describe("composer attachment reading", () => {
  test("classifies images and files, and aggregates every rejected reason", async () => {
    const files = [
      new File(["ok"], "chart.png", { type: "image/png" }),
      new File(["report"], "notes.txt", { type: "text/plain" }),
      new File([new Uint8Array(3 * 1024 * 1024 + 1)], "large.jpg", { type: "image/jpeg" }),
      new File(["broken"], "broken.webp", { type: "image/webp" }),
      new File(["ok2"], "second.png", { type: "image/png" }),
    ]
    let id = 0

    const result = await readComposerAttachments(files, {
      currentCount: 0,
      maxAttachments: 6,
      maxImageBytes: 3 * 1024 * 1024,
      maxFileBytes: 20 * 1024 * 1024,
      createId: () => `attachment-${++id}`,
      readFile: async (file) => {
        if (file.name === "broken.webp") throw new Error("read failed")
        return `data:${file.type};base64,${await file.text()}`
      },
    })

    expect(result.attachments.map((attachment) => attachment.name)).toEqual([
      "chart.png",
      "notes.txt",
      "second.png",
    ])
    expect(result.attachments.map((attachment) => attachment.kind)).toEqual([
      "image",
      "file",
      "image",
    ])
    expect(result.attachments.map((attachment) => attachment.data)).toEqual([
      "ok",
      "report",
      "ok2",
    ])
    expect(result.rejected).toEqual([
      "「large.jpg」超过 3MB",
      "「broken.webp」读取失败",
    ])
  })

  test("files may use the larger file budget while images stay capped", async () => {
    const files = [
      new File([new Uint8Array(4 * 1024 * 1024)], "annual-report.pdf", {
        type: "application/pdf",
      }),
      new File([new Uint8Array(4 * 1024 * 1024)], "screenshot.png", { type: "image/png" }),
    ]

    const result = await readComposerAttachments(files, {
      currentCount: 0,
      maxAttachments: 6,
      maxImageBytes: 3 * 1024 * 1024,
      maxFileBytes: 20 * 1024 * 1024,
      createId: () => "attachment-1",
      readFile: async () => "data:application/octet-stream;base64,ok",
    })

    expect(result.attachments.map((attachment) => attachment.name)).toEqual(["annual-report.pdf"])
    expect(result.rejected).toEqual(["「screenshot.png」超过 3MB"])
  })

  test("accepts a file at the exact byte limit", async () => {
    const maxBytes = 3 * 1024 * 1024
    const file = new File([new Uint8Array(maxBytes)], "limit.xlsx", {
      type: "application/vnd.openxmlformats-officedocument.spreadsheetml.sheet",
    })

    const result = await readComposerAttachments([file], {
      currentCount: 0,
      maxAttachments: 6,
      maxImageBytes: maxBytes,
      maxFileBytes: maxBytes,
      createId: () => "attachment-1",
      readFile: async () => "data:application/octet-stream;base64,ok",
    })

    expect(result.attachments).toHaveLength(1)
    expect(result.rejected).toEqual([])
  })

  test("a rejected file does not consume the remaining attachment slot", async () => {
    const files = [
      new File(["first"], "first.png", { type: "image/png" }),
      new File(["second"], "second.png", { type: "image/png" }),
    ]
    const readNames: string[] = []

    const result = await readComposerAttachments(files, {
      currentCount: 1,
      maxAttachments: 2,
      maxImageBytes: 1024,
      maxFileBytes: 1024,
      createId: () => "attachment-1",
      readFile: async (file) => {
        readNames.push(file.name)
        return `data:image/png;base64,${await file.text()}`
      },
    })

    expect(result.attachments.map((attachment) => attachment.name)).toEqual(["first.png"])
    expect(readNames).toEqual(["first.png"])
    expect(result.rejected).toEqual([attachmentLimitMessage(2)])
  })

  test("does not read beyond the available slots", async () => {
    const files = [
      new File(["first"], "first.png", { type: "image/png" }),
      new File(["second"], "second.png", { type: "image/png" }),
    ]
    const readNames: string[] = []

    const result = await readComposerAttachments(files, {
      currentCount: 5,
      maxAttachments: 6,
      maxImageBytes: 1024,
      maxFileBytes: 1024,
      createId: () => "attachment-1",
      readFile: async (file) => {
        readNames.push(file.name)
        return `data:image/png;base64,${await file.text()}`
      },
    })

    expect(result.attachments).toHaveLength(1)
    expect(readNames).toEqual(["first.png"])
    expect(result.rejected).toEqual([attachmentLimitMessage(6)])
  })
})

describe("attachment classification helpers", () => {
  test("detects supported image types from MIME or extension", () => {
    expect(isImageFile(new File([], "chart.png", { type: "image/png" }))).toBe(true)
    expect(isImageFile(new File([], "chart.PNG"))).toBe(true)
    expect(isImageFile(new File([], "icon.svg", { type: "image/svg+xml" }))).toBe(false)
    expect(isImageFile(new File([], "report.docx"))).toBe(false)
  })

  test("formats byte sizes for chips and errors", () => {
    expect(formatFileSize(512)).toBe("512B")
    expect(formatFileSize(2048)).toBe("2KB")
    expect(formatFileSize(3 * 1024 * 1024)).toBe("3MB")
    expect(formatFileSize(3.5 * 1024 * 1024)).toBe("3.5MB")
  })
})
