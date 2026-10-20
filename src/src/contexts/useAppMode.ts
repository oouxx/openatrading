import { useContext } from "react"
import { AppModeContext, type AppModeContextType } from "./appModeContext"

export function useAppMode(): AppModeContextType {
  const ctx = useContext(AppModeContext)
  if (!ctx) throw new Error("useAppMode must be used within AppModeProvider")
  return ctx
}
