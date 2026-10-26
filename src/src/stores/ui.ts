import { create } from "zustand"

const PREFIX = "openatrading-"

export type AppMode = "chat" | "watchlist" | "skill-editor" | "settings" | "roles" | "automations"
export type SettingsTab = "appearance" | "agent" | "extensions" | "social" | "mcp" | "about"

interface UiState {
  /** 当前主题: 'dark' | 'light'，默认跟随系统或 localStorage */
  theme: "dark" | "light"
  setTheme: (theme: "dark" | "light") => void

  /** 当前应用模式 */
  mode: AppMode
  setMode: (mode: AppMode) => void

  /** 设置页当前标签页 */
  settingsTab: SettingsTab
  setSettingsTab: (tab: SettingsTab) => void
}

function storedMode(): AppMode {
  const v = localStorage.getItem(`${PREFIX}mode`)
  if (
    v === "chat" ||
    v === "watchlist" ||
    v === "skill-editor" ||
    v === "settings" ||
    v === "roles" ||
    v === "automations"
  )
    return v
  return "chat"
}

function storedSettingsTab(): SettingsTab {
  const v = localStorage.getItem(`${PREFIX}settings-tab`)
  if (
    v === "appearance" ||
    v === "agent" ||
    v === "extensions" ||
    v === "social" ||
    v === "mcp" ||
    v === "about"
  )
    return v
  return "appearance"
}

const initialTheme: "dark" | "light" =
  (localStorage.getItem(`${PREFIX}theme`) as "dark" | "light") ||
  (window.matchMedia?.("(prefers-color-scheme: light)").matches ? "light" : "dark")

// 首次加载即应用已存储主题（否则 data-theme 只在用户点击后才会写入）
document.documentElement.setAttribute("data-theme", initialTheme)

export const useUiStore = create<UiState>((set) => ({
  theme: initialTheme,
  setTheme: (theme) => {
    localStorage.setItem(`${PREFIX}theme`, theme)
    document.documentElement.setAttribute("data-theme", theme)
    set({ theme })
  },

  mode: storedMode(),
  setMode: (mode) => {
    localStorage.setItem(`${PREFIX}mode`, mode)
    set({ mode })
  },

  settingsTab: storedSettingsTab(),
  setSettingsTab: (settingsTab) => {
    localStorage.setItem(`${PREFIX}settings-tab`, settingsTab)
    set({ settingsTab })
  },
}))
