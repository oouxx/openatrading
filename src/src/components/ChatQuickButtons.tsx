export default function ChatQuickButtons() {
  const openView = (viewId: string) => {
    window.dispatchEvent(new CustomEvent("open-right-panel", { detail: viewId }))
  }

  return (
    <div className="border-hairline flex items-center gap-1 border-t px-4 py-1.5">
      <button
        className="rounded-card border-hairline text-ink-muted hover:bg-bg-hover flex h-6 items-center gap-1 border bg-transparent px-2 py-1 text-[11px] transition-colors hover:text-white"
        onClick={() => openView("dragon-tiger")}
      >
        龙虎榜
      </button>
      <button
        className="rounded-card border-hairline text-ink-muted hover:bg-bg-hover flex h-6 items-center gap-1 border bg-transparent px-2 py-1 text-[11px] transition-colors hover:text-white"
        onClick={() => openView("breakout")}
      >
        晋级分析
      </button>
      <button
        className="rounded-card border-hairline text-ink-muted hover:bg-bg-hover flex h-6 items-center gap-1 border bg-transparent px-2 py-1 text-[11px] transition-colors hover:text-white"
        onClick={() => openView("replay")}
      >
        行情回放
      </button>
      <button
        className="rounded-card border-hairline text-ink-muted hover:bg-bg-hover flex h-6 items-center gap-1 border bg-transparent px-2 py-1 text-[11px] transition-colors hover:text-white"
        onClick={() => openView("roles")}
      >
        角色策略
      </button>
    </div>
  )
}
