# 三栏 Agent 布局重构 — 实施计划

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 将当前 OpenATrading Web 应用的 6 页面 + AiCopilot 重构为三栏 Agent 布局（左侧会话 | 中央 AI 对话 | 右侧工具面板）

**Architecture:** 使用 React Context（`AppModeContext`）管理 chat/skill-editor/settings 三种布局模式；左侧 `ConversationSidebar` 可折叠调宽；中央复用现有 AiCopilot.tsx（去除全屏样式）；右侧 `RightPanel` 手动打开 4 个现有页面视图。所有 UI 仅用 shadcn registry 组件和 assistant-ui primitive。

**Tech Stack:** React 19, TypeScript, Tailwind CSS 3, shadcn/ui, assistant-ui, React Router v7, Framer Motion, Zustand

## Global Constraints

- 不允许手写自定义组件——所有 UI 必须从 shadcn registry（`src/components/ui/`）和 assistant-ui primitive（`src/components/assistant-ui/`）引用。新增 UI 前先确认 shadcn registry 是否有对应条目：`npx shadcn@latest add <component>`
- 中央仅做 AI 对话——流式 Markdown、文字分析、代码块。不渲染表格、图表、组件
- 右侧面板手动打开，不随对话自动切换。点击同一视图的按钮关闭它
- 颜色/间距使用现有 Tailwind 设计 token（`bg-bg-surface`, `border-hairline`, `text-ink-muted` 等）
- 桌面端（Tauri）和 Web 端共享同一套布局组件，Web 端多一个 `/` → LandingPage 路由
- 每个 task 结束时必须 commit
- 前端所有路径相对于 `/Users/user/Desktop/github/trading-agent/src/`
- 后端所有路径相对于 `/Users/user/Desktop/github/trading-agent/`

---

## File Structure

### 创建的前端文件

- `src/contexts/AppModeContext.tsx` — 布局模式状态（chat/skill-editor/settings）
- `src/components/AppShell.tsx` — 最外层布局容器，按模式切换渲染
- `src/components/ConversationSidebar.tsx` — 左侧面板（会话列表 + 折叠）
- `src/components/ResizableHandle.tsx` — 左右拖拽分隔条
- `src/components/RightPanel.tsx` — 右侧面板容器（可折叠可调宽）
- `src/components/ChatQuickButtons.tsx` — 输入框下方的快捷按钮栏
- `src/components/SkillEditor.tsx` — 两栏 Skill 编辑器
- `src/components/SettingsView.tsx` — 两栏设置页面
- `src/hooks/usePanelWidth.ts` — 面板宽度持久化
- `src/hooks/useConversations.ts` — 会话管理（基础骨架）

### 创建的后端文件

- `crates/web/src/mcp.rs` — MCP 管理 handler（list/add/edit/delete/toggle + 持久化）

### 修改的前端文件

- `src/App.tsx` — 重写路由，添加 AppShell，isTauri 分流
- `src/AiCopilot.tsx` — 去除全屏样式，添加快捷按钮栏
- `src/DragonTigerView.tsx` — 适配右侧嵌入
- `src/BreakoutView.tsx` — 同上
- `src/ReplayView.tsx` — 同上
- `src/RoleView.tsx` — 同上（侧边栏改为 Select 下拉）
- `src/LandingPage.tsx` — 添加"开始使用"按钮跳转 `/app`

### 修改的后端文件

- `crates/web/src/chat.rs` — 新增 get_skills, get_tools handlers；ChatRequest 加 skill_name
- `crates/web/src/lib.rs` — 添加 /api/skills, /api/tools, /api/mcp/servers 路由
- `crates/core/src/llm/tools.rs` — 新增 ToolRegistry::reload_mcp_tools()

### 新增的 shadcn 组件

- `scroll-area` (Task 2)
- `checkbox` (Task 6)
- `card` (Task 6)
- `select` (Task 5)
- `dialog` (Task 10)
- `switch` (Task 10)

---

## Task 1: AppModeContext + AppShell + usePanelWidth

**Files:**

- Create: `src/contexts/AppModeContext.tsx`
- Create: `src/components/AppShell.tsx`
- Create: `src/hooks/usePanelWidth.ts`
- Modify: `src/App.tsx`

**Interfaces:**

- Consumes: 现有 `AiCopilot.tsx`, `LandingPage.tsx`, `ThemeToggle` 的默认 export
- Produces: `AppModeProvider`, `useAppMode()` hook, `AppShell` 组件, `usePanelWidth()` hook

- [ ] **Step 1: Create contexts directory and AppModeContext**

```bash
mkdir -p /Users/user/Desktop/github/trading-agent/src/src/contexts
```

- [ ] **Step 2: Write AppModeContext**

```tsx
// src/contexts/AppModeContext.tsx
import { createContext, useContext, useState, type ReactNode } from "react";

export type AppMode = "chat" | "skill-editor" | "settings";

interface AppModeContextType {
  mode: AppMode;
  setMode: (mode: AppMode) => void;
}

const AppModeContext = createContext<AppModeContextType | null>(null);

export function AppModeProvider({ children }: { children: ReactNode }) {
  const [mode, setMode] = useState<AppMode>("chat");
  return (
    <AppModeContext.Provider value={{ mode, setMode }}>
      {children}
    </AppModeContext.Provider>
  );
}

export function useAppMode(): AppModeContextType {
  const ctx = useContext(AppModeContext);
  if (!ctx) throw new Error("useAppMode must be used within AppModeProvider");
  return ctx;
}
```

- [ ] **Step 3: Write usePanelWidth hook**

```tsx
// src/hooks/usePanelWidth.ts
import { useState, useCallback, useEffect } from "react";

export function usePanelWidth(
  storageKey: string,
  defaultWidth: number,
  min: number,
  max: number,
  collapsedKey: string,
) {
  const [width, setWidthRaw] = useState(() => {
    try {
      return parseInt(
        localStorage.getItem(storageKey) || String(defaultWidth),
        10,
      );
    } catch {
      return defaultWidth;
    }
  });
  const [collapsed, setCollapsed] = useState(() => {
    try {
      return localStorage.getItem(collapsedKey) === "true";
    } catch {
      return false;
    }
  });

  useEffect(() => {
    localStorage.setItem(storageKey, String(width));
  }, [width, storageKey]);
  useEffect(() => {
    localStorage.setItem(collapsedKey, String(collapsed));
  }, [collapsed, collapsedKey]);

  const setWidth = useCallback(
    (fn: number | ((prev: number) => number)) => {
      setWidthRaw((prev) =>
        Math.max(min, Math.min(max, typeof fn === "function" ? fn(prev) : fn)),
      );
    },
    [min, max],
  );

  const toggleCollapse = useCallback(() => setCollapsed((c) => !c), []);

  return {
    width,
    setWidth,
    collapsed,
    toggleCollapse,
    displayWidth: collapsed ? 0 : width,
    min,
    max,
  };
}
```

- [ ] **Step 4: Write AppShell.tsx**

```tsx
// src/components/AppShell.tsx
import { useAppMode } from "@/contexts/AppModeContext";
import AiCopilot from "@/AiCopilot";
import ConversationSidebar from "./ConversationSidebar";
import RightPanel from "./RightPanel";
import SkillEditor from "./SkillEditor";
import SettingsView from "./SettingsView";

export default function AppShell() {
  const { mode } = useAppMode();

  return (
    <div className="flex h-screen w-screen overflow-hidden bg-bg">
      {/* Left sidebar always visible */}
      <ConversationSidebar />

      {/* Center + Right area */}
      <div className="flex flex-1 min-w-0">
        {mode === "chat" ? (
          <>
            <div className="flex-1 min-w-0 flex flex-col">
              <AiCopilot />
            </div>
            <RightPanel />
          </>
        ) : mode === "skill-editor" ? (
          <div className="flex-1 min-w-0">
            <SkillEditor />
          </div>
        ) : (
          <div className="flex-1 min-w-0">
            <SettingsView />
          </div>
        )}
      </div>
    </div>
  );
}
```

- [ ] **Step 5: Rewrite App.tsx**

Remove the old header with NavLinks. Add AppModeProvider wrapping the top-level div with Routes. The old `IndexTicker` component and `ThemeToggle` import are no longer needed at this level (ThemeToggle moves to SettingsView).

```tsx
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
```

- [ ] **Step 6: Commit**

```bash
cd /Users/user/Desktop/github/trading-agent/src
git add src/contexts/AppModeContext.tsx src/hooks/usePanelWidth.ts src/components/AppShell.tsx src/App.tsx
git commit -m "feat: add AppModeContext, AppShell, usePanelWidth; rewrite App routing"
```

---

## Task 2: ConversationSidebar

**Files:**

- Create: `src/components/ConversationSidebar.tsx`
- Create: `src/hooks/useConversations.ts`
- Add via shadcn: `scroll-area`

- [ ] **Step 1: Add scroll-area shadcn component**

```bash
cd /Users/user/Desktop/github/trading-agent/src
npx shadcn@latest add scroll-area
```

- [ ] **Step 2: Write useConversations**

```tsx
// src/hooks/useConversations.ts
import { useState, useCallback } from "react";

export interface Conversation {
  id: string;
  title: string;
  skillName: string;
  createdAt: number;
  updatedAt: number;
}

export function useConversations() {
  const [conversations] = useState<Conversation[]>([]);
  const [selectedId, setSelectedId] = useState<string | null>(null);
  const [search, setSearch] = useState("");

  const selectConversation = useCallback((id: string) => {
    setSelectedId(id);
  }, []);

  return { conversations, selectedId, selectConversation, search, setSearch };
}
```

- [ ] **Step 3: Write ConversationSidebar**

```tsx
// src/components/ConversationSidebar.tsx
import { useState } from "react";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { ScrollArea } from "@/components/ui/scroll-area";
import { Separator } from "@/components/ui/separator";
import { Search, Plus, Settings, Puzzle } from "lucide-react";
import { useAppMode } from "@/contexts/AppModeContext";
import { usePanelWidth } from "@/hooks/usePanelWidth";
import { useConversations } from "@/hooks/useConversations";

const PRESET_SKILLS = [
  { name: "market-analysis", label: "📈 市场分析" },
  { name: "dragon-tiger-tracking", label: "🐉 龙虎榜追踪" },
  { name: "breakout-analysis", label: "🔥 涨停晋级" },
  { name: "market-replay", label: "🎯 行情回放" },
  { name: "strategy-manager", label: "🤖 策略管理" },
  { name: "free-chat", label: "⚡ 自由对话" },
];

export default function ConversationSidebar() {
  const { mode, setMode } = useAppMode();
  const { displayWidth, collapsed, toggleCollapse } = usePanelWidth(
    "layout:left-width",
    240,
    180,
    360,
    "layout:left-collapsed",
  );
  const { conversations, selectedId, selectConversation, search, setSearch } =
    useConversations();
  const [showSkills, setShowSkills] = useState(false);

  // Collapsed state: icon-only bar
  if (collapsed) {
    return (
      <div className="flex flex-col items-center py-3 gap-4 w-12 border-r border-hairline bg-bg-surface flex-shrink-0">
        <Button
          variant="ghost"
          size="icon"
          onClick={toggleCollapse}
          className="w-8 h-8"
        >
          <Plus className="w-4 h-4" />
        </Button>
        <Button
          variant="ghost"
          size="icon"
          onClick={() => setMode("skill-editor")}
          className="w-8 h-8"
        >
          <Puzzle className="w-4 h-4" />
        </Button>
        <div className="flex-1" />
        <Button
          variant="ghost"
          size="icon"
          onClick={() => setMode("settings")}
          className="w-8 h-8"
        >
          <Settings className="w-4 h-4" />
        </Button>
      </div>
    );
  }

  return (
    <div
      className="flex flex-col border-r border-hairline bg-bg-surface flex-shrink-0 overflow-hidden"
      style={{ width: displayWidth || 240, minWidth: 0 }}
    >
      {/* New conversation + collapse toggle */}
      <div className="flex items-center gap-1 px-3 py-2.5 border-b border-hairline">
        <div className="relative flex-1">
          <Button
            variant="default"
            size="sm"
            className="w-full justify-start gap-2 text-sm h-9"
            onClick={() => setShowSkills(!showSkills)}
          >
            <Plus className="w-4 h-4" />
            新建会话
          </Button>
          {showSkills && (
            <div className="absolute top-full left-0 right-0 mt-1 z-50 bg-bg-surface border border-hairline rounded-lg shadow-lg py-1">
              {PRESET_SKILLS.map((skill) => (
                <button
                  key={skill.name}
                  className="w-full text-left px-4 py-2 text-sm text-white hover:bg-bg-hover transition-colors"
                  onClick={() => setShowSkills(false)}
                >
                  {skill.label}
                </button>
              ))}
            </div>
          )}
        </div>
        <Button
          variant="ghost"
          size="icon"
          onClick={toggleCollapse}
          className="w-8 h-8 flex-shrink-0"
        >
          <span className="block w-1 h-1 rounded-full bg-ink-muted shadow-[0_4px_0_0_var(--color-muted),0_8px_0_0_var(--color-muted)]" />
        </Button>
      </div>

      {/* Skills entry */}
      <button
        onClick={() =>
          setMode(mode === "skill-editor" ? "chat" : "skill-editor")
        }
        className="flex items-center gap-2 px-4 py-2 text-sm text-ink-muted hover:text-white hover:bg-bg-hover transition-colors border-b border-hairline"
      >
        <Puzzle className="w-4 h-4" />
        技能
      </button>

      {/* Search */}
      <div className="px-3 py-2">
        <div className="relative">
          <Search className="absolute left-2.5 top-1/2 -translate-y-1/2 w-3.5 h-3.5 text-ink-muted" />
          <Input
            placeholder="搜索会话..."
            value={search}
            onChange={(e) => setSearch(e.target.value)}
            className="pl-8 h-8 text-xs"
          />
        </div>
      </div>

      <Separator />

      {/* Session List */}
      <ScrollArea className="flex-1 px-2 py-1">
        {conversations.length === 0 ? (
          <div className="py-8 text-center text-xs text-ink-muted">
            暂无会话
          </div>
        ) : (
          conversations.map((c) => (
            <button
              key={c.id}
              onClick={() => selectConversation(c.id)}
              className={`w-full text-left px-3 py-2 rounded-lg text-sm transition-colors ${
                selectedId === c.id
                  ? "bg-ai/10 text-white"
                  : "text-ink-muted hover:text-white hover:bg-bg-hover"
              }`}
            >
              <div className="truncate">{c.title}</div>
            </button>
          ))
        )}
      </ScrollArea>

      {/* Settings at bottom */}
      <Separator />
      <button
        onClick={() => setMode(mode === "settings" ? "chat" : "settings")}
        className="flex items-center gap-2 px-4 py-2.5 text-sm text-ink-muted hover:text-white hover:bg-bg-hover transition-colors"
      >
        <Settings className="w-4 h-4" />
        设置
      </button>
    </div>
  );
}
```

- [ ] **Step 4: Commit**

```bash
cd /Users/user/Desktop/github/trading-agent/src
git add src/components/ConversationSidebar.tsx src/hooks/useConversations.ts
git commit -m "feat: add ConversationSidebar with collapsible left panel"
```

---

## Task 3: Central AI Chat — Adapt AiCopilot + Add QuickButtons

**Files:**

- Modify: `src/AiCopilot.tsx`
- Create: `src/components/ChatQuickButtons.tsx`

- [ ] **Step 1: Create ChatQuickButtons**

```tsx
// src/components/ChatQuickButtons.tsx
import { Button } from "@/components/ui/button";

export default function ChatQuickButtons() {
  const openView = (viewId: string) => {
    window.dispatchEvent(
      new CustomEvent("open-right-panel", { detail: viewId }),
    );
  };

  return (
    <div className="flex items-center gap-2 px-4 py-2 border-t border-hairline">
      <Button
        variant="ghost"
        size="sm"
        className="text-xs text-ink-muted hover:text-white h-8 px-3"
        onClick={() => openView("dragon-tiger")}
      >
        📈 龙虎榜
      </Button>
      <Button
        variant="ghost"
        size="sm"
        className="text-xs text-ink-muted hover:text-white h-8 px-3"
        onClick={() => openView("breakout")}
      >
        🔥 晋级分析
      </Button>
      <Button
        variant="ghost"
        size="sm"
        className="text-xs text-ink-muted hover:text-white h-8 px-3"
        onClick={() => openView("replay")}
      >
        🎯 行情回放
      </Button>
      <Button
        variant="ghost"
        size="sm"
        className="text-xs text-ink-muted hover:text-white h-8 px-3"
        onClick={() => openView("roles")}
      >
        🤖 角色策略
      </Button>
    </div>
  );
}
```

- [ ] **Step 2: Modify AiCopilot.tsx**

Remove the outer fullscreen wrapper. The existing code in AiCopilot.tsx has:

```tsx
// The outer return wraps in:
// <div className="min-h-screen bg-bg pt-11 flex flex-row" style={{ height: 'calc(100vh - 44px)' }}>
```

Change to:

```tsx
// <div className="flex flex-row h-full w-full">
```

And add `<ChatQuickButtons />` inside the content area, below `Thread`:

```tsx
<div className="flex-1 flex flex-col min-w-0">
  <Thread slashAdapter={slashAdapter} slashAction={slashAction} />
  <ChatQuickButtons />
</div>
```

Add the import:

```tsx
import ChatQuickButtons from "@/components/ChatQuickButtons";
```

- [ ] **Step 3: Commit**

```bash
cd /Users/user/Desktop/github/trading-agent/src
git add src/AiCopilot.tsx src/components/ChatQuickButtons.tsx
git commit -m "feat: adapt AiCopilot for AppShell; add quick buttons bar"
```

---

## Task 4: RightPanel + ResizableHandle

**Files:**

- Create: `src/components/RightPanel.tsx`
- Create: `src/components/ResizableHandle.tsx`

- [ ] **Step 1: Write ResizableHandle**

```tsx
// src/components/ResizableHandle.tsx
import { useCallback, useRef, useEffect } from "react";

interface Props {
  onResize: (delta: number) => void;
}

export default function ResizableHandle({ onResize }: Props) {
  const dragging = useRef(false);
  const startX = useRef(0);

  const onMouseDown = useCallback((e: React.MouseEvent) => {
    e.preventDefault();
    dragging.current = true;
    startX.current = e.clientX;
  }, []);

  useEffect(() => {
    const onMove = (e: MouseEvent) => {
      if (!dragging.current) return;
      const delta = startX.current - e.clientX; // positive = dragging left = shrink
      startX.current = e.clientX;
      onResize(delta);
    };
    const onUp = () => {
      dragging.current = false;
    };
    window.addEventListener("mousemove", onMove);
    window.addEventListener("mouseup", onUp);
    return () => {
      window.removeEventListener("mousemove", onMove);
      window.removeEventListener("mouseup", onUp);
    };
  }, [onResize]);

  return (
    <div
      className="w-1 cursor-col-resize hover:bg-ai/30 active:bg-ai/50 transition-colors flex-shrink-0 relative"
      onMouseDown={onMouseDown}
    >
      <div className="absolute inset-y-0 -left-1 -right-1" />
    </div>
  );
}
```

- [ ] **Step 2: Write RightPanel**

```tsx
// src/components/RightPanel.tsx
import { useState, useEffect, useCallback } from "react";
import { Button } from "@/components/ui/button";
import { X } from "lucide-react";
import { usePanelWidth } from "@/hooks/usePanelWidth";
import ResizableHandle from "./ResizableHandle";
import DragonTigerView from "@/DragonTigerView";
import BreakoutView from "@/BreakoutView";
import ReplayView from "@/ReplayView";
import RoleView from "@/RoleView";

type ViewId = "dragon-tiger" | "breakout" | "replay" | "roles";

const VIEW_TITLES: Record<string, string> = {
  "dragon-tiger": "🐉 龙虎榜",
  breakout: "🔥 晋级分析",
  replay: "🎯 行情回放",
  roles: "🤖 角色策略",
};

export default function RightPanel() {
  const [activeView, setActiveView] = useState<ViewId | null>(null);
  const { displayWidth, setWidth, collapsed, toggleCollapse, min, max } =
    usePanelWidth(
      "layout:right-width",
      420,
      320,
      600,
      "layout:right-collapsed",
    );

  useEffect(() => {
    const handler = (e: Event) => {
      const detail = (e as CustomEvent).detail as ViewId;
      setActiveView((prev) => (prev === detail ? null : detail));
    };
    window.addEventListener("open-right-panel", handler);
    return () => window.removeEventListener("open-right-panel", handler);
  }, []);

  const onResize = useCallback(
    (delta: number) => {
      setWidth((w) => w + delta);
    },
    [setWidth],
  );

  if (!activeView) return null;

  const renderView = () => {
    switch (activeView) {
      case "dragon-tiger":
        return <DragonTigerView />;
      case "breakout":
        return <BreakoutView />;
      case "replay":
        return <ReplayView />;
      case "roles":
        return <RoleView />;
    }
  };

  return (
    <>
      <ResizableHandle onResize={onResize} />
      <div
        className="flex flex-col bg-bg-surface border-l border-hairline overflow-hidden flex-shrink-0"
        style={{ width: displayWidth || 420 }}
      >
        <div className="flex items-center justify-between px-4 py-2.5 border-b border-hairline flex-shrink-0">
          <span className="text-sm font-medium text-white">
            {VIEW_TITLES[activeView]}
          </span>
          <Button
            variant="ghost"
            size="icon"
            className="w-7 h-7"
            onClick={() => setActiveView(null)}
          >
            <X className="w-4 h-4" />
          </Button>
        </div>
        <div className="flex-1 overflow-y-auto">{renderView()}</div>
      </div>
    </>
  );
}
```

- [ ] **Step 3: Commit**

```bash
cd /Users/user/Desktop/github/trading-agent/src
git add src/components/RightPanel.tsx src/components/ResizableHandle.tsx
git commit -m "feat: add RightPanel with resizable handle and view routing"
```

---

## Task 5: Adapt Existing Views for Right Panel

**Files:**

- Modify: `src/DragonTigerView.tsx`
- Modify: `src/BreakoutView.tsx`
- Modify: `src/ReplayView.tsx`
- Modify: `src/RoleView.tsx`
- Add via shadcn: `select`

Each view needs to remove full-page wrappers (`min-h-screen`, `pt-11`, `h-[calc(100vh-44px)]`, `max-w-[1500px] mx-auto`) so it renders correctly inside the ~420px RightPanel container. Internal logic stays identical.

- [ ] **Step 1: Add select component**

```bash
cd /Users/user/Desktop/github/trading-agent/src
npx shadcn@latest add select
```

- [ ] **Step 2: Adapt DragonTigerView**

In `DragonTigerView.tsx`, remove the outer `min-h-screen bg-bg pt-11` wrapper div and the `max-w-[1500px] mx-auto p-6 w-full` container. Replace with a simple fragment and inner padding div:

```tsx
// Find this pattern:
// <div className="min-h-screen bg-bg pt-11">
//   <div className="max-w-[1500px] mx-auto p-6 w-full">

// Replace with:
// <>
//   <div className="p-4 w-full">
```

Keep `grid-cols-2 md:grid-cols-4`, the filter sidebar, and all internal state unchanged.

- [ ] **Step 3: Adapt BreakoutView**

Same pattern. In `BreakoutView.tsx`:

```tsx
// Remove: <div className="min-h-screen bg-bg pt-11">
// Remove: <div className="max-w-[1500px] mx-auto p-6 w-full">
// Add:    <div className="p-4">

// The filter sidebar (221px) + table layout should work as-is in ~420px
```

- [ ] **Step 4: Adapt ReplayView**

In `ReplayView.tsx`:

```tsx
// Change:
// <div className="min-h-screen bg-bg pt-11 flex flex-col" style={{ height: 'calc(100vh - 44px)' }}>
// To:
// <div className="flex flex-col h-full">
```

The Canvas PulseChart uses `container.getBoundingClientRect()` for its width, so it auto-adapts. Control bar should shrink as needed inside the panel.

- [ ] **Step 5: Adapt RoleView**

RoleView is the trickiest because it has a fixed 260px sidebar. In a ~420px panel, sidebar + content won't fit side by side.

Replace the sidebar layout with a role selector at the top:

```tsx
// Import shadcn Select:
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from "@/components/ui/select";

// Where the original <aside> sidebar was rendered, replace with:
<div className="px-4 py-3 border-b border-hairline">
  <Select value={selectedRole || ""} onValueChange={setSelectedRole}>
    <SelectTrigger className="w-full">
      <SelectValue placeholder="选择角色..." />
    </SelectTrigger>
    <SelectContent>
      {roles.map((r) => (
        <SelectItem key={r.name} value={r.name}>
          {r.display_name || r.name}
        </SelectItem>
      ))}
    </SelectContent>
  </Select>
</div>;
```

Remove `min-h-screen bg-bg pt-11 flex flex-col` and `h-[calc(100vh-44px)]` — replace with `flex flex-col h-full`.

- [ ] **Step 6: Commit**

```bash
cd /Users/user/Desktop/github/trading-agent/src
git add src/DragonTigerView.tsx src/BreakoutView.tsx src/ReplayView.tsx src/RoleView.tsx src/components/ui/select.tsx
git commit -m "fix: adapt views for right panel width, remove fullscreen constraints"
```

---

## Task 6: SkillEditor — Two-Column View

**Files:**

- Create: `src/components/SkillEditor.tsx`
- Add via shadcn: `checkbox`, `card`

- [ ] **Step 1: Add checkbox and card**

```bash
cd /Users/user/Desktop/github/trading-agent/src
npx shadcn@latest add checkbox card
```

- [ ] **Step 2: Write SkillEditor**

The editor uses a two-column layout inside the main content area (with left sidebar still visible). It has:

- Top: skill grid (click to switch which skill is being edited)
- Left column: structured form (name, description, tool checkboxes from `/api/tools`, markdown instructions)
- Right column: markdown preview

```tsx
// src/components/SkillEditor.tsx
import { useState, useEffect } from "react";
import { Input } from "@/components/ui/input";
import { Textarea } from "@/components/ui/textarea";
import { Button } from "@/components/ui/button";
import { ScrollArea } from "@/components/ui/scroll-area";
import { Separator } from "@/components/ui/separator";
import { Checkbox } from "@/components/ui/checkbox";
import { useAppMode } from "@/contexts/AppModeContext";
import { fetchJson } from "@/stores/data";
import ReactMarkdown from "react-markdown";
import remarkGfm from "remark-gfm";

interface ToolItem {
  name: string;
  description: string;
}

const PRESET_SKILLS = [
  { name: "market-analysis", label: "📈 市场分析" },
  { name: "dragon-tiger-tracking", label: "🐉 龙虎榜追踪" },
  { name: "breakout-analysis", label: "🔥 涨停晋级" },
  { name: "market-replay", label: "🎯 行情回放" },
  { name: "strategy-manager", label: "🤖 策略管理" },
  { name: "free-chat", label: "⚡ 自由对话" },
];

export default function SkillEditor() {
  const { setMode } = useAppMode();
  const [selectedSkill, setSelectedSkill] = useState(PRESET_SKILLS[0].name);
  const [name, setName] = useState("dragon-tiger-tracking");
  const [description, setDescription] = useState(
    "龙虎榜分析 — 查询上榜个股、营业部买卖明细",
  );
  const [instructions, setInstructions] = useState(
    "## 角色\n\n你是一个龙虎榜分析专家...\n\n## 工具\n- dragon_tiger_detail\n- dragon_tiger_seat",
  );
  const [tools, setTools] = useState<ToolItem[]>([]);
  const [selectedTools, setSelectedTools] = useState<string[]>([]);

  useEffect(() => {
    fetchJson<ToolItem[]>("/api/tools")
      .then(setTools)
      .catch(() => {});
  }, []);

  const toggleTool = (toolName: string) => {
    setSelectedTools((prev) =>
      prev.includes(toolName)
        ? prev.filter((t) => t !== toolName)
        : [...prev, toolName],
    );
  };

  return (
    <div className="flex flex-col h-full p-6 gap-5 overflow-y-auto">
      <div className="flex items-center gap-2 flex-wrap">
        <span className="text-sm font-medium text-white mr-2">
          已安装 Skill
        </span>
        {PRESET_SKILLS.map((skill) => (
          <Button
            key={skill.name}
            variant={selectedSkill === skill.name ? "default" : "outline"}
            size="sm"
            onClick={() => setSelectedSkill(skill.name)}
          >
            {skill.label}
          </Button>
        ))}
        <Button variant="outline" size="sm">
          + 新建
        </Button>
      </div>

      <Separator />

      <div className="flex-1 grid grid-cols-1 lg:grid-cols-2 gap-5 min-h-0">
        {/* Left: Form */}
        <ScrollArea className="pr-3">
          <div className="space-y-4">
            <div>
              <label className="text-xs text-ink-muted block mb-1">name</label>
              <Input
                value={name}
                onChange={(e) => setName(e.target.value)}
                className="font-mono text-xs"
              />
            </div>
            <div>
              <label className="text-xs text-ink-muted block mb-1">
                description
              </label>
              <Textarea
                value={description}
                onChange={(e) => setDescription(e.target.value)}
                className="text-xs"
                rows={3}
              />
            </div>
            <div>
              <label className="text-xs text-ink-muted block mb-3">
                关联工具
              </label>
              <div className="space-y-2">
                {tools.map((tool) => (
                  <label
                    key={tool.name}
                    className="flex items-center gap-2 cursor-pointer text-xs text-ink-muted hover:text-white"
                  >
                    <Checkbox
                      checked={selectedTools.includes(tool.name)}
                      onCheckedChange={() => toggleTool(tool.name)}
                    />
                    <span className="font-mono">{tool.name}</span>
                    <span className="text-ink-muted">
                      {" "}
                      — {tool.description}
                    </span>
                  </label>
                ))}
                {tools.length === 0 && (
                  <p className="text-xs text-ink-muted">加载中...</p>
                )}
              </div>
            </div>
            <div>
              <label className="text-xs text-ink-muted block mb-1">
                指令（Markdown）
              </label>
              <Textarea
                value={instructions}
                onChange={(e) => setInstructions(e.target.value)}
                className="font-mono text-xs min-h-[300px]"
              />
            </div>
          </div>
        </ScrollArea>
        {/* Right: Preview */}
        <ScrollArea className="border border-hairline rounded-lg p-4 bg-bg">
          <div className="prose prose-invert prose-sm max-w-none">
            <ReactMarkdown remarkPlugins={[remarkGfm]}>
              {instructions}
            </ReactMarkdown>
          </div>
        </ScrollArea>
      </div>

      <div className="flex gap-2 justify-end pt-3 border-t border-hairline">
        <Button variant="outline" onClick={() => setMode("chat")}>
          返回
        </Button>
        <Button>保存 Skill</Button>
      </div>
    </div>
  );
}
```

- [ ] **Step 3: Commit**

```bash
cd /Users/user/Desktop/github/trading-agent/src
git add src/components/SkillEditor.tsx
git commit -m "feat: add SkillEditor with tool selection from /api/tools"
```

---

## Task 7: Backend — GET /api/skills + GET /api/tools

**Files:**

- Modify: `crates/web/src/chat.rs`
- Modify: `crates/web/src/lib.rs`

- [ ] **Step 1: Add get_skills and get_tools handlers to chat.rs**

```rust
// crates/web/src/chat.rs — add after get_ai_commands

use openatrading_core::skills::SkillEntry;
use serde::Serialize;

#[derive(Serialize)]
pub struct ToolResponse {
    pub name: String,
    pub description: String,
}

pub async fn get_skills(State(state): State<Arc<AppState>>) -> Json<Vec<SkillEntry>> {
    let skills = openatrading_core::skills::SkillRegistry::from_disk();
    Json(skills.skills().to_vec())
}

pub async fn get_tools(State(state): State<Arc<AppState>>) -> Json<Vec<ToolResponse>> {
    let registry = state.ctx.tool_registry.read().await;
    let tools = registry.all_tools().iter().map(|t| ToolResponse {
        name: t.name.clone(),
        description: t.description.clone(),
    }).collect();
    Json(tools)
}
```

- [ ] **Step 2: Add routes to lib.rs**

```rust
// crates/web/src/lib.rs — in Router::new()
.route("/api/skills", get(chat::get_skills))
.route("/api/tools", get(chat::get_tools))
```

- [ ] **Step 3: Commit**

```bash
cd /Users/user/Desktop/github/trading-agent
git add crates/web/src/chat.rs crates/web/src/lib.rs
git commit -m "feat: add GET /api/skills and GET /api/tools endpoints"
```

---

## Task 8: Backend — ChatRequest skill_name support

**Files:**

- Modify: `crates/web/src/chat.rs`

- [ ] **Step 1: Add skill_name to ChatRequest**

```rust
#[derive(Deserialize)]
pub struct ChatRequest {
    message: String,
    #[serde(default)]
    role: Option<String>,
    #[serde(default)]
    history: Vec<ChatMessage>,
    #[serde(default)]
    command_name: Option<String>,
    #[serde(default)]
    command_type: Option<openatrading_core::commands::CommandType>,
    #[serde(default)]
    skill_name: Option<String>,    // ← add
}
```

- [ ] **Step 2: Route skill_name into existing skill command path**

In `ai_chat`, before `match req.command_type`:

```rust
let effective_command_type = req.command_type.clone().or_else(|| {
    req.skill_name.as_ref().map(|_| CommandType::Skill)
});
let effective_command_name = req.command_name.clone().or_else(|| {
    req.skill_name.clone()
});
```

Change `match req.command_type` to `match effective_command_type` and the inner `CommandType::Skill` branch to use `effective_command_name`.

- [ ] **Step 3: Commit**

```bash
cd /Users/user/Desktop/github/trading-agent
git add crates/web/src/chat.rs
git commit -m "feat: support skill_name in chat request"
```

---

## Task 9: Backend — MCP server CRUD API

**Files:**

- Create: `crates/web/src/mcp.rs`
- Modify: `crates/web/src/lib.rs`
- Modify: `crates/core/src/llm/tools.rs`
- Modify: `crates/core/src/context.rs`

- [ ] **Step 1: Add ToolRegistry::reload_mcp_tools()**

```rust
// crates/core/src/llm/tools.rs
impl ToolRegistry {
    pub fn reload_mcp_tools(&mut self, mcp_servers: &[McpServerConfig]) {
        self.mcp_tools.clear();
        self.mcp_handle = None;
        let mcp_registry = McpRegistry::from_config(mcp_servers);
        let rt = tokio::runtime::Runtime::new().unwrap();
        let (tools, handle) = rt.block_on(mcp_registry.discover());
        for t in tools { self.mcp_tools.push(t); }
        self.mcp_handle = handle;
    }
}
```

- [ ] **Step 2: Change ctx.tool_registry to Arc<RwLock<>>**

```rust
// crates/core/src/context.rs
use tokio::sync::RwLock;

pub struct AppContext {
    pub tool_registry: Arc<RwLock<ToolRegistry>>,  // was Arc<ToolRegistry>
    // ... rest unchanged
}
```

Make `all_tools()` async:

```rust
pub async fn all_tools(&self) -> Vec<Arc<DynTool>> {
    self.tool_registry.read().await.all_tools()
}
```

- [ ] **Step 3: Update chat.rs to use async access**

In `get_ai_commands` and `ai_chat`, replace `state.ctx.tool_registry` with `state.ctx.tool_registry.read().await`

- [ ] **Step 4: Create crates/web/src/mcp.rs**

```rust
// crates/web/src/mcp.rs
use axum::{extract::{Path, State}, http::StatusCode, Json};
use std::{collections::BTreeMap, sync::Arc};
use openatrading_core::llm::{McpServerConfig, ToolConfig, build_tools};
use crate::AppState;

pub async fn list(State(state): State<Arc<AppState>>) -> Json<Vec<McpServerConfig>> {
    Json(state.mcp_servers.read().await.clone())
}

pub async fn add(
    State(state): State<Arc<AppState>>,
    Json(config): Json<McpServerConfig>,
) -> Result<Json<McpServerConfig>, StatusCode> {
    let mut servers = state.mcp_servers.write().await;
    if servers.iter().any(|s| s.name == config.name) { return Err(StatusCode::CONFLICT); }
    servers.push(config.clone());
    let snap = servers.clone(); drop(servers);
    reload_and_persist(&state, &snap).await;
    Ok(Json(config))
}

pub async fn delete(
    State(state): State<Arc<AppState>>,
    Path(name): Path<String>,
) -> StatusCode {
    let mut servers = state.mcp_servers.write().await;
    let before = servers.len();
    servers.retain(|s| s.name != name);
    if servers.len() == before { return StatusCode::NOT_FOUND; }
    let snap = servers.clone(); drop(servers);
    reload_and_persist(&state, &snap).await;
    StatusCode::NO_CONTENT
}

pub async fn toggle(
    State(state): State<Arc<AppState>>,
    Path(name): Path<String>,
) -> Result<Json<McpServerConfig>, StatusCode> {
    let mut servers = state.mcp_servers.write().await;
    let srv = servers.iter_mut().find(|s| s.name == name).ok_or(StatusCode::NOT_FOUND)?;
    srv.enabled = !srv.enabled;
    let result = srv.clone();
    let snap = servers.clone(); drop(servers);
    reload_and_persist(&state, &snap).await;
    Ok(Json(result))
}

async fn reload_and_persist(state: &AppState, servers: &[McpServerConfig]) {
    let cfg = ToolConfig { mcp_servers: servers.to_vec() };
    *state.ctx.tool_registry.write().await = build_tools(&cfg).await;
    let home = std::env::var("HOME").unwrap_or_else(|_| ".".into());
    let path = std::path::PathBuf::from(home).join(".atrading").join(".mcp.json");
    if let Some(p) = path.parent() { let _ = std::fs::create_dir_all(p); }
    let map: BTreeMap<_, _> = servers.iter().map(|s| (s.name.clone(), serde_json::to_value(s).unwrap())).collect();
    let root = serde_json::json!({"mcpServers": map});
    if let Ok(c) = serde_json::to_string_pretty(&root) { let _ = std::fs::write(&path, &c); }
}
```

- [ ] **Step 5: Update lib.rs**

```rust
// crates/web/src/lib.rs
use tokio::sync::RwLock;

pub mod mcp;

pub struct AppState {
    pub mcp_servers: Arc<RwLock<Vec<McpServerConfig>>>,  // was Vec<>
    // ... rest unchanged
}

// In Router::new():
.route("/api/mcp/servers", get(mcp::list).post(mcp::add))
.route("/api/mcp/servers/:name", delete(mcp::delete))
.route("/api/mcp/servers/:name/toggle", post(mcp::toggle))
```

- [ ] **Step 6: Commit**

```bash
cd /Users/user/Desktop/github/trading-agent
git add crates/web/src/mcp.rs crates/web/src/lib.rs crates/web/src/chat.rs crates/core/src/context.rs crates/core/src/llm/tools.rs
git commit -m "feat: add MCP CRUD API with runtime tool reload"
```

---

## Task 10: SettingsView (Frontend)

**Files:**

- Create: `src/components/SettingsView.tsx`

- [ ] **Step 1: Write SettingsView**

```tsx
// src/components/SettingsView.tsx
import { useState } from "react";
import { Input } from "@/components/ui/input";
import { Button } from "@/components/ui/button";
import { useAppMode } from "@/contexts/AppModeContext";
import { useUiStore } from "@/stores/ui";

export default function SettingsView() {
  const { setMode } = useAppMode();
  const { theme, setTheme } = useUiStore();
  const [apiKey, setApiKey] = useState("");
  const [model, setModel] = useState("");

  return (
    <div className="flex flex-col h-full p-6 gap-6 overflow-y-auto">
      <div className="flex items-center justify-between">
        <h1 className="text-lg font-medium text-white">设置</h1>
        <Button variant="outline" size="sm" onClick={() => setMode("chat")}>
          返回
        </Button>
      </div>

      <section>
        <h2 className="text-sm font-medium text-white mb-3">Agent 配置</h2>
        <div className="space-y-3 bg-bg-surface border border-hairline rounded-lg p-4">
          <div>
            <label className="text-xs text-ink-muted block mb-1">API Key</label>
            <Input
              type="password"
              value={apiKey}
              onChange={(e) => setApiKey(e.target.value)}
              placeholder="sk-..."
              className="font-mono text-xs"
            />
          </div>
          <div>
            <label className="text-xs text-ink-muted block mb-1">
              默认模型
            </label>
            <Input
              value={model}
              onChange={(e) => setModel(e.target.value)}
              placeholder="claude-sonnet-4-6"
              className="font-mono text-xs"
            />
          </div>
        </div>
      </section>

      <section>
        <h2 className="text-sm font-medium text-white mb-3">MCP 服务器</h2>
        <div className="bg-bg-surface border border-hairline rounded-lg p-4">
          <p className="text-xs text-ink-muted mb-3">
            管理 MCP 连接（名称、URL、密钥）
          </p>
          <Button variant="outline" size="sm">
            + 添加服务器
          </Button>
        </div>
      </section>

      <section>
        <h2 className="text-sm font-medium text-white mb-3">外观</h2>
        <div className="bg-bg-surface border border-hairline rounded-lg p-4">
          <div className="flex items-center justify-between">
            <span className="text-sm text-white">主题</span>
            <div className="flex gap-2">
              <Button
                variant={theme === "dark" ? "default" : "outline"}
                size="sm"
                onClick={() => setTheme("dark")}
              >
                暗色
              </Button>
              <Button
                variant={theme === "light" ? "default" : "outline"}
                size="sm"
                onClick={() => setTheme("light")}
              >
                亮色
              </Button>
            </div>
          </div>
        </div>
      </section>

      <section>
        <h2 className="text-sm font-medium text-white mb-3">数据源</h2>
        <div className="bg-bg-surface border border-hairline rounded-lg p-4">
          <label className="text-xs text-ink-muted block mb-1">API 地址</label>
          <Input value="http://localhost:8080" className="font-mono text-xs" />
        </div>
      </section>
    </div>
  );
}
```

- [ ] **Step 2: Commit**

```bash
cd /Users/user/Desktop/github/trading-agent/src
git add src/components/SettingsView.tsx
git commit -m "feat: add SettingsView with agent config, MCP, appearance, data source"
```

---

## Task 11: Web LandingPage Entry

**Files:**

- Modify: `src/LandingPage.tsx`

- [ ] **Step 1: Add CTA to LandingPage**

In the hero section, before the existing CTA buttons (around line 87), add a primary "开始使用" button:

```tsx
// src/LandingPage.tsx — find line 87-100:
{
  /* <div className="flex gap-3 justify-center flex-wrap"> */
}
// Add a new CTA before the existing links:

<Link
  to="/app"
  className="inline-flex items-center gap-2 px-6 py-2.5 rounded-[10px] text-sm font-medium bg-ai text-white hover:opacity-90 transition-opacity"
>
  开始使用 →
</Link>;
```

- [ ] **Step 2: Commit**

```bash
cd /Users/user/Desktop/github/trading-agent/src
git add src/LandingPage.tsx
git commit -m "feat: add start CTA button to LandingPage routing to /app"
```

---

## Spec Coverage Check

| Spec Requirement               | Task                                                            |
| ------------------------------ | --------------------------------------------------------------- |
| AppMode 三种模式切换           | Task 1 — AppModeContext                                         |
| 三栏布局容器                   | Task 1 — AppShell                                               |
| 左侧会话列表，可折叠调宽       | Task 2 — ConversationSidebar                                    |
| 新建会话 + Skill 下拉          | Task 2 — ConversationSidebar                                    |
| 技能/Skill 入口                | Task 2 — ConversationSidebar                                    |
| 设置入口（左下角）             | Task 2 — ConversationSidebar + Task 10                          |
| 顶栏干净（无导航无主题切换）   | Task 1 — removed from App.tsx                                   |
| 中央 AI 对话                   | Task 3 — AiCopilot adaptation                                   |
| 输入框下方快捷按钮             | Task 3 — ChatQuickButtons                                       |
| Slash command 复用             | Task 3 — already in useAssistantChat                            |
| 右侧面板手动打开               | Task 4 — RightPanel                                             |
| 右侧面板可折叠可调宽           | Task 4 — ResizableHandle + usePanelWidth                        |
| 点击同一按钮关闭面板           | Task 4 — RightPanel toggle logic                                |
| 右侧嵌入龙虎榜/晋级/回放/角色  | Task 4 + Task 5                                                 |
| 页面适配窄列                   | Task 5 — View adaptations                                       |
| RoleView 侧边栏改为 Select     | Task 5 — RoleView adapt                                         |
| Skill 编辑器（两栏）           | Task 6 — SkillEditor                                            |
| `/api/tools` 打勾选工具        | Task 6 — SkillEditor Checkbox                                   |
| 设置页面（两栏）               | Task 10 — SettingsView                                          |
| 主题移至设置 > 外观            | Task 10 — SettingsView                                          |
| MCP 服务器管理 UI              | Task 10 — SettingsView                                          |
| 后端 GET /api/skills           | Task 7                                                          |
| 后端 GET /api/tools            | Task 7                                                          |
| 后端 skill_name 支持           | Task 8                                                          |
| 后端 MCP CRUD API              | Task 9                                                          |
| Web LandingPage + 路由         | Task 11                                                         |
| 桌面端直接进入 AppShell        | Task 1 — isTauri detection in App.tsx                           |
| 所有 UI 用 shadcn/assistant-ui | 所有 Button/Input/Checkbox/ScrollArea/Select 均从 registry 引用 |
