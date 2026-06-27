export default function ChatQuickButtons() {
  const openView = (viewId: string) => {
    window.dispatchEvent(
      new CustomEvent("open-right-panel", { detail: viewId }),
    );
  };

  return (
    <div className="flex items-center gap-1 border-t border-hairline px-4 py-1.5">
      <button className="flex h-6 items-center gap-1 rounded-card border border-hairline bg-transparent px-2 py-1 text-[11px] text-ink-muted transition-colors hover:bg-bg-hover hover:text-white" onClick={() => openView("dragon-tiger")}>
        龙虎榜
      </button>
      <button className="flex h-6 items-center gap-1 rounded-card border border-hairline bg-transparent px-2 py-1 text-[11px] text-ink-muted transition-colors hover:bg-bg-hover hover:text-white" onClick={() => openView("breakout")}>
        晋级分析
      </button>
      <button className="flex h-6 items-center gap-1 rounded-card border border-hairline bg-transparent px-2 py-1 text-[11px] text-ink-muted transition-colors hover:bg-bg-hover hover:text-white" onClick={() => openView("replay")}>
        行情回放
      </button>
      <button className="flex h-6 items-center gap-1 rounded-card border border-hairline bg-transparent px-2 py-1 text-[11px] text-ink-muted transition-colors hover:bg-bg-hover hover:text-white" onClick={() => openView("roles")}>
        角色策略
      </button>
    </div>
  );
}
