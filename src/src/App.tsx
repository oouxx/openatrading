// src/App.tsx
import { Routes, Route, Navigate } from "react-router-dom";
import { AppModeProvider } from "@/contexts/AppModeContext";
import AppShell from "@/components/AppShell";
import LandingPage from "./LandingPage";

const isTauri = "__TAURI_INTERNALS__" in window;

export default function App() {
  return (
    <AppModeProvider>
      <div className="min-h-screen bg-bg">
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
  );
}
