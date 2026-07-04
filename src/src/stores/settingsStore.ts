import { create } from "zustand"

const PREFIX = "openatrading-"

interface SettingsState {
  apiKey: string
  provider: string
  model: string
  apiUrl: string
  setApiKey: (value: string) => void
  setProvider: (value: string) => void
  setModel: (value: string) => void
  setApiUrl: (value: string) => void
}

export const useSettingsStore = create<SettingsState>((set) => ({
  apiKey: localStorage.getItem(`${PREFIX}api-key`) || "",
  provider: localStorage.getItem(`${PREFIX}provider`) || "openrouter",
  model: localStorage.getItem(`${PREFIX}model`) || "claude-sonnet-4-6",
  apiUrl: localStorage.getItem(`${PREFIX}api-url`) || "",
  setApiKey: (apiKey) => {
    localStorage.setItem(`${PREFIX}api-key`, apiKey)
    set({ apiKey })
  },
  setProvider: (provider) => {
    localStorage.setItem(`${PREFIX}provider`, provider)
    set({ provider })
  },
  setModel: (model) => {
    localStorage.setItem(`${PREFIX}model`, model)
    set({ model })
  },
  setApiUrl: (apiUrl) => {
    localStorage.setItem(`${PREFIX}api-url`, apiUrl)
    set({ apiUrl })
  },
}))
