import type { PromptFile, PromptImage } from "../agents/types"

/** 输入区图片附件：发送给模型的数据加上仅用于本地展示和移除的元信息。 */
export interface ComposerImageAttachment extends PromptImage {
  kind: "image"
  id: string
  name: string
  size: number
}

/** 输入区文件附件。`data` 是原始文件的 base64，类型由后端按字节识别。 */
export interface ComposerFileAttachment extends PromptFile {
  kind: "file"
  id: string
  name: string
  size: number
  data: string
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
const IMAGE_MIME_TYPES = new Set(["image/png", "image/jpeg", "image/gif", "image/webp", "image/bmp"])
const IMAGE_EXTENSIONS = new Set(["png", "jpg", "jpeg", "gif", "webp", "bmp"])

export function isImageFile(file: File): boolean {
  if (file.type) return IMAGE_MIME_TYPES.has(file.type)
  const extension = file.name.split(".").pop()?.toLowerCase()
  return extension !== undefined && IMAGE_EXTENSIONS.has(extension)
}

export interface ReadComposerAttachmentsOptions {
  currentCount: number
  maxAttachments: number
  maxImageBytes: number
  maxFileBytes: number
  readFile: (file: File) => Promise<string>
  createId: () => string
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
 * 校验并读取一批用户选择的文件。
 *
 * 图片与普通文件共用一个附件名额；被拒绝或读取失败的文件不消耗容量，
 * 也不会阻止同批其他文件。返回的 `attachments` 仍需通过
 * `mergeComposerAttachments` 按最新状态合并。
 */
export async function readComposerAttachments(
  files: File[],
  options: ReadComposerAttachmentsOptions
): Promise<ReadComposerAttachmentsResult> {
  const attachments: ComposerAttachment[] = []
  const rejected: string[] = []
  let available = Math.max(0, Math.floor(options.maxAttachments) - options.currentCount)

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

    try {
      const dataUrl = await options.readFile(file)
      const separator = dataUrl.indexOf(",")
      const data = separator >= 0 ? dataUrl.slice(separator + 1) : dataUrl
      const id = options.createId()
      attachments.push(
        image
          ? {
              kind: "image",
              id,
              name: label,
              size: file.size,
              data,
              mimeType: file.type || "image/png",
            }
          : {
              kind: "file",
              id,
              name: label,
              size: file.size,
              data,
              mimeType: file.type || undefined,
            }
      )
      available -= 1
    } catch {
      rejected.push(`「${label}」读取失败`)
    }
  }

  return { attachments, rejected }
}
