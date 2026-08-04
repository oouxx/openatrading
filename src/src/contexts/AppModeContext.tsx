// src/contexts/AppModeContext.tsx
import { createContext, useContext, type ReactNode } from "react"
import { useUiStore } from "@/stores/ui"
import type { AppMode } from "@/stores/ui"

interface AppModeContextType {
  mode: AppMode
  setMode: (mode: AppMode) => void
}

const AppModeContext = createContext<AppModeContextType | null>(null)

export function AppModeProvider({ children }: { children: ReactNode }) {
  const mode = useUiStore((s) => s.mode)
  const setMode = useUiStore((s) => s.setMode)
  return <AppModeContext.Provider value={{ mode, setMode }}>{children}</AppModeContext.Provider>
}

export function useAppMode(): AppModeContextType {
  const ctx = useContext(AppModeContext)
  if (!ctx) throw new Error("useAppMode must be used within AppModeProvider")
  return ctx
}
