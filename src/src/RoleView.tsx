import { useState, useEffect, useMemo, useCallback, useRef } from 'react'
import ReactMarkdown from 'react-markdown'
import remarkGfm from 'remark-gfm'
import { AnimatePresence } from 'framer-motion'
import { useQueries } from '@tanstack/react-query'
import { fetchJson, type RoleEntry, type RolePosition, type ClosedPosition, type PnlPoint } from '@/stores/data'
import { roleKeys } from '@/hooks/query/queryKeys'
import { useRoleList, useRolePositions, useRoleHistory, useRolePnlHistory } from '@/hooks/query/useRoleQueries'
import { useCreateRole, useUpdateRoleStrategy, useDeleteRole, useResetRole } from '@/hooks/query/useRoleMutations'
import KlineDrawer from '@/components/KlineDrawer'
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from "@/components/ui/select"

function formatPct(v: number) {
  return (v >= 0 ? '+' : '') + (v * 100).toFixed(2) + '%'
}

function pctClass(v: number) {
  return v >= 0 ? 'text-up' : 'text-down'
}

// ── Sidebar ───────────────────────────────────────────

function _Sidebar({
  roles, selectedRole, onSelect, onDelete, onCreate, rolePnl, rolePositionCounts,
  search, setSearch, totalCount,
}: {
  roles: RoleEntry[]
  selectedRole: string | null
  onSelect: (name: string) => void
  onDelete: (name: string) => void
  onCreate: () => void
  rolePnl: Record<string, number>
  rolePositionCounts: Record<string, number>
  search: string
  setSearch: (s: string) => void
  totalCount: number
}) {
  const filtered = roles.filter((r) => {
    const displayName = (r as any).display_name || r.name
    return displayName.toLowerCase().includes(search.toLowerCase())
  })

  return (
    <aside className="flex w-[260px] min-w-[260px] flex-col overflow-y-auto border-r border-hairline bg-bg-surface">
      <div className="flex items-center justify-between gap-2 border-b border-hairline p-4">
        <h2 className="text-[15px] font-medium text-white">角色列表</h2>
        <span className="text-[11px] text-ink-muted">{totalCount} 个</span>
      </div>
      <div className="border-b border-hairline px-4 py-2.5">
        <input
          type="text"
          placeholder="搜索角色…"
          value={search}
          onChange={(e) => setSearch(e.target.value)}
          className="w-full rounded-lg border border-hairline bg-bg px-2.5 py-1.5 text-xs text-white transition-colors outline-none focus:border-ai"
        />
      </div>
      <div className="flex-1 overflow-y-auto py-2">
        {filtered.map((role) => {
          const displayName = (role as any).display_name || role.name
          const pnl = rolePnl[role.name] || 0
          const count = rolePositionCounts[role.name] || 0
          return (
            <div
              key={role.name}
              className={`flex cursor-pointer items-center justify-between gap-2.5 border-l-[3px] px-4 py-2.5 transition-colors ${
                selectedRole === role.name
                  ? 'border-l-ai bg-ai/10'
                  : 'border-l-transparent hover:bg-bg-hover'
              }`}
              onClick={() => onSelect(role.name)}
            >
              <div className="min-w-0 flex-1">
                <div className="truncate text-[13px] font-medium text-white">{displayName}</div>
                <div className="mt-0.5 flex gap-2.5 text-[11px] text-ink-muted">
                  <span>{count} 持仓</span>
                  <span className={`font-mono text-[11px] font-semibold ${isNaN(pnl) ? 'text-ink-muted' : pnl > 0 ? 'text-up' : pnl < 0 ? 'text-down' : 'text-white'}`}>
                    {count > 0 ? (isNaN(pnl) ? 'N/A' : `${pnl >= 0 ? '+' : ''}${pnl.toFixed(1)}%`) : '空仓'}
                  </span>
                </div>
              </div>
              <button
                onClick={(e) => {
                  e.stopPropagation()
                  onDelete(role.name)
                }}
                className="flex size-6 flex-shrink-0 cursor-pointer items-center justify-center rounded-md border-none bg-transparent text-base text-ink-muted opacity-0 transition-all hover:bg-up/10 hover:text-up"
                style={{ opacity: selectedRole === role.name ? 1 : undefined }}
              >
                ×
              </button>
            </div>
          )
        })}
        <button
          onClick={onCreate}
          className="w-full cursor-pointer px-4 py-2.5 text-left text-xs text-ink-muted transition-colors hover:bg-bg-hover hover:text-ai"
        >
          + 新建角色
        </button>
      </div>
    </aside>
  )
}

// ── Stats Row ──────────────────────────────────────────

function StatsRow({ positions, history }: { positions: RolePosition[]; history: ClosedPosition[] }) {
  const currentPnl = positions.reduce((s, p) => s + (p.current_price - p.cost_price) * p.shares, 0)
  const histPnlAccurate = history.reduce((s, h) => s + (h.exit_price - h.entry_price) * h.shares, 0)
  const totalTrades = positions.length + history.length
  const winTrades = [...positions.filter(p => (p.current_price - p.cost_price) * p.shares > 0), ...history.filter(h => h.pnl_pct > 0)].length
  const winRate = totalTrades > 0 ? ((winTrades / totalTrades) * 100).toFixed(1) : '—'

  return (
    <div className="grid grid-cols-2 gap-3 md:grid-cols-4">
      <StatCard label="当前持仓盈亏" value={`${currentPnl >= 0 ? '+' : ''}¥${Math.abs(currentPnl).toLocaleString(undefined, { minimumFractionDigits: 2, maximumFractionDigits: 2 })}`} positive={currentPnl >= 0} negative={currentPnl < 0} />
      <StatCard label="历史累计盈亏" value={`${histPnlAccurate >= 0 ? '+' : ''}¥${Math.abs(histPnlAccurate).toLocaleString(undefined, { minimumFractionDigits: 2, maximumFractionDigits: 2 })}`} positive={histPnlAccurate >= 0} negative={histPnlAccurate < 0} />
      <StatCard label="总交易次数" value={String(totalTrades)} />
      <StatCard label="胜率" value={`${winRate}%`} />
    </div>
  )
}

function StatCard({ label, value, positive, negative }: { label: string; value: string; positive?: boolean; negative?: boolean }) {
  return (
    <div className="rounded-[10px] border border-hairline bg-bg-surface px-4 py-3.5">
      <div className="text-[11px] tracking-wider text-ink-muted uppercase">{label}</div>
      <div className={`mt-1 font-mono text-lg font-semibold ${positive ? 'text-down' : negative ? 'text-up' : 'text-white'}`}>
        {value}
      </div>
    </div>
  )
}

// ── P&L Chart (SVG) — uses backend pnl-history endpoint ──

function PnlChart({ points, initialCapital }: { points: PnlPoint[]; initialCapital: number }) {
  if (points.length === 0) {
    return <div className="py-12 text-center text-xs text-ink-muted">暂无交易数据，无法生成盈利曲线</div>
  }
  const W = 700; const H = 260; const padL = 50; const padR = 20; const padT = 20; const padB = 30
  const plotW = W - padL - padR; const plotH = H - padT - padB

  // Map each point to equity delta from initial capital
  const values = points.map((p) => (p.equity ?? initialCapital) - initialCapital)
  const minVal = Math.min(0, ...values) - 100
  const maxVal = Math.max(0, ...values) + 100
  const range = maxVal - minVal || 1
  const x = (i: number) => padL + (i / Math.max(points.length - 1, 1)) * plotW
  const y = (v: number) => padT + (1 - (v - minVal) / range) * plotH
  const zeroY = y(0)

  let pathD = ''; let areaD = ''
  points.forEach((p, i) => {
    const cmd = i === 0 ? 'M' : 'L'
    const val = (p.equity ?? initialCapital) - initialCapital
    const xv = x(i).toFixed(1); const yv = y(val).toFixed(1)
    pathD += `${cmd}${xv},${yv} `
    areaD += `${cmd}${xv},${yv} `
  })
  areaD += `L${x(points.length - 1).toFixed(1)},${zeroY} L${x(0).toFixed(1)},${zeroY} Z`

  // Y labels
  let yLabels = ''
  for (let i = 0; i <= 4; i++) {
    const val = minVal + (range * i / 4)
    const ly = y(val)
    yLabels += `<text x="${padL - 8}" y="${ly + 3}" text-anchor="end" font-size="9" fill="#888780">¥${val.toFixed(0)}</text>`
    yLabels += `<line x1="${padL}" y1="${ly}" x2="${W - padR}" y2="${ly}" stroke="rgba(255,255,255,0.06)" stroke-width="0.5" stroke-dasharray="3,3"/>`
  }

  // X labels — show MM-DD
  const labelStep = Math.max(1, Math.floor(points.length / 5))
  const xLabels = points.filter((_, i) => i % labelStep === 0 || i === points.length - 1).map((p) => {
    const idx = points.indexOf(p)
    return `<text x="${x(idx).toFixed(1)}" y="${H - 8}" text-anchor="middle" font-size="9" fill="#888780">${p.date.slice(5)}</text>`
  }).join('')

  // Highlight final point with a larger circle if it reflects live mark-to-market
  const circles = points.map((p, i) => {
    const isLast = i === points.length - 1
    const r = isLast ? "4.5" : "3"
    const fill = isLast ? "var(--color-accent)" : "var(--color-accent)"
    const stroke = isLast ? "#fff" : "#fff"
    const val = (p.equity ?? initialCapital) - initialCapital
    return `<circle cx="${x(i).toFixed(1)}" cy="${y(val).toFixed(1)}" r="${r}" fill="${fill}" stroke="${stroke}" stroke-width="1.5"/>`
  }).join('')

  return (
    <svg viewBox={`0 0 ${W} ${H}`} style={{ width: '100%', height: 280 }} dangerouslySetInnerHTML={{ __html: `
      ${yLabels}
      <line x1="${padL}" y1="${zeroY}" x2="${W - padR}" y2="${zeroY}" stroke="oklch(70%_0.02_250)" stroke-width="1" stroke-dasharray="6,4"/>
      <path d="${areaD}" fill="var(--color-accent)" opacity="0.08"/>
      <path d="${pathD}" fill="none" stroke="var(--color-accent)" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"/>
      ${circles}
      ${xLabels}
    `}} />
  )
}

// ── Main Component ────────────────────────────────────

const TABS = [
  { id: 'strategy', label: '策略文档' },
  { id: 'holdings', label: '持仓管理' },
  { id: 'performance', label: '盈利分析' },
]

export default function RoleView() {
  const [selectedRole, setSelectedRole] = useState<string | null>(null)
  const [_search, _setSearch] = useState('')
  const [tab, setTab] = useState('strategy')
  const [holdingView, setHoldingView] = useState<'current' | 'history'>('current')
  const [showCreate, setShowCreate] = useState(false)
  const [showDeleteConfirm, setShowDeleteConfirm] = useState<string | null>(null)
  const [showResetConfirm, setShowResetConfirm] = useState<string | null>(null)
  const [newName, setNewName] = useState('')
  const [editStrategyText, setEditStrategyText] = useState('')
  const [saveFeedback, setSaveFeedback] = useState(false)
  const [selectedStock, setSelectedStock] = useState<{ symbol: string; name: string } | null>(null)
  const mdEditorRef = useRef<HTMLTextAreaElement>(null)

  const { data: roles = [] } = useRoleList()
  const { data: positions = [] } = useRolePositions(selectedRole ?? '')
  const { data: history = [] } = useRoleHistory(selectedRole ?? '')
  const { data: pnlPoints = [] } = useRolePnlHistory(selectedRole ?? '')

  const createRoleMutation = useCreateRole()
  const updateStrategyMutation = useUpdateRoleStrategy()
  const deleteRoleMutation = useDeleteRole()
  const resetRoleMutation = useResetRole()

  // Fetch all roles' positions for sidebar PnL stats
  const positionQueries = useQueries({
    queries: roles.map((r) => ({
      queryKey: roleKeys.positions(r.name),
      queryFn: () => fetchJson<RolePosition[]>(`/api/roles/${r.name}/positions`),
      staleTime: 5 * 60 * 1000,
      enabled: !!r.name,
    })),
  })

  // Select first role on load
  useEffect(() => {
    if (roles.length > 0 && !selectedRole) {
      setSelectedRole(roles[0].name)
    }
  }, [roles])

  // Load strategy into editor when role changes
  useEffect(() => {
    if (selectedRole) {
      const role = roles.find((r) => r.name === selectedRole)
      if (role) setEditStrategyText(role.strategy)
    }
  }, [selectedRole, roles])

  const currentRole = roles.find((r) => r.name === selectedRole)

  const _rolePnl = useMemo(() => {
    const result: Record<string, number> = {}
    roles.forEach((r, i) => {
      const pos = positionQueries[i]?.data
      if (!pos?.length) { result[r.name] = 0; return }
      const totalValue = pos.reduce((s, p) => s + p.current_price * p.shares, 0)
      const totalCost = pos.reduce((s, p) => s + p.cost_price * p.shares, 0)
      // Only compute PnL % when cost basis is meaningful (>0)
      result[r.name] = totalCost > 0 ? ((totalValue - totalCost) / totalCost) * 100 : Number.NaN
    })
    return result
  }, [roles, positionQueries])

  const _rolePositionCounts = useMemo(() => {
    const result: Record<string, number> = {}
    roles.forEach((r, i) => { result[r.name] = positionQueries[i]?.data?.length || 0 })
    return result
  }, [roles, positionQueries])

  const createRole = useCallback(async () => {
    const name = newName.trim()
    if (!name) return
    const id = name
      .toLowerCase()
      .replace(/[^a-z0-9\u4e00-\u9fff_-]/g, '_')
      .replace(/_+/g, '_')
      .replace(/^_|_$/g, '')
      .slice(0, 64) || 'role_' + Date.now().toString(36)
    await createRoleMutation.mutateAsync({ name: id, displayName: name, strategy: `# ${name}\n\n在此编写策略文档…` })
    setShowCreate(false)
    setNewName('')
    setTimeout(() => setSelectedRole(id), 100)
  }, [newName, createRoleMutation])

  const deleteRole = useCallback(async (name: string) => {
    await deleteRoleMutation.mutateAsync(name)
    setShowDeleteConfirm(null)
    if (selectedRole === name) setSelectedRole(null)
  }, [selectedRole, deleteRoleMutation])

  const resetRole = useCallback(async (name: string) => {
    await resetRoleMutation.mutateAsync(name)
    setShowResetConfirm(null)
  }, [resetRoleMutation])

  const saveStrategy = useCallback(async () => {
    if (!selectedRole) return
    await updateStrategyMutation.mutateAsync({ name: selectedRole, strategy: editStrategyText })
    setSaveFeedback(true)
    setTimeout(() => setSaveFeedback(false), 1500)
  }, [selectedRole, editStrategyText, updateStrategyMutation])

  return (
    <div className="flex h-full flex-col">
      <div className="flex flex-1 overflow-hidden">
        {/* Sidebar replaced with Select at top for narrow right panel */}
        <div className="flex-shrink-0 border-b border-hairline px-4 py-3">
          <div className="flex items-center gap-2">
            <Select
              value={selectedRole || ""}
              onValueChange={(v) => {
                setSelectedRole(v);
                setShowCreate(v === "__create__");
              }}
            >
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
            <button
              onClick={() => setShowCreate(true)}
              className="flex-shrink-0 cursor-pointer rounded-lg bg-ai px-3 py-1.5 text-xs font-medium text-white transition-opacity hover:opacity-90"
            >
              + 新建
            </button>
          </div>
        </div>

        {/* Main Content */}
        <main className="flex flex-1 flex-col gap-5 overflow-y-auto px-7 py-6">
          {!selectedRole ? (
            <div className="flex flex-1 items-center justify-center text-xs text-ink-muted">
              ← 选择或创建一个角色
            </div>
          ) : (
            <>
              {/* Header + Tabs */}
              <div className="flex flex-wrap items-center justify-between gap-3">
                <h1 className="text-2xl leading-tight font-medium text-white">{currentRole?.display_name || currentRole?.name}</h1>
                <div className="flex gap-1 rounded-[10px] bg-bg-hover p-1">
                  {TABS.map((t) => (
                    <button
                      key={t.id}
                      onClick={() => setTab(t.id)}
                      className={`cursor-pointer rounded-lg px-4 py-1.5 text-xs font-medium transition-colors ${
                        tab === t.id ? 'bg-bg-surface text-white shadow-sm' : 'text-ink-muted hover:text-white'
                      }`}
                    >
                      {t.label}
                    </button>
                  ))}
                </div>
              </div>

              {/* Stats */}
              <StatsRow positions={positions} history={history} />

              {/* Tab: Strategy */}
              {tab === 'strategy' && (
                <div className="overflow-hidden rounded-xl border border-hairline bg-bg-surface">
                  <div className="flex flex-wrap items-center justify-between gap-3 border-b border-hairline px-5 py-3.5">
                    <h3 className="text-base font-medium text-white">策略文档（Markdown）</h3>
                    <button
                      onClick={saveStrategy} disabled={currentRole?.readonly}
                      className={`inline-flex cursor-pointer items-center gap-1.5 rounded-md px-2.5 py-1 text-[11px] font-medium transition-colors ${
                        saveFeedback ? 'border-none bg-down text-white' : (currentRole?.readonly ? 'cursor-not-allowed border border-hairline bg-ink-dim text-ink-muted opacity-40' : 'border border-hairline bg-ink-dim text-ink-muted hover:text-white')
                      }`}
                    >
                      {saveFeedback ? '✓ 已保存' : '保存策略'}
                    </button>
                  </div>
                  <div className="grid grid-cols-1 gap-0 lg:grid-cols-2" style={{ minHeight: 300 }}>
                    <textarea
                      ref={mdEditorRef}
                      value={editStrategyText}
                      onChange={(e) => setEditStrategyText(e.target.value)}
                      readOnly={currentRole?.readonly}
                      className="min-h-[300px] w-full resize-y border-0 border-r border-b border-hairline bg-bg p-5 font-mono text-xs leading-relaxed text-white outline-none lg:border-r lg:border-b-0"
                      placeholder="在此编写策略文档，支持 Markdown 格式…"
                      style={{ background: 'rgb(var(--color-bg-rgb) / 0.8)' }}
                    />
                    <div className="markdown-preview min-h-[300px] overflow-y-auto p-5 text-[13px] leading-relaxed">
                      {editStrategyText ? (
                        <ReactMarkdown remarkPlugins={[remarkGfm]}>{editStrategyText}</ReactMarkdown>
                      ) : (
                        <p className="text-ink-muted">预览区域</p>
                      )}
                    </div>
                  </div>
                </div>
              )}

              {/* Tab: Holdings */}
              {tab === 'holdings' && (
                <div className="overflow-hidden rounded-xl border border-hairline bg-bg-surface">
                  <div className="flex flex-wrap items-center justify-between gap-3 border-b border-hairline px-5 py-3.5">
                    <h3 className="text-base font-medium text-white">持仓明细</h3>
                    <div className="flex gap-2">
                      <button
                        onClick={() => setHoldingView('current')}
                        className={`cursor-pointer rounded-md px-2.5 py-1 text-[11px] font-medium transition-colors ${
                          holdingView === 'current' ? 'bg-ai/20 text-ai' : 'text-ink-muted hover:text-white'
                        }`}
                      >
                        当前持仓
                      </button>
                      <button
                        onClick={() => setHoldingView('history')}
                        className={`cursor-pointer rounded-md px-2.5 py-1 text-[11px] font-medium transition-colors ${
                          holdingView === 'history' ? 'bg-ai/20 text-ai' : 'text-ink-muted hover:text-white'
                        }`}
                      >
                        历史持仓
                      </button>
                    </div>
                  </div>
                  <div className="overflow-x-auto">
                    {holdingView === 'current' ? (
                      positions.length > 0 ? (
                        <table className="w-full border-collapse text-xs">
                          <thead>
                            <tr>
                              <Th>股票</Th><Th>成本</Th><Th>现价</Th><Th>盈亏</Th><Th>盈亏%</Th><Th>数量</Th><Th>入场日期</Th><Th>入场理由</Th>
                            </tr>
                          </thead>
                          <tbody>
                            {positions.map((p: RolePosition) => {
                              const pnlPct = (p.current_price - p.cost_price) / p.cost_price
                              const pnl = (p.current_price - p.cost_price) * p.shares
                              return (
                                <tr key={p.id}>
                                  <Td bold><button className="cursor-pointer text-left transition-colors hover:text-ai" onClick={() => setSelectedStock({ symbol: p.symbol, name: p.stock_name })}>{p.stock_name}</button></Td>
                                  <Td>{p.cost_price.toFixed(2)}</Td>
                                  <Td>{p.current_price.toFixed(2)}</Td>
                                  <Td className={pctClass(pnlPct)}>{pnl >= 0 ? '+' : ''}¥{Math.abs(pnl).toLocaleString(undefined, { maximumFractionDigits: 2 })}</Td>
                                  <Td className={pctClass(pnlPct)}>{formatPct(pnlPct)}</Td>
                                  <Td>{p.shares}</Td>
                                  <Td muted>{p.entry_date}</Td>
                                  <Td reason>{p.entry_reason}</Td>
                                </tr>
                              )
                            })}
                          </tbody>
                        </table>
                      ) : <div className="py-12 text-center text-xs text-ink-muted">空仓</div>
                    ) : (
                      history.length > 0 ? (
                        <table className="w-full border-collapse text-xs">
                          <thead>
                            <tr>
                              <Th>股票</Th><Th>入场价</Th><Th>出场价</Th><Th>盈亏</Th><Th>盈亏%</Th><Th>数量</Th><Th>持有期</Th><Th>出场理由</Th>
                            </tr>
                          </thead>
                          <tbody>
                            {history.map((h: ClosedPosition) => {
                              const pnl = (h.exit_price - h.entry_price) * h.shares
                              return (
                                <tr key={h.position_id}>
                                  <Td bold><button className="cursor-pointer text-left transition-colors hover:text-ai" onClick={() => setSelectedStock({ symbol: h.symbol, name: h.stock_name })}>{h.stock_name}</button></Td>
                                  <Td>{h.entry_price.toFixed(2)}</Td>
                                  <Td>{h.exit_price.toFixed(2)}</Td>
                                  <Td className={pctClass(h.pnl_pct)}>{pnl >= 0 ? '+' : ''}¥{Math.abs(pnl).toLocaleString(undefined, { maximumFractionDigits: 2 })}</Td>
                                  <Td className={pctClass(h.pnl_pct)}>{formatPct(h.pnl_pct)}</Td>
                                  <Td>{h.shares}</Td>
                                  <Td muted>{h.entry_date} → {h.exit_date}</Td>
                                  <Td reason>{h.exit_reason}</Td>
                                </tr>
                              )
                            })}
                          </tbody>
                        </table>
                      ) : <div className="py-12 text-center text-xs text-ink-muted">暂无历史持仓记录</div>
                    )}
                  </div>
                </div>
              )}

              {/* Tab: Performance */}
              {tab === 'performance' && (
                <div className="overflow-hidden rounded-xl border border-hairline bg-bg-surface">
                  <div className="border-b border-hairline px-5 py-3.5">
                    <h3 className="text-base font-medium text-white">累计盈利曲线</h3>
                  </div>
                  <PnlChart points={pnlPoints} initialCapital={1_000_000} />
                  <div className="flex gap-5 border-t border-hairline px-5 py-3 text-[11px] text-ink-muted">
                    <span className="flex items-center gap-1.5">
                      <span className="inline-block size-2 rounded-full bg-ai" />
                      累计盈亏
                    </span>
                    <span className="flex items-center gap-1.5">
                      <span className="inline-block size-2 rounded-full" style={{ background: 'oklch(70% 0.02 250)' }} />
                      基准线
                    </span>
                  </div>
                </div>
              )}

              {/* Action Buttons */}
              <div className="mt-auto flex gap-2">
                <button
                  onClick={() => setShowResetConfirm(selectedRole)}
                  className="cursor-pointer rounded-md border border-hairline px-2.5 py-1 text-[11px] text-ink-muted transition-colors hover:border-up/30 hover:text-up"
                >
                  重置账户
                </button>
              </div>
            </>
          )}
        </main>
      </div>

      {/* Create Modal */}
      {showCreate && (
        <div className="fixed inset-0 z-50 flex items-center justify-center" style={{ background: 'rgba(0,0,0,0.3)' }} onClick={() => setShowCreate(false)}>
          <div className="w-[380px] max-w-[90vw] rounded-[14px] border border-hairline bg-bg-surface p-6 shadow-xl" onClick={(e) => e.stopPropagation()}>
            <h3 className="mb-4 text-lg font-medium text-white">新建角色</h3>
            <label className="mb-1 block text-xs text-ink-muted">角色名称</label>
            <input
              type="text"
              value={newName}
              onChange={(e) => setNewName(e.target.value)}
              className="mb-3 w-full rounded-lg border border-hairline bg-bg px-3 py-2.5 text-sm text-white transition-colors outline-none focus:border-ai"
              placeholder="例如：套利策略 v2"
              onKeyDown={(e) => e.key === 'Enter' && createRole()}
              autoFocus
            />
            <div className="mt-4 flex justify-end gap-2">
              <button onClick={() => setShowCreate(false)} className="cursor-pointer rounded-lg bg-ink-dim px-3.5 py-1.5 text-xs text-ink-muted transition-colors hover:text-white">取消</button>
              <button onClick={createRole} className="cursor-pointer rounded-lg bg-ai px-3.5 py-1.5 text-xs font-medium text-white transition-opacity hover:opacity-90">创建</button>
            </div>
          </div>
        </div>
      )}

      {/* Delete Confirm */}
      {showDeleteConfirm && (
        <div className="fixed inset-0 z-50 flex items-center justify-center" style={{ background: 'rgba(0,0,0,0.3)' }} onClick={() => setShowDeleteConfirm(null)}>
          <div className="w-[360px] max-w-[90vw] rounded-[14px] border border-hairline bg-bg-surface p-6 shadow-xl" onClick={(e) => e.stopPropagation()}>
            <h3 className="mb-2 text-lg font-medium text-white">删除角色</h3>
            <p className="mb-4 text-xs text-ink-muted">确定删除此角色？此操作不可恢复。</p>
            <div className="flex justify-end gap-2">
              <button onClick={() => setShowDeleteConfirm(null)} className="cursor-pointer rounded-lg bg-ink-dim px-3.5 py-1.5 text-xs text-ink-muted transition-colors hover:text-white">取消</button>
              <button onClick={() => deleteRole(showDeleteConfirm)} className="cursor-pointer rounded-lg border border-up px-3.5 py-1.5 text-xs text-up transition-colors hover:bg-up hover:text-white">删除</button>
            </div>
          </div>
        </div>
      )}

      {/* Reset Confirm */}
      {showResetConfirm && (
        <div className="fixed inset-0 z-50 flex items-center justify-center" style={{ background: 'rgba(0,0,0,0.3)' }} onClick={() => setShowResetConfirm(null)}>
          <div className="w-[360px] max-w-[90vw] rounded-[14px] border border-hairline bg-bg-surface p-6 shadow-xl" onClick={(e) => e.stopPropagation()}>
            <h3 className="mb-2 text-lg font-medium text-white">重置账户</h3>
            <p className="mb-4 text-xs text-ink-muted">重置将清空该角色的所有持仓和历史记录。</p>
            <div className="flex justify-end gap-2">
              <button onClick={() => setShowResetConfirm(null)} className="cursor-pointer rounded-lg bg-ink-dim px-3.5 py-1.5 text-xs text-ink-muted transition-colors hover:text-white">取消</button>
              <button onClick={() => resetRole(showResetConfirm)} className="cursor-pointer rounded-lg border border-up px-3.5 py-1.5 text-xs text-up transition-colors hover:bg-up hover:text-white">重置</button>
            </div>
          </div>
        </div>
      )}

      {/* Stock Detail Drawer */}
      <AnimatePresence>
        {selectedStock && <KlineDrawer symbol={selectedStock.symbol} name={selectedStock.name} onClose={() => setSelectedStock(null)} />}
      </AnimatePresence>
    </div>
  )
}

// ── Table helpers ──────────────────────────────────────

function Th({ children }: { children: React.ReactNode }) {
  return <th className="border-b border-hairline bg-ink-dim px-3.5 py-2.5 text-left text-[11px] font-medium tracking-wider whitespace-nowrap text-ink-muted uppercase">{children}</th>
}
function Td({ children, className, bold, muted, reason }: { children: React.ReactNode; className?: string; bold?: boolean; muted?: boolean; reason?: boolean }) {
  let cls = 'px-3.5 py-2.5 border-b border-hairline text-white whitespace-nowrap'
  if (bold) cls += ' font-semibold font-mono text-xs'
  if (muted) cls += ' text-[11px] text-ink-muted'
  if (reason) cls += ' max-w-[260px] whitespace-normal text-[11px] text-ink-muted leading-relaxed'
  if (className) cls += ' ' + className
  return <td className={cls}>{children}</td>
}
