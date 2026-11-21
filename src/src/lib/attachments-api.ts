import { isTauri } from "@/lib/env"
import { getApiBaseUrl } from "@/stores/data"

import type {
  AttachmentUploader,
  InspectedPath,
  PathAttachmentUploader,
  UploadedAttachment,
} from "./composer-attachments"

/** 后端上传接口的响应。 */
interface UploadResponse {
  attachments: UploadedAttachment[]
  rejected?: { name: string; error: string }[]
}

/** 上传进度回调：`loaded`/`total` 是本批已传/待传字节数。 */
export type UploadProgress = (loaded: number, total: number) => void

/** 用户取消上传。 */
export class UploadCancelled extends Error {
  constructor() {
    super("已取消上传")
    this.name = "UploadCancelled"
  }
}

/** File → 标准 base64（无 `data:` 前缀）。桌面端 IPC 用得到。 */
function fileToBase64(file: File): Promise<string> {
  return new Promise((resolve, reject) => {
    const reader = new FileReader()
    reader.onerror = () => reject(new Error(`读取「${file.name || "附件"}」失败`))
    reader.onload = () => {
      const result = typeof reader.result === "string" ? reader.result : ""
      const separator = result.indexOf(",")
      resolve(separator >= 0 ? result.slice(separator + 1) : result)
    }
    reader.readAsDataURL(file)
  })
}

/** 把后端错误体转成用户能看懂的一句话（与 `fetchJson` 同风格）。 */
function errorMessage(status: number, text: string): string {
  if (text) {
    try {
      const parsed: unknown = JSON.parse(text)
      const error = (parsed as { error?: unknown }).error
      if (typeof error === "string" && error.trim()) return error.trim()
    } catch {
      /* 非 JSON，回退到状态码 */
    }
  }
  return `上传失败（${status}）`
}

/**
 * 上传一批文件到 `POST /api/attachments`。
 *
 * 用 `XMLHttpRequest` 而不是 `fetch`：只有 XHR 能拿到**上传进度**（`fetch` 至今
 * 没有 request-body 进度事件）。一个 30MB 的 PDF 传十几秒却只显示"上传中…"
 * 是最容易被当成卡死的交互。
 *
 * 取消走 `signal`：abort 之后 `xhr.abort()` 会让服务端那边的连接断开，
 * 半成品由服务端的 `.part` 机制兜底清掉。
 */
function uploadViaHttp(
  sessionId: string,
  items: { file: File; name: string; mimeType?: string }[],
  signal?: AbortSignal,
  onProgress?: UploadProgress
): Promise<UploadResponse> {
  const total = items.reduce((sum, item) => sum + item.file.size, 0)
  const form = new FormData()
  for (const item of items) form.append("file", item.file, item.name)

  return new Promise((resolve, reject) => {
    if (signal?.aborted) {
      reject(new UploadCancelled())
      return
    }
    const xhr = new XMLHttpRequest()
    xhr.open("POST", `${getApiBaseUrl()}/api/attachments?session=${encodeURIComponent(sessionId)}`)
    xhr.upload.onprogress = (event) => {
      if (event.lengthComputable) onProgress?.(event.loaded, event.total)
      else onProgress?.(0, total)
    }
    xhr.onload = () => {
      if (xhr.status >= 200 && xhr.status < 300) {
        try {
          const parsed = JSON.parse(xhr.responseText) as UploadResponse
          onProgress?.(total, total)
          resolve(parsed)
        } catch {
          reject(new Error("上传响应无法解析"))
        }
        return
      }
      reject(new Error(errorMessage(xhr.status, xhr.responseText)))
    }
    xhr.onerror = () => reject(new Error("上传失败：网络错误"))
    xhr.onabort = () => reject(new UploadCancelled())
    signal?.addEventListener("abort", () => xhr.abort(), { once: true })
    xhr.send(form)
  })
}

/**
 * 附件上传的唯二实现：Web 走 multipart（带进度/取消），桌面端走 IPC。
 *
 * 桌面端没有本地 HTTP 服务（路由是进程内 oneshot），所以单独给了一条命令；
 * 两条路径最终都落到同一份 `attachment_store`，配额与元信息格式一致。
 * IPC 拿不到字节级进度，按「已完成文件数」给进度。
 */
export const uploadAttachments: AttachmentUploader = async (sessionId, items, options) => {
  if (items.length === 0) return { attachments: [], rejected: [] }

  if (isTauri) {
    const { invoke } = await import("@tauri-apps/api/core")
    const attachments: UploadedAttachment[] = []
    const rejected: { name: string; error: string }[] = []
    let loaded = 0
    const total = items.reduce((sum, item) => sum + item.file.size, 0)
    for (const item of items) {
      if (options?.signal?.aborted) throw new UploadCancelled()
      const data = await fileToBase64(item.file)
      const result = (await invoke("store_attachments", {
        sessionId,
        attachments: [{ name: item.name, mimeType: item.mimeType, data }],
      })) as UploadResponse
      attachments.push(...result.attachments)
      rejected.push(...(result.rejected ?? []))
      loaded += item.file.size
      options?.onProgress?.(loaded, total)
    }
    return { attachments, rejected }
  }

  const result = await uploadViaHttp(sessionId, items, options?.signal, options?.onProgress)
  return { attachments: result.attachments, rejected: result.rejected ?? [] }
}

/**
 * 桌面端按路径上传：文件由桌面进程直接读，字节不进 IPC。
 *
 * `Tauri` 拦截了 OS 拖拽事件（`dragDropEnabled` 默认开启），拖进来的文件没有
 * `File` 对象，只有绝对路径。这条路径顺便解决了「300MB 文件经 IPC 传 base64」
 * 的内存问题。
 */
export const uploadAttachmentsByPath: PathAttachmentUploader = async (
  sessionId,
  paths,
  options
) => {
  if (paths.length === 0) return { attachments: [], rejected: [] }
  if (!isTauri) throw new Error("只有桌面端支持按路径上传")
  if (options?.signal?.aborted) throw new UploadCancelled()

  const { invoke } = await import("@tauri-apps/api/core")
  const result = (await invoke("store_attachments_from_paths", {
    sessionId,
    paths,
  })) as UploadResponse
  options?.onProgress?.(1, 1)
  return { attachments: result.attachments, rejected: result.rejected ?? [] }
}

/**
 * 探测拖入路径的元信息（桌面端拖拽）。Web 端返回空数组。
 *
 * 只读 `metadata`，不读内容——拖一个几百 MB 的文件进来也不该先把内存撑起来。
 */
export async function inspectAttachmentPaths(paths: string[]): Promise<InspectedPath[]> {
  if (!isTauri || paths.length === 0) return []
  const { invoke } = await import("@tauri-apps/api/core")
  return (await invoke("inspect_attachment_paths", { paths })) as InspectedPath[]
}
