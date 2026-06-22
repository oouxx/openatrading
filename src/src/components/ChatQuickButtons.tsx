export default function ChatQuickButtons() {
  const openView = (viewId: string) => {
    window.dispatchEvent(
      new CustomEvent("open-right-panel", { detail: viewId }),
    );
  };

  return (
    <div className="flex items-center gap-1 px-4 py-1.5 border-t border-hairline">
      <button className="flex items-center gap-1 px-2 py-1 h-6 text-[11px] text-ink-muted border border-hairline rounded-card bg-transparent hover:bg-bg-hover hover:text-white transition-colors" onClick={() => openView("dragon-tiger")}>
        龙虎榜
      </button>
      <button className="flex items-center gap-1 px-2 py-1 h-6 text-[11px] text-ink-muted border border-hairline rounded-card bg-transparent hover:bg-bg-hover hover:text-white transition-colors" onClick={() => openView("breakout")}>
        晋级分析
      </button>
      <button className="flex items-center gap-1 px-2 py-1 h-6 text-[11px] text-ink-muted border border-hairline rounded-card bg-transparent hover:bg-bg-hover hover:text-white transition-colors" onClick={() => openView("replay")}>
        行情回放
      </button>
      <button className="flex items-center gap-1 px-2 py-1 h-6 text-[11px] text-ink-muted border border-hairline rounded-card bg-transparent hover:bg-bg-hover hover:text-white transition-colors" onClick={() => openView("roles")}>
        角色策略
      </button>
    </div>
  );
}
