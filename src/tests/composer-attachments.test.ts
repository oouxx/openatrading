import { describe, expect, test } from "bun:test"

import {
  attachmentLimitMessage,
  attachmentsFromPaths,
  formatFileSize,
  isImageFile,
  mergeComposerAttachments,
  readComposerAttachments,
  uploadComposerAttachments,
  type ComposerFileAttachment,
  type ComposerImageAttachment,
} from "../src/lib/composer-attachments"

function composerImage(id: string, size = 10): ComposerImageAttachment {
  return {
    kind: "image",
    id,
    name: `${id}.png`,
    size,
    mimeType: "image/png",
    file: new File(["x"], `${id}.png`, { type: "image/png" }),
    previewUrl: `blob:${id}`,
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
  test("classifies images and files, and aggregates every rejected reason", () => {
    const files = [
      new File(["ok"], "chart.png", { type: "image/png" }),
      new File(["report"], "notes.txt", { type: "text/plain" }),
      new File(["ok2"], "second.png", { type: "image/png" }),
    ]
    let id = 0

    const result = readComposerAttachments(files, {
      currentCount: 0,
      maxAttachments: 6,
      createId: () => `attachment-${++id}`,
      createPreviewUrl: (file) => `blob:${file.name}`,
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
    // 不再读内容：保留的是原始 File，发送时才上传。
    expect(result.attachments.map((attachment) => attachment.file.name)).toEqual([
      "chart.png",
      "notes.txt",
      "second.png",
    ])
    expect(result.rejected).toEqual([])
  })

  test("has no byte limit: a huge file is accepted", () => {
    const file = new File([new Uint8Array(64 * 1024 * 1024)], "big.log", {
      type: "text/plain",
    })

    const result = readComposerAttachments([file], {
      currentCount: 0,
      maxAttachments: 6,
      createId: () => "attachment-1",
      createPreviewUrl: () => "blob:x",
    })

    expect(result.attachments).toHaveLength(1)
    expect(result.attachments[0].size).toBe(64 * 1024 * 1024)
    expect(result.rejected).toEqual([])
  })

  test("a rejected file does not consume the remaining attachment slot", () => {
    const files = [
      new File(["first"], "first.png", { type: "image/png" }),
      new File(["second"], "second.png", { type: "image/png" }),
    ]

    const result = readComposerAttachments(files, {
      currentCount: 1,
      maxAttachments: 2,
      createId: () => "attachment-1",
      createPreviewUrl: () => "blob:x",
    })

    expect(result.attachments.map((attachment) => attachment.name)).toEqual(["first.png"])
    expect(result.rejected).toEqual([attachmentLimitMessage(2)])
  })

  test("does not accept beyond the available slots", () => {
    const files = [
      new File(["first"], "first.png", { type: "image/png" }),
      new File(["second"], "second.png", { type: "image/png" }),
    ]

    const result = readComposerAttachments(files, {
      currentCount: 5,
      maxAttachments: 6,
      createId: () => "attachment-1",
      createPreviewUrl: () => "blob:x",
    })

    expect(result.attachments).toHaveLength(1)
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

describe("prompt payloads", () => {
  function attachment(overrides: Partial<ComposerFileAttachment>): ComposerFileAttachment {
    return {
      kind: "file",
      id: "local-1",
      name: "notes.md",
      size: 10,
      mimeType: "text/markdown",
      file: new File(["0123456789"], "notes.md", { type: "text/markdown" }),
      ...overrides,
    }
  }

  test("a path attachment is referenced without any transfer", async () => {
    const uploads: string[] = []
    const result = await uploadComposerAttachments(
      [attachment({ path: "/tmp/notes.md", file: undefined })],
      "sess-1",
      async (_sid, items) => {
        uploads.push(...items.map((i) => i.name))
        return { attachments: [], rejected: [] }
      }
    )

    expect(uploads).toEqual([], "有路径就不该上传")
    expect(result.files).toEqual([
      {
        path: "/tmp/notes.md",
        data: undefined,
        name: "notes.md",
        mimeType: "text/markdown",
        size: 10,
      },
    ])
    expect(result.rejected).toEqual([])
  })

  test("a browser file is uploaded and becomes a server path", async () => {
    const result = await uploadComposerAttachments([attachment({})], "sess-1", async () => ({
      attachments: [{ path: "/data/attachments/sess-1/uuid-notes.md", name: "notes.md", size: 10 }],
      rejected: [],
    }))

    expect(result.files[0].path).toBe("/data/attachments/sess-1/uuid-notes.md")
    expect(result.files[0].data).toBeUndefined()
  })

  test("without an uploader (desktop paste) the bytes are inlined", async () => {
    const result = await uploadComposerAttachments([attachment({})], "sess-1")

    expect(result.files[0].path).toBeUndefined()
    // FileReader 的 data URL 去掉前缀后就是标准 base64。
    expect(result.files[0].data).toBe(Buffer.from("0123456789").toString("base64"))
  })

  test("image payload keeps its local preview and never carries base64 twice", async () => {
    const result = await uploadComposerAttachments(
      [
        {
          kind: "image",
          id: "img-1",
          name: "chart.png",
          size: 3,
          mimeType: "image/png",
          path: "/tmp/chart.png",
          previewUrl: "blob:chart",
        },
      ],
      "sess-1"
    )

    expect(result.images).toEqual([
      { path: "/tmp/chart.png", data: undefined, mimeType: "image/png", previewUrl: "blob:chart" },
    ])
  })

  test("an upload rejection fails the whole message", async () => {
    const result = await uploadComposerAttachments([attachment({})], "sess-1", async () => ({
      attachments: [],
      rejected: [{ name: "notes.md", error: "磁盘已满" }],
    }))

    expect(result.files).toEqual([])
    expect(result.rejected).toEqual(["「notes.md」磁盘已满"])
  })
})
