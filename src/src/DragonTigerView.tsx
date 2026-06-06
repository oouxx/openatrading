import { useState, useEffect, useCallback } from "react";
import {
  useDragonTigerDetail,
  useDragonTigerStockSeat,
  type DragonTigerDetailItem,
} from "@/hooks/query/useDragonTiger";

function fmtCap(v: number): string {
  if (v >= 1e12) return (v / 1e12).toFixed(2) + "万亿";
  if (v >= 1e8) return (v / 1e8).toFixed(0) + "亿";
  return v.toFixed(0);
}

export default function DragonTigerView() {
  const [page, setPage] = useState(1);
  const pageSize = 20;
  const { data: resp, isLoading, error } = useDragonTigerDetail(page, pageSize);
  const [dirFilter, setDirFilter] = useState<"all" | "buy" | "sell">("all");
  const [search, setSearch] = useState("");
  const [selected, setSelected] = useState<DragonTigerDetailItem | null>(null);

  const { data: buySeats = [], isLoading: buyLoading } =
    useDragonTigerStockSeat(
      selected?.code ?? null,
      selected?.trade_date
        ? selected.trade_date.slice(0, 10).replace(/-/g, "")
        : null,
      "buy",
    );
  const { data: sellSeats = [], isLoading: sellLoading } =
    useDragonTigerStockSeat(
      selected?.code ?? null,
      selected?.trade_date
        ? selected.trade_date.slice(0, 10).replace(/-/g, "")
        : null,
      "sell",
    );

  useEffect(() => {
    const onKey = (e: KeyboardEvent) => {
      if (e.key === "Escape") setSelected(null);
    };
    document.addEventListener("keydown", onKey);
    return () => document.removeEventListener("keydown", onKey);
  }, []);

  const items = resp?.data ?? [];
  const total = resp?.total ?? 0;
  const totalPages = Math.max(1, Math.ceil(total / pageSize));

  const filtered = items.filter((s) => {
    if (dirFilter === "buy" && (s.billboard_net_amt ?? 0) <= 0) return false;
    if (dirFilter === "sell" && (s.billboard_net_amt ?? 0) >= 0) return false;
    if (search && !s.name.toLowerCase().includes(search.toLowerCase()) && !s.code.toLowerCase().includes(search.toLowerCase())) {
      return false;
    }
    return true;
  });

  const goPrev = useCallback(() => {
    setPage((p) => Math.max(1, p - 1));
    setSelected(null);
  }, []);
  const goNext = useCallback(() => {
    setPage((p) => Math.min(totalPages, p + 1));
    setSelected(null);
  }, [totalPages]);
  const goPage = useCallback((p: number) => {
    setPage(p);
    setSelected(null);
  }, []);

  if (isLoading) {
    return (
      <div
        className="min-h-screen bg-bg pt-11 flex items-center justify-center"
        style={{ height: "calc(100vh - 44px)" }}
      >
        <div className="flex items-center gap-3 text-xs text-ink-muted">
          <span className="w-2 h-2 rounded-full bg-ai animate-pulse" />
          加载龙虎榜数据...
        </div>
      </div>
    );
  }

  if (error) {
    return (
      <div
        className="min-h-screen bg-bg pt-11 flex items-center justify-center"
        style={{ height: "calc(100vh - 44px)" }}
      >
        <p className="text-xs text-ink-muted">{error.message}</p>
      </div>
    );
  }

  const stats = [
    {
      l: "上榜家数",
      v: String(total),
      hl: true,
    },
    {
      l: "总净买入",
      v: `${((resp?.total_net_amt ?? 0) / 1e8).toFixed(2)}亿`,
      cl:
        (resp?.total_net_amt ?? 0) >= 0
          ? "var(--color-up)"
          : "var(--color-down)",
    },
    {
      l: "净买入家数",
      v: String(resp?.net_buy_count ?? 0),
      s: `占比${total > 0 ? (((resp?.net_buy_count ?? 0) / total) * 100).toFixed(0) : 0}%`,
    },
    {
      l: "净卖出家数",
      v: String(resp?.net_sell_count ?? 0),
      s: `占比${total > 0 ? (((resp?.net_sell_count ?? 0) / total) * 100).toFixed(0) : 0}%`,
    },
  ];

  const th = {
    textAlign: "left" as const,
    padding: "9px 10px",
    fontSize: "0.66rem",
    fontWeight: 500,
    color: "var(--color-muted, #888780)",
    textTransform: "uppercase" as const,
    letterSpacing: "0.03em",
    borderBottom: "1px solid var(--color-border)",
    background: "rgb(var(--color-fg-rgb) / 0.04)",
    position: "sticky" as const,
    top: 0,
    zIndex: 1,
    whiteSpace: "nowrap" as const,
  };
  const td = {
    padding: "7px 10px",
    borderBottom: "1px solid rgb(var(--color-fg-rgb) / 0.04)",
    whiteSpace: "nowrap" as const,
  };

  return (
    <div className="min-h-screen bg-bg pt-11">
      <div className="max-w-[1500px] mx-auto p-6 w-full">
        <div className="grid grid-cols-2 md:grid-cols-4 gap-3 mb-[18px]">
          {stats.map((s) => (
            <div
              key={s.l}
              className={`bg-bg-surface border rounded-[10px] px-4 py-3.5 text-center ${s.hl ? "border-ai bg-ai/5" : "border-hairline"}`}
            >
              <div className="text-[11px] text-ink-muted uppercase tracking-wider mb-1">
                {s.l}
              </div>
              <div
                className="font-mono text-[1.4rem] font-semibold text-white"
                style={s.cl ? { color: s.cl } : undefined}
              >
                {s.v}
              </div>
              {s.s && (
                <div className="text-[10px] text-ink-muted mt-0.5">{s.s}</div>
              )}
            </div>
          ))}
        </div>

        <div className="grid grid-cols-1 md:grid-cols-[220px_1fr] gap-4 items-start">
          <div className="bg-bg-surface border border-hairline rounded-xl overflow-hidden md:order-none order-2">
            <div className="px-3.5 py-3 border-b border-hairline text-[13px] font-medium text-white">
              筛选
            </div>
            <div className="p-3">
              <div className="mb-3">
                <div className="text-[10px] font-semibold text-ink-muted uppercase tracking-wider mb-1.5">
                  搜索
                </div>
                <input
                  type="text"
                  placeholder="名称或代码…"
                  value={search}
                  onChange={(e) => setSearch(e.target.value)}
                  className="w-full px-2.5 py-1.5 rounded-lg border border-hairline text-xs bg-bg text-white outline-none focus:border-ai transition-colors"
                />
              </div>
              <div>
                <div className="text-[10px] font-semibold text-ink-muted uppercase tracking-wider mb-1.5">
                  净买卖
                </div>
                <div className="flex flex-wrap gap-1.5">
                  {(["all", "buy", "sell"] as const).map((d) => {
                    const labels = {
                      all: "全部",
                      buy: "净买入",
                      sell: "净卖出",
                    };
                    const activeCls =
                      d === "buy"
                        ? "bg-up border-up text-white"
                        : d === "sell"
                          ? "bg-down border-down text-white"
                          : "bg-ai border-ai text-white";
                    return (
                      <button
                        key={d}
                        onClick={() => setDirFilter(d)}
                        className={`px-3 py-1 rounded-2xl border text-[11px] cursor-pointer transition-colors ${dirFilter === d ? activeCls : "border-hairline text-ink-muted hover:text-white"}`}
                      >
                        {labels[d]}
                      </button>
                    );
                  })}
                </div>
              </div>
            </div>
          </div>

          <div className="bg-bg-surface border border-hairline rounded-xl overflow-hidden md:order-none order-1">
            <div className="px-4 py-2.5 border-b border-hairline flex items-center justify-between gap-2.5">
              <h3 className="text-[13px] font-medium text-white">
                龙虎榜上榜个股
              </h3>
              <span className="text-[11px] text-ink-muted">
                第{page}/{totalPages}页 · {total} 只
              </span>
            </div>
            <div className="overflow-auto" style={{ maxHeight: "62vh" }}>
              <table
                className="w-full border-collapse"
                style={{ fontSize: "0.77rem" }}
              >
                <thead>
                  <tr>
                    <th style={th}>名称</th>
                    <th style={th}>净买入(万)</th>
                    <th style={th}>涨跌幅</th>
                    <th style={th}>上榜原因</th>
                  </tr>
                </thead>
                <tbody>
                  {filtered.length === 0 ? (
                    <tr>
                      <td
                        colSpan={4}
                        style={{
                          textAlign: "center",
                          padding: "36px",
                          color: "var(--color-muted, #888780)",
                        }}
                      >
                        无匹配数据
                      </td>
                    </tr>
                  ) : (
                    filtered.map((s) => {
                      const up = (s.billboard_net_amt ?? 0) >= 0;
                      return (
                        <tr
                          key={s.code}
                          className={`cursor-pointer transition-colors [&>td]:hover:bg-bg-hover ${selected?.code === s.code ? "[&>td]:!bg-ai/10" : ""}`}
                          onClick={() =>
                            setSelected(
                              selected?.code === s.code ? null : s,
                            )
                          }
                        >
                          <td style={td}>
                            <span className="font-medium text-ai cursor-pointer hover:underline">
                              {s.name}
                            </span>
                            <span className="text-[11px] text-ink-muted ml-1">
                              {s.code}
                            </span>
                          </td>
                          <td style={td}>
                            <span
                              className={`font-mono font-semibold text-xs ${up ? "text-up" : "text-down"}`}
                            >
                              {up ? "+" : ""}
                              {((s.billboard_net_amt ?? 0) / 1e8).toFixed(2)}
                              亿
                            </span>
                          </td>
                          <td style={td}>
                            <span
                              className={`font-mono font-semibold text-xs ${(s.change_rate ?? 0) >= 0 ? "text-up" : "text-down"}`}
                            >
                              {(s.change_rate ?? 0) >= 0 ? "+" : ""}
                              {s.change_rate?.toFixed(2) ?? "—"}%
                            </span>
                          </td>
                          <td style={td}>
                            <span className="text-[10px] text-ink-muted max-w-[200px] inline-block whitespace-normal">
                              {s.explanation || "—"}
                            </span>
                          </td>
                        </tr>
                      );
                    })
                  )}
                </tbody>
              </table>
            </div>
            {totalPages > 1 && (
              <div className="flex items-center justify-center gap-2 px-4 py-3 border-t border-hairline">
                <button
                  onClick={goPrev}
                  disabled={page <= 1}
                  className="px-3 py-1 rounded-lg border border-hairline text-[11px] text-ink-muted hover:text-white disabled:opacity-30 disabled:cursor-not-allowed cursor-pointer transition-colors"
                >
                  上一页
                </button>
                {(() => {
                  const pages: (number | "...")[] = [];
                  const maxVisible = 5;
                  if (totalPages <= maxVisible + 2) {
                    for (let i = 1; i <= totalPages; i++) pages.push(i);
                  } else {
                    pages.push(1);
                    if (page > 3) pages.push("...");
                    const start = Math.max(2, page - 1);
                    const end = Math.min(totalPages - 1, page + 1);
                    for (let i = start; i <= end; i++) pages.push(i);
                    if (page < totalPages - 2) pages.push("...");
                    pages.push(totalPages);
                  }
                  return pages.map((p, i) =>
                    p === "..." ? (
                      <span key={`e${i}`} className="text-[11px] text-ink-muted px-1">
                        ...
                      </span>
                    ) : (
                      <button
                        key={p}
                        onClick={() => goPage(p)}
                        className={`px-3 py-1 rounded-lg border text-[11px] cursor-pointer transition-colors ${page === p ? "bg-ai border-ai text-white" : "border-hairline text-ink-muted hover:text-white"}`}
                      >
                        {p}
                      </button>
                    ),
                  );
                })()}
                <button
                  onClick={goNext}
                  disabled={page >= totalPages}
                  className="px-3 py-1 rounded-lg border border-hairline text-[11px] text-ink-muted hover:text-white disabled:opacity-30 disabled:cursor-not-allowed cursor-pointer transition-colors"
                >
                  下一页
                </button>
              </div>
            )}
          </div>
        </div>

        {/* Detail Modal */}
        {selected && (
          <div
            className="fixed inset-0 z-50 flex items-center justify-center p-5"
            style={{ background: "rgba(0,0,0,0.35)" }}
            onClick={(e) => {
              if (e.target === e.currentTarget) setSelected(null);
            }}
          >
            <div className="bg-bg-surface rounded-2xl border border-hairline w-[700px] max-w-[95vw] max-h-[90vh] flex flex-col shadow-2xl overflow-hidden">
              <div className="px-5 py-4 border-b border-hairline flex items-center justify-between flex-shrink-0">
                <span className="text-[15px] font-medium text-white">
                  {selected.name} ({selected.code})
                </span>
                <button
                  onClick={() => setSelected(null)}
                  className="text-xl text-ink-muted hover:text-white px-2 py-0.5 rounded-md cursor-pointer"
                >
                  ✕
                </button>
              </div>
              <div className="flex-1 overflow-y-auto p-5">
                {selected.explanation && (
                  <div className="mb-4">
                    <h4 className="text-[13px] font-medium text-white mb-2">
                      上榜原因
                    </h4>
                    <p className="text-xs text-ink-muted">
                      {selected.explanation}
                    </p>
                  </div>
                )}
                <div className="grid grid-cols-2 gap-2.5 mb-4">
                  {[
                    { l: "收盘价", v: selected.close_price?.toFixed(2) || "—" },
                    {
                      l: "涨跌幅",
                      v:
                        selected.change_rate != null
                          ? `${selected.change_rate.toFixed(2)}%`
                          : "—",
                      up: (selected.change_rate || 0) >= 0,
                    },
                    {
                      l: "换手率",
                      v:
                        selected.turnover_rate != null
                          ? `${selected.turnover_rate.toFixed(2)}%`
                          : "—",
                    },
                    {
                      l: "流通市值",
                      v: selected.free_market_cap
                        ? fmtCap(selected.free_market_cap)
                        : "—",
                    },
                  ].map((m) => (
                    <div
                      key={m.l}
                      className="bg-ink-dim rounded-lg px-3 py-2 text-center"
                    >
                      <div className="text-[10px] text-ink-muted">{m.l}</div>
                      <div
                        className={`font-mono text-xs font-semibold ${m.up ? "text-up" : (m as any).down ? "text-down" : "text-white"}`}
                      >
                        {m.v}
                      </div>
                    </div>
                  ))}
                </div>
                {buyLoading || sellLoading ? (
                  <div className="flex items-center justify-center py-8">
                    <span className="w-2 h-2 rounded-full bg-ai animate-pulse" />
                    <span className="ml-2 text-xs text-ink-muted">
                      加载席位数据...
                    </span>
                  </div>
                ) : (
                  <>
                    {buySeats.length > 0 && (
                      <div className="mb-4">
                        <h4 className="text-[13px] font-medium text-up mb-2">
                          买方席位 ({buySeats.length})
                        </h4>
                        <table className="w-full border-collapse text-xs">
                          <thead>
                            <tr>
                              <th style={th}>营业部</th>
                              <th style={th}>买入(万)</th>
                              <th style={th}>买入占比</th>
                            </tr>
                          </thead>
                          <tbody>
                            {buySeats.map((s, i) => (
                              <tr key={i}>
                                <td style={td}>
                                  {s.dept_name}
                                  {s.seat_type === "机构" ? (
                                    <span className="text-[10px] bg-up/10 text-up px-1.5 py-0.5 rounded ml-1">
                                      机构
                                    </span>
                                  ) : null}
                                </td>
                                <td
                                  style={{
                                    ...td,
                                    fontFamily: "JetBrains Mono, monospace",
                                    color: "var(--color-up)",
                                  }}
                                >
                                  {s.buy_amt
                                    ? (s.buy_amt / 1e4).toFixed(0)
                                    : "—"}
                                </td>
                                <td
                                  style={{
                                    ...td,
                                    fontFamily: "JetBrains Mono, monospace",
                                  }}
                                >
                                  {s.buy_ratio != null
                                    ? `${(s.buy_ratio * 100).toFixed(1)}%`
                                    : "—"}
                                </td>
                              </tr>
                            ))}
                          </tbody>
                        </table>
                      </div>
                    )}
                    {sellSeats.length > 0 && (
                      <div className="mb-4">
                        <h4 className="text-[13px] font-medium text-down mb-2">
                          卖方席位 ({sellSeats.length})
                        </h4>
                        <table className="w-full border-collapse text-xs">
                          <thead>
                            <tr>
                              <th style={th}>营业部</th>
                              <th style={th}>卖出(万)</th>
                              <th style={th}>卖出占比</th>
                            </tr>
                          </thead>
                          <tbody>
                            {sellSeats.map((s, i) => (
                              <tr key={i}>
                                <td style={td}>
                                  {s.dept_name}
                                  {s.seat_type === "机构" ? (
                                    <span className="text-[10px] bg-up/10 text-up px-1.5 py-0.5 rounded ml-1">
                                      机构
                                    </span>
                                  ) : null}
                                </td>
                                <td
                                  style={{
                                    ...td,
                                    fontFamily: "JetBrains Mono, monospace",
                                    color: "var(--color-down)",
                                  }}
                                >
                                  {s.sell_amt
                                    ? (s.sell_amt / 1e4).toFixed(0)
                                    : "—"}
                                </td>
                                <td
                                  style={{
                                    ...td,
                                    fontFamily: "JetBrains Mono, monospace",
                                  }}
                                >
                                  {s.sell_ratio != null
                                    ? `${(s.sell_ratio * 100).toFixed(1)}%`
                                    : "—"}
                                </td>
                              </tr>
                            ))}
                          </tbody>
                        </table>
                      </div>
                    )}
                  </>
                )}
                {selected.d1_chg != null && (
                  <div className="mb-4">
                    <h4 className="text-[13px] font-medium text-white mb-2">
                      上榜后表现
                    </h4>
                    <div className="grid grid-cols-4 gap-2">
                      {[
                        { l: "T+1", v: selected.d1_chg },
                        { l: "T+2", v: selected.d2_chg },
                        { l: "T+5", v: selected.d5_chg },
                        { l: "T+10", v: selected.d10_chg },
                      ].map((d, i) => {
                        const dv = d.v || 0;
                        return (
                          <div
                            key={i}
                            className="bg-ink-dim rounded-lg px-3 py-2 text-center"
                          >
                            <div className="text-[10px] text-ink-muted">
                              {d.l}
                            </div>
                            <div
                              className={`font-mono text-xs font-semibold ${dv >= 0 ? "text-up" : "text-down"}`}
                            >
                              {dv >= 0 ? "+" : ""}
                              {dv.toFixed(2)}%
                            </div>
                          </div>
                        );
                      })}
                    </div>
                  </div>
                )}
              </div>
            </div>
          </div>
        )}
      </div>
    </div>
  );
}
