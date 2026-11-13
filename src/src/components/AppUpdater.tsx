import { useCallback, useEffect, useRef, useState } from "react"
import type { Update } from "@tauri-apps/plugin-updater"
import { Button } from "@/components/ui/button"
import { isTauri } from "@/lib/env"
import { SettingRow, SettingsSection } from "./settings-ui"

type Phase = "idle" | "checking" | "latest" | "available" | "downloading" | "ready" | "error"

function errorMessage(e: unknown): string {
  if (e instanceof Error) return e.message
  return typeof e === "string" ? e : "未知错误"
}

/**
 * 应用内更新（仅 Tauri 桌面端，Web 端不渲染）。
 *
 * 检查 / 下载 / 安装全部由 `tauri-plugin-updater` 完成，更新包在安装前会用
 * 编译进客户端的公钥校验 minisign 签名，因此不信任下载源本身。
 *
 * 开发模式仍可检查；`tauri dev` 没有可分发的安装包，安装会失败并显示错误。
 */
export function AppUpdaterSection() {
  const [phase, setPhase] = useState<Phase>("idle")
  const [update, setUpdate] = useState<Update | null>(null)
  const [progress, setProgress] = useState<number | null>(null)
  const [message, setMessage] = useState("")
  const updateRef = useRef<Update | null>(null)

  const checkForUpdate = useCallback(async () => {
    setPhase("checking")
    setProgress(null)
    setMessage("")
    try {
      const { check } = await import("@tauri-apps/plugin-updater")
      const next = await check({ timeout: 20_000 })
      // 重新检查时释放上一条更新在 Rust 侧持有的句柄；句柄可能已被安装流程释放。
      try {
        await updateRef.current?.close()
      } catch {
        /* 已释放的资源不需要处理 */
      }
      updateRef.current = next
      setUpdate(next)
      if (next) {
        setPhase("available")
        setMessage(`新版本 ${next.version} 可用`)
      } else {
        setPhase("latest")
        setMessage("已是最新版本")
      }
    } catch (e) {
      setPhase("error")
      setMessage(errorMessage(e))
    }
  }, [])

  useEffect(() => {
    if (!isTauri) return
    // 打开「关于」页时自动检查一次，用户不必先点按钮才知道有没有新版本。
    /* eslint-disable-next-line react-hooks/set-state-in-effect -- 初始化时检查更新 */
    void checkForUpdate()
  }, [checkForUpdate])

  const install = useCallback(async () => {
    if (!update) return
    setPhase("downloading")
    setProgress(0)
    setMessage("")
    let total = 0
    let received = 0
    try {
      await update.downloadAndInstall((event) => {
        if (event.event === "Started") {
          total = event.data.contentLength ?? 0
          setProgress(total > 0 ? 0 : null)
        } else if (event.event === "Progress") {
          received += event.data.chunkLength
          if (total > 0) setProgress(Math.min(100, Math.round((received / total) * 100)))
        } else {
          setProgress(100)
        }
      })
      // 安装已完成，Rust 侧句柄随命令一起释放。
      updateRef.current = null
      setUpdate(null)
      setPhase("ready")
      setMessage("更新已安装，重启后生效")
    } catch (e) {
      setPhase("error")
      setMessage(errorMessage(e))
    }
  }, [update])

  const relaunch = useCallback(async () => {
    try {
      const { invoke } = await import("@tauri-apps/api/core")
      await invoke("restart_app")
    } catch (e) {
      setPhase("error")
      setMessage(`重启失败：${errorMessage(e)}`)
    }
  }, [])

  if (!isTauri) return null

  let hint = "从 GitHub Releases 获取最新版本"
  switch (phase) {
    case "checking":
      hint = "正在检查…"
      break
    case "available":
    case "latest":
    case "ready":
      hint = message
      break
    case "downloading":
      hint = progress === null ? "正在下载并安装…" : `正在下载并安装 ${progress}%`
      break
    case "error":
      hint = "无法获取更新"
      break
  }

  const action = (() => {
    switch (phase) {
      case "checking":
        return (
          <Button size="sm" disabled>
            检查中…
          </Button>
        )
      case "available":
        return (
          <Button size="sm" onClick={() => void install()}>
            下载并安装
          </Button>
        )
      case "downloading":
        return (
          <Button size="sm" disabled>
            更新中…
          </Button>
        )
      case "ready":
        return (
          <Button size="sm" onClick={() => void relaunch()}>
            重启应用
          </Button>
        )
      default:
        return (
          <Button
            size="sm"
            variant={phase === "error" ? "outline" : "default"}
            onClick={() => void checkForUpdate()}
          >
            {phase === "error" ? "重试" : phase === "latest" ? "重新检查" : "检查更新"}
          </Button>
        )
    }
  })()

  return (
    <SettingsSection title="更新">
      <SettingRow label="应用更新" hint={hint} control={action} />
      {phase === "downloading" && progress !== null && (
        <div className="py-3">
          <div
            role="progressbar"
            aria-valuemin={0}
            aria-valuemax={100}
            aria-valuenow={progress}
            className="bg-canvas h-1.5 w-full overflow-hidden rounded-sm"
          >
            <div className="bg-brand h-full transition-[width]" style={{ width: `${progress}%` }} />
          </div>
        </div>
      )}
      {phase === "error" && <p className="text-danger text-caption pb-3">{message}</p>}
    </SettingsSection>
  )
}
