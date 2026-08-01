import { isTauri } from "@/lib/env"
import { Routes, Route, Navigate } from "react-router-dom"
import { AppModeProvider } from "@/contexts/AppModeContext"
import AppShell from "@/components/AppShell"
import LandingPage from "./LandingPage"

export default function App() {
  return (
    <AppModeProvider>
      <div className="bg-bg min-h-screen">
        <Routes>
          {isTauri ? (
            <Route path="/" element={<AppShell />} />
          ) : (
            <>
              <Route path="/" element={<LandingPage />} />
              <Route path="/app" element={<AppShell />} />
            </>
          )}
          <Route path="*" element={<Navigate to="/" replace />} />
        </Routes>
      </div>
    </AppModeProvider>
  )
}
