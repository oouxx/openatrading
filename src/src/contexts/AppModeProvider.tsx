import type { ReactNode } from "react"
import { useUiStore } from "@/stores/ui"
import { AppModeContext } from "./appModeContext"

export function AppModeProvider({ children }: { children: ReactNode }) {
  const mode = useUiStore((s) => s.mode)
  const setMode = useUiStore((s) => s.setMode)
  return <AppModeContext.Provider value={{ mode, setMode }}>{children}</AppModeContext.Provider>
}
