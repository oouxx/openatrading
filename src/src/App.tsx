import { lazy, Suspense } from "react"
import { isTauri } from "@/lib/env"
import { Routes, Route, Navigate } from "react-router-dom"
import { AppModeProvider } from "@/contexts/AppModeProvider"
import { Skeleton } from "@/components/ui/skeleton"

const AppShell = lazy(() => import("@/components/AppShell"))
const LandingPage = lazy(() => import("./LandingPage"))

function PageFallback() {
  return (
    <div className="bg-bg flex min-h-screen items-center justify-center p-8">
      <Skeleton className="h-8 w-48" />
    </div>
  )
}

export default function App() {
  return (
    <AppModeProvider>
      <div className="bg-bg min-h-screen">
        <Suspense fallback={<PageFallback />}>
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
        </Suspense>
      </div>
    </AppModeProvider>
  )
}
