import { create } from "zustand"

const PREFIX = "openatrading-"

interface SettingsState {
  apiUrl: string
  setApiUrl: (value: string) => void
}

export const useSettingsStore = create<SettingsState>((set) => ({
  apiUrl: localStorage.getItem(`${PREFIX}api-url`) || "",
  setApiUrl: (apiUrl) => {
    localStorage.setItem(`${PREFIX}api-url`, apiUrl)
    set({ apiUrl })
  },
}))
