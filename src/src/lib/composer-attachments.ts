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
  /** 桌面端拖拽得到的绝对路径（按路径直读，不进 IPC 内存）。 */
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

export interface ReadComposerAttachmentsOptions {
  currentCount: number
  /** 输入框里已有附件占用的字节数，用于合计预算。 */
  currentBytes?: number
  maxAttachments: number
  maxImageBytes: number
  maxFileBytes: number
  /** 整条消息所有附件合计上限；不传表示不限制。 */
  maxTotalBytes?: number
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

/** 合计预算的拒绝文案：说清楚上限与当前余量，用户才知道要减掉多少。 */
export function attachmentsTotalLimitMessage(maxTotalBytes: number, usedBytes: number): string {
  const remaining = Math.max(0, maxTotalBytes - usedBytes)
  return `附件合计不能超过 ${formatFileSize(maxTotalBytes)}（仅剩 ${formatFileSize(remaining)}）`
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
  maxAttachments: number,
  maxTotalBytes?: number
): MergeComposerAttachmentsResult {
  const limit = Math.max(0, Math.floor(maxAttachments))
  const existing = current.slice(0, limit)
  const available = Math.max(0, limit - existing.length)
  const rejected: string[] = []
  const accepted: ComposerAttachment[] = []
  let usedBytes = existing.reduce((sum, attachment) => sum + attachment.size, 0)

  for (const attachment of incoming) {
    if (accepted.length >= available) {
      rejected.push(attachmentLimitMessage(maxAttachments))
      break
    }
    if (maxTotalBytes !== undefined && usedBytes + attachment.size > maxTotalBytes) {
      rejected.push(attachmentsTotalLimitMessage(maxTotalBytes, usedBytes))
      continue
    }
    accepted.push(attachment)
    usedBytes += attachment.size
  }

  return {
    attachments: [...existing, ...accepted],
    added: accepted.length,
    rejected: [...new Set(rejected)],
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
  // 合计预算按「已有 + 本批已收」累计，避免一张张加起来冲破单帧上限。
  const maxTotalBytes = options.maxTotalBytes
  let usedBytes = Math.max(0, options.currentBytes ?? 0)
  const preview = options.createPreviewUrl ?? ((file: File) => URL.createObjectURL(file))

  for (const file of files) {
    if (available <= 0) {
      rejected.push(attachmentLimitMessage(options.maxAttachments))
      break
    }

    const image = isImageFile(file)
    const label = file.name || (image ? "剪贴板图片" : "剪贴板文件")
    const maxBytes = image ? options.maxImageBytes : options.maxFileBytes
    if (file.size > maxBytes) {
      rejected.push(`「${label}」超过 ${formatFileSize(maxBytes)}`)
      continue
    }
    if (maxTotalBytes !== undefined && usedBytes + file.size > maxTotalBytes) {
      rejected.push(
        `「${label}」会让附件合计超过 ${formatFileSize(maxTotalBytes)}（仅剩 ${formatFileSize(
          Math.max(0, maxTotalBytes - usedBytes)
        )}）`
      )
      continue
    }

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
    usedBytes += file.size
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

/** 服务端返回的已落盘附件。 */
export interface UploadedAttachment {
  id: string
  name: string
  size: number
  mimeType?: string
}

/** 上传结果：成功的附件 + 逐个被拒的原因。 */
export interface UploadComposerAttachmentsResult {
  uploaded: UploadedAttachment[]
  rejected: string[]
}

/** 进度回调：(已传字节, 总字节)。 */
export type AttachmentUploadProgress = (loaded: number, total: number) => void

/** 上传器的附加选项：取消信号 + 进度回调。 */
export interface AttachmentUploadOptions {
  signal?: AbortSignal
  onProgress?: AttachmentUploadProgress
}

/** 上传器：把本地文件交给服务端落盘，返回 id。Web 走 multipart，桌面走 IPC。 */
export type AttachmentUploader = (
  sessionId: string,
  items: { file: File; name: string; mimeType?: string }[],
  options?: AttachmentUploadOptions
) => Promise<{ attachments: UploadedAttachment[]; rejected: { name: string; error: string }[] }>

/** 按路径上传（桌面拖拽）：服务端/桌面进程直接读那个路径，字节不进内存。 */
export type PathAttachmentUploader = (
  sessionId: string,
  paths: string[],
  options?: AttachmentUploadOptions
) => Promise<{ attachments: UploadedAttachment[]; rejected: { name: string; error: string }[] }>

/**
 * 发送前把附件上传成 id，并拆成 pi-rs 需要的两路载荷。
 *
 * 上传是**发送时**发生的：见 [`readComposerAttachments`] 里关于会话 id 的说明。
 * 任何一个附件被拒都会让整条消息失败（保持"要么都到、要么都不发"的语义），
 * 避免用户以为附件已经发出去了。
 */
export async function uploadComposerAttachments(
  attachments: ComposerAttachment[],
  sessionId: string,
  upload: AttachmentUploader,
  options?: AttachmentUploadOptions & {
    /** 路径上传（桌面拖拽）。附件带 `path` 时必须有它，否则报错。 */
    uploadPaths?: PathAttachmentUploader
  }
): Promise<{ images: PromptImage[]; files: PromptFile[]; rejected: string[] }> {
  if (attachments.length === 0) return { images: [], files: [], rejected: [] }

  const withFile = attachments.filter((attachment) => attachment.file)
  const withPath = attachments.filter((attachment) => !attachment.file && attachment.path)
  if (withFile.length + withPath.length !== attachments.length) {
    return {
      images: [],
      files: [],
      rejected: ["有附件既没有本地文件也没有路径，无法上传"],
    }
  }
  if (withPath.length > 0 && !options?.uploadPaths) {
    return { images: [], files: [], rejected: ["当前环境不支持按路径上传附件"] }
  }

  const uploadedById = new Map<string, UploadedAttachment>()
  const rejected: string[] = []
  const record = (
    sources: ComposerAttachment[],
    result: { attachments: UploadedAttachment[]; rejected: { name: string; error: string }[] }
  ) => {
    rejected.push(...result.rejected.map((item) => `「${item.name}」${item.error}`))
    if (result.attachments.length !== sources.length) {
      // 有具体原因就只说具体原因，别在用户面前堆一句无信息的兜底文案。
      if (result.rejected.length === 0) rejected.push("有附件未能上传，请重试")
      return
    }
    sources.forEach((source, index) => {
      const uploaded = result.attachments[index]
      if (uploaded) uploadedById.set(source.id, uploaded)
    })
  }

  if (withFile.length > 0) {
    record(
      withFile,
      await upload(
        sessionId,
        withFile.map((attachment) => ({
          // 过滤时已保证存在
          file: attachment.file as File,
          name: attachment.name,
          mimeType: attachment.mimeType,
        })),
        options
      )
    )
  }
  if (withPath.length > 0) {
    const uploadPaths = options?.uploadPaths
    if (!uploadPaths) return { images: [], files: [], rejected }
    record(
      withPath,
      await uploadPaths(
        sessionId,
        withPath.map((attachment) => attachment.path as string),
        options
      )
    )
  }
  if (rejected.length > 0) return { images: [], files: [], rejected }

  const images: PromptImage[] = []
  const files: PromptFile[] = []
  attachments.forEach((source) => {
    const uploaded = uploadedById.get(source.id)
    if (!uploaded) return
    if (source.kind === "image") {
      // 预览用本地 blob：消息里不再携带 base64。
      images.push({
        id: uploaded.id,
        mimeType: uploaded.mimeType || source.mimeType,
        previewUrl: source.previewUrl,
      })
    } else {
      files.push({
        id: uploaded.id,
        name: uploaded.name,
        mimeType: uploaded.mimeType || source.mimeType,
        size: uploaded.size,
      })
    }
  })

  return { images, files, rejected }
}
