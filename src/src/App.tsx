import { Routes, Route, NavLink, Navigate } from "react-router-dom";
import LandingPage from "./LandingPage";
import TradingView from "./TradingView";
import RoleView from "./RoleView";
import AiCopilot from "./AiCopilot";
import ReplayView from "./ReplayView";
import BreakoutView from "./BreakoutView";
import DragonTigerView from "./DragonTigerView";
import ThemeToggle from "./components/ThemeToggle";
import { useIndexQuotes } from "@/hooks/query/useIndexQuotes";

const isTauri = "__TAURI_INTERNALS__" in window;

function IndexTicker() {
  const { data: quotes = [] } = useIndexQuotes();
  if (!quotes.length) return null;

  const iconInfo: Record<string, { label: string; bg: string }> = {
    "000001.SS": { label: "沪", bg: "bg-up" },
    "399001.SZ": { label: "深", bg: "bg-[oklch(45%_0.14_240)]" },
    "000300.SS": { label: "300", bg: "bg-ai" },
    "399006.SZ": { label: "创", bg: "bg-down" },
    "000688.SS": { label: "科", bg: "bg-[oklch(42%_0.16_280)]" },
  };

  return (
    <div className="hidden md:flex items-center gap-2">
      {quotes.map((q) => {
        const up = q.px_change_rate >= 0;
        const icon = iconInfo[q.symbol] || { label: q.name[0], bg: "bg-ai" };
        return (
          <div key={q.symbol} className="flex items-center gap-1.5 px-2 py-1 rounded-lg bg-bg-surface border border-hairline">
            <span className={`w-5 h-5 rounded flex items-center justify-center text-[8px] font-bold text-white flex-shrink-0 ${icon.bg}`}>
              {icon.label}
            </span>
            <span className="font-mono text-[11px] text-white font-medium">
              {q.last_px.toLocaleString(undefined, { minimumFractionDigits: 2, maximumFractionDigits: 2 })}
            </span>
            <span className={`font-mono text-[11px] font-semibold ${up ? "text-up" : "text-down"}`}>
              {up ? "+" : ""}{q.px_change_rate.toFixed(2)}%
            </span>
          </div>
        );
      })}
    </div>
  );
}

export default function App() {
  return (
    <div className="min-h-screen bg-bg">
      <header
        className="fixed top-0 w-full z-50 h-11 border-b border-hairline"
        style={{ background: "var(--color-header-bg)" }}
      >
        <div className="max-w-[1600px] mx-auto px-6 h-full flex items-center justify-between">
          <div className="flex items-center gap-6 flex-shrink-0">
            {isTauri ? (
              <span className="text-nav-link font-semibold text-white tracking-tight">
                OpenATrading
              </span>
            ) : (
              <NavLink
                to="/"
                className="text-nav-link font-semibold text-white tracking-tight hover:text-ai transition-colors"
              >
                OpenATrading
              </NavLink>
            )}
            <NavLink
              to="/dragon-tiger"
              className={({ isActive }) =>
                `text-nav-link font-mono text-ink-muted hover:text-white transition-colors ${isActive ? "!text-white" : ""}`
              }
            >
              龙虎榜
            </NavLink>
            <NavLink
              to="/breakout"
              className={({ isActive }) =>
                `text-nav-link font-mono text-ink-muted hover:text-white transition-colors ${isActive ? "!text-white" : ""}`
              }
            >
              晋级分析
            </NavLink>
            <NavLink
              to="/replay"
              className={({ isActive }) =>
                `text-nav-link font-mono text-ink-muted hover:text-white transition-colors ${isActive ? "!text-white" : ""}`
              }
            >
              行情回放
            </NavLink>
            <NavLink
              to="/roles"
              className={({ isActive }) =>
                `text-nav-link font-mono text-ink-muted hover:text-white transition-colors ${isActive ? "!text-white" : ""}`
              }
            >
              角色策略
            </NavLink>
            <NavLink
              to="/copilot"
              className={({ isActive }) =>
                `text-nav-link font-mono text-ink-muted hover:text-white transition-colors ${isActive ? "!text-white" : ""}`
              }
            >
              AI 问答
            </NavLink>
          </div>
          <div className="flex items-center gap-4">
            <IndexTicker />
            <ThemeToggle />
          </div>
        </div>
      </header>

      <Routes>
        <Route path="/" element={isTauri ? <TradingView /> : <LandingPage />} />
        <Route path="/trading" element={<TradingView />} />
        <Route path="/roles" element={<RoleView />} />
        <Route path="/copilot" element={<AiCopilot />} />
        <Route path="/replay" element={<ReplayView />} />
        <Route path="/breakout" element={<BreakoutView />} />
        <Route path="/dragon-tiger" element={<DragonTigerView />} />
        <Route path="*" element={<Navigate to="/" replace />} />
      </Routes>
    </div>
  );
}
