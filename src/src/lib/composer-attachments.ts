import type { PromptFile, PromptImage } from "../agents/types"

/**
 * 输入区图片附件：本地 `File` + 仅用于展示的元信息。
 *
 * `id` 是**本地渲染 id**（不含任何服务端含义）；真正的落盘附件 id 要等发送时
 * 上传后才有。`previewUrl` 是 `URL.createObjectURL` 的结果，移除/发送后必须 revoke。
 */
export interface ComposerImageAttachment {
  kind: "image"
  id: string
  name: string
  size: number
  mimeType: string
  /** 浏览器选择/粘贴/拖拽拿到的文件；桌面端按路径拖入时为空。 */
  file?: File
  /** 桌面端拖拽得到的绝对路径：直接进 prompt，字节不进 IPC。 */
  path?: string
  /** 没有本地文件时的预览地址（桌面端拖拽图片暂无缩略图，置空）。 */
  previewUrl: string
}

/** 输入区文件附件。类型由后端按字节识别，`mimeType` 只是提示。 */
export interface ComposerFileAttachment {
  kind: "file"
  id: string
  name: string
  size: number
  mimeType?: string
  /** 同 [`ComposerImageAttachment`]：`file` 与 `path` 二选一。 */
  file?: File
  path?: string
}

export type ComposerAttachment = ComposerImageAttachment | ComposerFileAttachment

export function isImageAttachment(
  attachment: ComposerAttachment
): attachment is ComposerImageAttachment {
  return attachment.kind === "image"
}

export function isFileAttachment(
  attachment: ComposerAttachment
): attachment is ComposerFileAttachment {
  return attachment.kind === "file"
}

/** pi-rs 能直接作为图片块消化的格式（SVG 等会走文本嵌入）。 */
const IMAGE_MIME_TYPES = new Set([
  "image/png",
  "image/jpeg",
  "image/gif",
  "image/webp",
  "image/bmp",
])
const IMAGE_EXTENSIONS = new Set(["png", "jpg", "jpeg", "gif", "webp", "bmp"])

export function isImageFile(file: File): boolean {
  if (file.type) return IMAGE_MIME_TYPES.has(file.type)
  const extension = file.name.split(".").pop()?.toLowerCase()
  return extension !== undefined && IMAGE_EXTENSIONS.has(extension)
}

/**
 * 收纳选项。
 *
 * **没有字节上限**：非图片附件不会被内联进 prompt，它们只是把绝对路径交给
 * pi-rs 的 `read` 工具；图片由 pi-rs 按模型内联上限缩放。本地再设一道字节
 * 上限，只会拒掉后端本来处理得了的文件。
 */
export interface ReadComposerAttachmentsOptions {
  currentCount: number
  maxAttachments: number
  createId: () => string
  /** 创建图片预览 URL；默认用 `URL.createObjectURL`，测试可注入。 */
  createPreviewUrl?: (file: File) => string
}

export interface ReadComposerAttachmentsResult {
  attachments: ComposerAttachment[]
  rejected: string[]
}

export interface MergeComposerAttachmentsResult {
  attachments: ComposerAttachment[]
  added: number
  rejected: string[]
}

export function attachmentLimitMessage(maxAttachments: number): string {
  return `一条消息最多 ${maxAttachments} 个附件`
}

export function formatFileSize(bytes: number): string {
  if (bytes < 1024) return `${bytes}B`
  if (bytes < 1024 * 1024) return `${Math.round(bytes / 1024)}KB`
  const megabytes = bytes / 1024 / 1024
  return `${Number.isInteger(megabytes) ? megabytes : megabytes.toFixed(1)}MB`
}

/**
 * 把已完成读取的附件合并到最新附件列表。
 *
 * 读取是异步的：两个读取批次可能都基于旧的 `currentCount` 开始，
 * 因此容量必须在真正写入状态时按最新列表重新裁剪。
 */
export function mergeComposerAttachments(
  current: ComposerAttachment[],
  incoming: ComposerAttachment[],
  maxAttachments: number
): MergeComposerAttachmentsResult {
  const limit = Math.max(0, Math.floor(maxAttachments))
  const existing = current.slice(0, limit)
  const available = Math.max(0, limit - existing.length)
  const accepted = incoming.slice(0, available)
  const rejected = accepted.length < incoming.length ? [attachmentLimitMessage(maxAttachments)] : []

  return {
    attachments: [...existing, ...accepted],
    added: accepted.length,
    rejected,
  }
}

/**
 * 校验并收纳一批用户选择的文件。
 *
 * **这里不读文件内容**：附件在发送时才上传（`uploadComposerAttachments`）。
 * 提前读成 base64 会让"选完不发"也占着内存，也没有会话 id 可挂——落盘附件的
 * 目录是 `<数据根>/attachments/<session_id>/`，而会话 id 要到发送时才确定。
 */
export function readComposerAttachments(
  files: File[],
  options: ReadComposerAttachmentsOptions
): ReadComposerAttachmentsResult {
  const attachments: ComposerAttachment[] = []
  const rejected: string[] = []
  let available = Math.max(0, Math.floor(options.maxAttachments) - options.currentCount)
  const preview = options.createPreviewUrl ?? ((file: File) => URL.createObjectURL(file))

  for (const file of files) {
    if (available <= 0) {
      rejected.push(attachmentLimitMessage(options.maxAttachments))
      break
    }

    const image = isImageFile(file)
    const label = file.name || (image ? "剪贴板图片" : "剪贴板文件")

    const id = options.createId()
    attachments.push(
      image
        ? {
            kind: "image",
            id,
            name: label,
            size: file.size,
            mimeType: file.type || "image/png",
            file,
            previewUrl: preview(file),
          }
        : {
            kind: "file",
            id,
            name: label,
            size: file.size,
            mimeType: file.type || undefined,
            file,
          }
    )
    available -= 1
  }

  return { attachments, rejected }
}

/** 桌面拖拽得到的路径元信息（由 `inspect_attachment_paths` 提供）。 */
export interface InspectedPath {
  path: string
  name: string
  size: number
  mimeType?: string
  isFile: boolean
}

/**
 * 把「拖入的路径」变成输入框附件。
 *
 * 桌面端拖拽没有 `File` 对象（Tauri 拦截了 OS 事件，只给绝对路径），所以这里
 * 不读内容、也不生成缩略图：图片直接显示文件名与大小，换来的是大文件不占内存。
 */
export function attachmentsFromPaths(
  inspected: InspectedPath[],
  options: { createId: () => string }
): ComposerAttachment[] {
  return inspected
    .filter((item) => item.isFile && item.size > 0)
    .map((item) => {
      const mimeType = item.mimeType ?? ""
      if (mimeType.startsWith("image/")) {
        return {
          kind: "image" as const,
          id: options.createId(),
          name: item.name,
          size: item.size,
          mimeType,
          path: item.path,
          previewUrl: "",
        }
      }
      return {
        kind: "file" as const,
        id: options.createId(),
        name: item.name,
        size: item.size,
        mimeType: mimeType || undefined,
        path: item.path,
      }
    })
}

/** 释放图片预览 URL：移除附件、发送完成、组件卸载时都要调用。 */
export function revokeAttachmentPreviews(attachments: ComposerAttachment[]): void {
  for (const attachment of attachments) {
    if (attachment.kind === "image") {
      try {
        URL.revokeObjectURL(attachment.previewUrl)
      } catch {
        /* 非浏览器环境（测试）忽略 */
      }
    }
  }
}

/** 进度回调：(已传字节, 总字节)。 */
export type AttachmentUploadProgress = (loaded: number, total: number) => void

/** 上传器的附加选项：取消信号 + 进度回调。 */
export interface AttachmentUploadOptions {
  signal?: AbortSignal
  onProgress?: AttachmentUploadProgress
}

/** 服务端落盘后返回的元信息：路径就是 prompt 里要引用的东西。 */
export interface UploadedAttachment {
  path: string
  name: string
  size: number
  mimeType?: string
}

/** 上传器：把本地文件交给服务端落盘，返回绝对路径（Web 端专用）。 */
export type AttachmentUploader = (
  sessionId: string,
  items: { file: File; name: string; mimeType?: string }[],
  options?: AttachmentUploadOptions
) => Promise<{ attachments: UploadedAttachment[]; rejected: { name: string; error: string }[] }>

/**
 * File → 标准 base64（无 `data:` 前缀）。桌面端没有路径时内联用。
 *
 * 用 `arrayBuffer()` + `btoa` 而不是 `FileReader`：前者在任何 JS 运行时都能
 * 用（测试跑在 Bun 里，没有 `FileReader`），也少一层事件回调。
 */
export async function readFileAsBase64(file: File): Promise<string> {
  const bytes = new Uint8Array(await file.arrayBuffer())
  let binary = ""
  const chunk = 0x8000
  for (let offset = 0; offset < bytes.length; offset += chunk) {
    binary += String.fromCharCode(...bytes.subarray(offset, offset + chunk))
  }
  return btoa(binary)
}

/**
 * 发送前把附件变成 pi-rs 需要的载荷。
 *
 * 三条路：
 *
 * 1. 有 `path`（桌面拖拽/选择器）→ 直接引用，**不经过任何传输**；
 * 2. 有 `file` 且给了上传器（Web 端）→ 上传换服务器路径（浏览器没有路径）；
 * 3. 有 `file` 但没有上传器（桌面粘贴）→ 内联 base64。
 *
 * 每个附件都必须先落在一个真实路径上（或内联），否则 `read` 工具读不到。
 */
export async function uploadComposerAttachments(
  attachments: ComposerAttachment[],
  sessionId: string,
  upload?: AttachmentUploader,
  options?: AttachmentUploadOptions
): Promise<{ images: PromptImage[]; files: PromptFile[]; rejected: string[] }> {
  if (attachments.length === 0) return { images: [], files: [], rejected: [] }

  const pending = attachments.filter((attachment) => !attachment.path && attachment.file)
  const uploadedByKey = new Map<string, UploadedAttachment>()
  const rejected: string[] = []

  if (pending.length > 0 && upload) {
    try {
      const result = await upload(
        sessionId,
        pending.map((attachment) => ({
          file: attachment.file as File,
          name: attachment.name,
          mimeType: attachment.mimeType,
        })),
        options
      )
      rejected.push(...result.rejected.map((item) => `「${item.name}」${item.error}`))
      if (result.attachments.length === pending.length) {
        pending.forEach((attachment, index) => {
          const uploaded = result.attachments[index]
          if (uploaded) uploadedByKey.set(attachment.id, uploaded)
        })
      } else if (result.rejected.length === 0) {
        rejected.push("有附件未能上传，请重试")
      }
    } catch (e) {
      const reason = e instanceof Error ? e.message : "上传失败"
      return { images: [], files: [], rejected: [reason] }
    }
  }
  if (rejected.length > 0) return { images: [], files: [], rejected }

  const images: PromptImage[] = []
  const files: PromptFile[] = []
  for (const attachment of attachments) {
    const uploaded = uploadedByKey.get(attachment.id)
    const path = attachment.path ?? uploaded?.path
    let data: string | undefined
    if (!path && attachment.file) {
      try {
        data = await readFileAsBase64(attachment.file)
      } catch (e) {
        rejected.push(e instanceof Error ? e.message : "读取失败")
        continue
      }
    }
    if (!path && !data) {
      rejected.push(`「${attachment.name}」既没有路径也没有内容`)
      continue
    }

    if (attachment.kind === "image") {
      images.push({
        path,
        data,
        mimeType: uploaded?.mimeType || attachment.mimeType,
        // 预览用本地 blob：消息里不再携带图片字节。
        previewUrl: attachment.previewUrl,
      })
    } else {
      files.push({
        path,
        data,
        name: attachment.name,
        mimeType: uploaded?.mimeType || attachment.mimeType,
        size: uploaded?.size ?? attachment.size,
      })
    }
  }

  return { images, files, rejected }
}

/** 释放图片预览 URL：移除附件、发送完成、组件卸载时都要调用。 */
