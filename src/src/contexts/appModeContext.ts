import { createContext } from "react"
import type { AppMode } from "@/stores/ui"

export interface AppModeContextType {
  mode: AppMode
  setMode: (mode: AppMode) => void
}

export const AppModeContext = createContext<AppModeContextType | null>(null)
