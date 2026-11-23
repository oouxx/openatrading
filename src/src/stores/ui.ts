import { create } from "zustand"

const PREFIX = "openatrading-"

export type AppMode = "chat" | "watchlist" | "settings" | "roles"
export type SettingsTab =
  | "appearance"
  | "layouts"
  | "agent"
  | "extensions"
  | "skills"
  | "social"
  | "mcp"
  | "cache"
  | "about"

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
  // 旧版的"技能"独立页面已并入设置，老用户直接落到设置的技能页
  if (v === "skill-editor") return "settings"
  if (v === "chat" || v === "watchlist" || v === "settings" || v === "roles") return v
  return "chat"
}

function storedSettingsTab(): SettingsTab {
  const v = localStorage.getItem(`${PREFIX}settings-tab`)
  if (
    v === "appearance" ||
    v === "layouts" ||
    v === "agent" ||
    v === "extensions" ||
    v === "skills" ||
    v === "social" ||
    v === "mcp" ||
    v === "cache" ||
    v === "about"
  )
    return v
  // 从旧的独立技能页迁过来时，直接打开技能页签
  if (localStorage.getItem(`${PREFIX}mode`) === "skill-editor") return "skills"
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
