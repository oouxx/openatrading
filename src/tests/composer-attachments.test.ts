import { describe, expect, test } from "bun:test"

import {
  attachmentLimitMessage,
  attachmentsFromPaths,
  attachmentsTotalLimitMessage,
  formatFileSize,
  isImageFile,
  mergeComposerAttachments,
  readComposerAttachments,
  uploadComposerAttachments,
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
      new File([new Uint8Array(3 * 1024 * 1024 + 1)], "large.jpg", { type: "image/jpeg" }),
      new File(["ok2"], "second.png", { type: "image/png" }),
    ]
    let id = 0

    const result = readComposerAttachments(files, {
      currentCount: 0,
      maxAttachments: 6,
      maxImageBytes: 3 * 1024 * 1024,
      maxFileBytes: 20 * 1024 * 1024,
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
    expect(result.rejected).toEqual(["「large.jpg」超过 3MB"])
  })

  test("files may use the larger file budget while images stay capped", () => {
    const files = [
      new File([new Uint8Array(4 * 1024 * 1024)], "annual-report.pdf", {
        type: "application/pdf",
      }),
      new File([new Uint8Array(4 * 1024 * 1024)], "screenshot.png", { type: "image/png" }),
    ]

    const result = readComposerAttachments(files, {
      currentCount: 0,
      maxAttachments: 6,
      maxImageBytes: 3 * 1024 * 1024,
      maxFileBytes: 20 * 1024 * 1024,
      createId: () => "attachment-1",
      createPreviewUrl: () => "blob:x",
    })

    expect(result.attachments.map((attachment) => attachment.name)).toEqual(["annual-report.pdf"])
    expect(result.rejected).toEqual(["「screenshot.png」超过 3MB"])
  })

  test("accepts a file at the exact byte limit", () => {
    const maxBytes = 3 * 1024 * 1024
    const file = new File([new Uint8Array(maxBytes)], "limit.xlsx", {
      type: "application/vnd.openxmlformats-officedocument.spreadsheetml.sheet",
    })

    const result = readComposerAttachments([file], {
      currentCount: 0,
      maxAttachments: 6,
      maxImageBytes: maxBytes,
      maxFileBytes: maxBytes,
      createId: () => "attachment-1",
      createPreviewUrl: () => "blob:x",
    })

    expect(result.attachments).toHaveLength(1)
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
      maxImageBytes: 1024,
      maxFileBytes: 1024,
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
      maxImageBytes: 1024,
      maxFileBytes: 1024,
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

describe("composer attachment total budget", () => {
  test("rejects a file that would push the batch over the total budget", async () => {
    const files = [
      new File([new Uint8Array(6 * 1024 * 1024)], "a.pdf", { type: "application/pdf" }),
      new File([new Uint8Array(6 * 1024 * 1024)], "b.pdf", { type: "application/pdf" }),
    ]

    const result = await readComposerAttachments(files, {
      currentCount: 0,
      currentBytes: 0,
      maxAttachments: 10,
      maxImageBytes: 10 * 1024 * 1024,
      maxFileBytes: 10 * 1024 * 1024,
      maxTotalBytes: 10 * 1024 * 1024,
      createId: () => "attachment-1",
      readFile: async () => "data:application/pdf;base64,ok",
    })

    expect(result.attachments.map((attachment) => attachment.name)).toEqual(["a.pdf"])
    expect(result.rejected).toEqual(["「b.pdf」会让附件合计超过 10MB（仅剩 4MB）"])
  })

  test("counts bytes already in the composer against the budget", async () => {
    const result = await readComposerAttachments(
      [new File([new Uint8Array(3 * 1024 * 1024)], "c.png", { type: "image/png" })],
      {
        currentCount: 1,
        currentBytes: 8 * 1024 * 1024,
        maxAttachments: 10,
        maxImageBytes: 20 * 1024 * 1024,
        maxFileBytes: 20 * 1024 * 1024,
        maxTotalBytes: 10 * 1024 * 1024,
        createId: () => "attachment-1",
        readFile: async () => "data:image/png;base64,ok",
      }
    )

    expect(result.attachments).toHaveLength(0)
    expect(result.rejected).toEqual(["「c.png」会让附件合计超过 10MB（仅剩 2MB）"])
  })

  test("merge trims incoming attachments by bytes, not just by count", () => {
    const current = [{ ...composerImage("1"), size: 8 * 1024 * 1024 }]
    const incoming = [
      { ...composerImage("2"), size: 1 * 1024 * 1024 },
      { ...composerImage("3"), size: 5 * 1024 * 1024 },
      { ...composerImage("4"), size: 1 * 1024 * 1024 },
    ]

    const merged = mergeComposerAttachments(current, incoming, 10, 10 * 1024 * 1024)

    expect(merged.attachments.map((attachment) => attachment.id)).toEqual(["1", "2", "4"])
    expect(merged.added).toBe(2)
    expect(merged.rejected).toEqual([
      attachmentsTotalLimitMessage(10 * 1024 * 1024, 9 * 1024 * 1024),
    ])
  })

  test("no total budget means no byte-based rejection", () => {
    const current = [{ ...composerImage("1"), size: 50 * 1024 * 1024 }]
    const merged = mergeComposerAttachments(current, [composerImage("2")], 10)

    expect(merged.attachments.map((attachment) => attachment.id)).toEqual(["1", "2"])
    expect(merged.rejected).toEqual([])
  })
})

describe("composer attachment upload", () => {
  function uploaderReturning(
    attachments: { id: string; name: string; size: number; mimeType?: string }[],
    rejected: { name: string; error: string }[] = []
  ) {
    const calls: { sessionId: string; names: string[] }[] = []
    const upload = async (
      sessionId: string,
      items: { file: File; name: string; mimeType?: string }[]
    ) => {
      calls.push({ sessionId, names: items.map((item) => item.name) })
      return { attachments, rejected }
    }
    return { upload, calls }
  }

  test("splits uploads into image and file payloads carrying ids only", async () => {
    const attachments = [
      composerImage("img-1"),
      {
        kind: "file" as const,
        id: "local-2",
        name: "q3.md",
        size: 57,
        mimeType: "text/markdown",
        file: new File(["x"], "q3.md", { type: "text/markdown" }),
      },
    ]
    const { upload, calls } = uploaderReturning([
      { id: "srv-img", name: "img-1.png", size: 10, mimeType: "image/png" },
      { id: "srv-file", name: "q3.md", size: 57, mimeType: "text/markdown" },
    ])

    const result = await uploadComposerAttachments(attachments, "sess-1", upload)

    expect(calls).toEqual([{ sessionId: "sess-1", names: ["img-1.png", "q3.md"] }])
    // 图片：只有 id + 预览 URL，没有 base64。
    expect(result.images).toEqual([
      { id: "srv-img", mimeType: "image/png", previewUrl: "blob:img-1" },
    ])
    expect(result.images[0]).not.toHaveProperty("data")
    // 文件：id + 展示元信息。
    expect(result.files).toEqual([
      { id: "srv-file", name: "q3.md", mimeType: "text/markdown", size: 57 },
    ])
    expect(result.files[0]).not.toHaveProperty("data")
    expect(result.rejected).toEqual([])
  })

  test("any rejection fails the whole message instead of sending a partial set", async () => {
    const attachments = [composerImage("a"), composerImage("b")]
    const { upload } = uploaderReturning(
      [{ id: "srv-a", name: "a.png", size: 10, mimeType: "image/png" }],
      [{ name: "b.png", error: "超过单会话配额" }]
    )

    const result = await uploadComposerAttachments(attachments, "sess-1", upload)

    expect(result.images).toEqual([])
    expect(result.files).toEqual([])
    expect(result.rejected).toEqual(["「b.png」超过单会话配额"])
  })

  test("no attachments means no upload call", async () => {
    const { upload, calls } = uploaderReturning([])

    const result = await uploadComposerAttachments([], "sess-1", upload)

    expect(calls).toEqual([])
    expect(result).toEqual({ images: [], files: [], rejected: [] })
  })
})

describe("desktop path attachments", () => {
  test("path drops become attachments without reading the file", () => {
    const attachments = attachmentsFromPaths(
      [
        { path: "/tmp/a.png", name: "a.png", size: 1024, mimeType: "image/png", isFile: true },
        {
          path: "/tmp/b.pdf",
          name: "b.pdf",
          size: 2048,
          mimeType: "application/pdf",
          isFile: true,
        },
        { path: "/tmp/dir", name: "dir", size: 0, mimeType: "", isFile: false },
        { path: "/tmp/empty.txt", name: "empty.txt", size: 0, isFile: true },
      ],
      { createId: () => `att-${Math.random().toString(36).slice(2, 7)}` }
    )

    // 目录与空文件被过滤掉。
    expect(attachments.map((attachment) => attachment.name)).toEqual(["a.png", "b.pdf"])
    expect(attachments.map((attachment) => attachment.kind)).toEqual(["image", "file"])
    // 只有路径，没有本地 File：字节由桌面进程直接读。
    expect(attachments.every((attachment) => attachment.file === undefined)).toBe(true)
    expect(attachments.map((attachment) => attachment.path)).toEqual(["/tmp/a.png", "/tmp/b.pdf"])
    // 图片没有缩略图（读缩略图就等于把整个文件读进内存）。
    expect(attachments[0].previewUrl).toBe("")
  })

  test("path attachments are uploaded through the path uploader", async () => {
    const attachments = attachmentsFromPaths(
      [{ path: "/tmp/a.pdf", name: "a.pdf", size: 10, mimeType: "application/pdf", isFile: true }],
      { createId: () => "local-1" }
    )
    const pathCalls: string[][] = []
    const fileCalls: string[] = []

    const result = await uploadComposerAttachments(
      attachments,
      "sess-1",
      async (_sid, items) => {
        fileCalls.push(...items.map((item) => item.name))
        return { attachments: [], rejected: [] }
      },
      {
        uploadPaths: async (_sid, paths) => {
          pathCalls.push(paths)
          return {
            attachments: [{ id: "srv-1", name: "a.pdf", size: 10, mimeType: "application/pdf" }],
            rejected: [],
          }
        },
      }
    )

    expect(fileCalls).toEqual([])
    expect(pathCalls).toEqual([["/tmp/a.pdf"]])
    expect(result.files).toEqual([
      { id: "srv-1", name: "a.pdf", mimeType: "application/pdf", size: 10 },
    ])
    expect(result.rejected).toEqual([])
  })

  test("path attachments fail loudly when the environment has no path uploader", async () => {
    const attachments = attachmentsFromPaths(
      [{ path: "/tmp/a.pdf", name: "a.pdf", size: 10, isFile: true }],
      { createId: () => "local-1" }
    )

    const result = await uploadComposerAttachments(attachments, "sess-1", async () => ({
      attachments: [],
      rejected: [],
    }))

    expect(result.files).toEqual([])
    expect(result.rejected).toEqual(["当前环境不支持按路径上传附件"])
  })

  test("mixed file and path attachments keep their original order", async () => {
    const fromFile = composerImage("local-img")
    const fromPath = attachmentsFromPaths(
      [{ path: "/tmp/b.pdf", name: "b.pdf", size: 5, isFile: true }],
      { createId: () => "local-path" }
    )[0]

    const result = await uploadComposerAttachments(
      [fromFile, fromPath],
      "sess-1",
      async () => ({
        attachments: [{ id: "srv-img", name: "local-img.png", size: 10, mimeType: "image/png" }],
        rejected: [],
      }),
      {
        uploadPaths: async () => ({
          attachments: [{ id: "srv-pdf", name: "b.pdf", size: 5 }],
          rejected: [],
        }),
      }
    )

    expect(result.images.map((image) => image.id)).toEqual(["srv-img"])
    expect(result.files.map((file) => file.id)).toEqual(["srv-pdf"])
  })
})
