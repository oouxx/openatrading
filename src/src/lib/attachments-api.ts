import { getApiBaseUrl } from "@/stores/data"

import type {
  AttachmentUploadProgress,
  AttachmentUploader,
  InspectedPath,
  UploadedAttachment,
} from "./composer-attachments"

/** 后端上传接口的响应。 */
interface UploadResponse {
  attachments: UploadedAttachment[]
  rejected?: { name: string; error: string }[]
}

/** 用户取消上传。 */
export class UploadCancelled extends Error {
  constructor() {
    super("已取消上传")
    this.name = "UploadCancelled"
  }
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
 * 浏览器上传：`POST /api/attachments` 落盘，返回服务器上的绝对路径。
 *
 * 只有 Web 端需要它——桌面端的拖拽/选择器都自带路径。用 `XMLHttpRequest`
 * 而不是 `fetch`：只有 XHR 能拿到**上传进度**（`fetch` 至今没有 request-body
 * 进度事件），一个 300MB 的日志传几十秒却只显示"上传中…"很容易被当成卡死。
 */
export const uploadAttachments: AttachmentUploader = (sessionId, items, options) => {
  if (items.length === 0) return Promise.resolve({ attachments: [], rejected: [] })

  const total = items.reduce((sum, item) => sum + item.file.size, 0)
  const form = new FormData()
  for (const item of items) form.append("file", item.file, item.name)

  return new Promise((resolve, reject) => {
    if (options?.signal?.aborted) {
      reject(new UploadCancelled())
      return
    }
    const xhr = new XMLHttpRequest()
    xhr.open("POST", `${getApiBaseUrl()}/api/attachments?session=${encodeURIComponent(sessionId)}`)
    const report: AttachmentUploadProgress | undefined = options?.onProgress
    xhr.upload.onprogress = (event) => {
      if (event.lengthComputable) report?.(event.loaded, event.total)
      else report?.(0, total)
    }
    xhr.onload = () => {
      if (xhr.status >= 200 && xhr.status < 300) {
        try {
          const parsed = JSON.parse(xhr.responseText) as UploadResponse
          report?.(total, total)
          resolve({ attachments: parsed.attachments, rejected: parsed.rejected ?? [] })
        } catch {
          reject(new Error("上传响应无法解析"))
        }
        return
      }
      reject(new Error(errorMessage(xhr.status, xhr.responseText)))
    }
    xhr.onerror = () => reject(new Error("上传失败：网络错误"))
    xhr.onabort = () => reject(new UploadCancelled())
    options?.signal?.addEventListener("abort", () => xhr.abort(), { once: true })
    xhr.send(form)
  })
}

/**
 * 探测拖入路径的元信息（桌面端拖拽）。Web 端返回空数组。
 *
 * 只读 `metadata`，不读内容——拖一个几百 MB 的文件进来也不该先把内存撑起来。
 */
export async function inspectAttachmentPaths(paths: string[]): Promise<InspectedPath[]> {
  if (paths.length === 0) return []
  const { invoke } = await import("@tauri-apps/api/core")
  return (await invoke("inspect_attachment_paths", { paths })) as InspectedPath[]
}
