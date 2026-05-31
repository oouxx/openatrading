import { useState, useEffect, useMemo, useCallback, useRef } from 'react'
import ReactMarkdown from 'react-markdown'
import remarkGfm from 'remark-gfm'
import { useQueries } from '@tanstack/react-query'
import { fetchJson, type RoleEntry, type RolePosition, type ClosedPosition } from '@/stores/data'
import { roleKeys } from '@/hooks/query/queryKeys'
import { useRoleList, useRolePositions, useRoleHistory } from '@/hooks/query/useRoleQueries'
import { useCreateRole, useUpdateRoleStrategy, useDeleteRole, useResetRole } from '@/hooks/query/useRoleMutations'

function formatPct(v: number) {
  return (v >= 0 ? '+' : '') + (v * 100).toFixed(2) + '%'
}

function pctClass(v: number) {
  return v >= 0 ? 'text-up' : 'text-down'
}

// ── Sidebar ───────────────────────────────────────────

function Sidebar({
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
    <aside className="w-[260px] min-w-[260px] bg-bg-surface border-r border-hairline flex flex-col overflow-y-auto">
      <div className="px-4 py-4 border-b border-hairline flex items-center justify-between gap-2">
        <h2 className="text-[15px] font-medium text-white">角色列表</h2>
        <span className="text-[11px] text-ink-muted">{totalCount} 个</span>
      </div>
      <div className="px-4 py-2.5 border-b border-hairline">
        <input
          type="text"
          placeholder="搜索角色…"
          value={search}
          onChange={(e) => setSearch(e.target.value)}
          className="w-full px-2.5 py-1.5 rounded-lg border border-hairline text-xs bg-bg text-white outline-none focus:border-ai transition-colors"
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
              className={`flex items-center justify-between px-4 py-2.5 cursor-pointer transition-colors gap-2.5 border-l-[3px] ${
                selectedRole === role.name
                  ? 'bg-ai/10 border-l-ai'
                  : 'border-l-transparent hover:bg-bg-hover'
              }`}
              onClick={() => onSelect(role.name)}
            >
              <div className="flex-1 min-w-0">
                <div className="text-[13px] font-medium text-white truncate">{displayName}</div>
                <div className="text-[11px] text-ink-muted flex gap-2.5 mt-0.5">
                  <span>{count} 持仓</span>
                  <span className={`font-semibold font-mono text-[11px] ${pnl >= 0 ? 'text-down' : 'text-up'}`}>
                    {count > 0 ? `${pnl >= 0 ? '+' : ''}${pnl.toFixed(1)}%` : '空仓'}
                  </span>
                </div>
              </div>
              <button
                onClick={(e) => {
                  e.stopPropagation()
                  onDelete(role.name)
                }}
                className="opacity-0 w-6 h-6 rounded-md border-none bg-transparent text-ink-muted cursor-pointer flex items-center justify-center text-base hover:bg-up/10 hover:text-up transition-all flex-shrink-0"
                style={{ opacity: selectedRole === role.name ? 1 : undefined }}
              >
                ×
              </button>
            </div>
          )
        })}
        <button
          onClick={onCreate}
          className="w-full px-4 py-2.5 text-left text-xs text-ink-muted hover:text-ai hover:bg-bg-hover transition-colors cursor-pointer"
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
    <div className="grid grid-cols-2 md:grid-cols-4 gap-3">
      <StatCard label="当前持仓盈亏" value={`${currentPnl >= 0 ? '+' : ''}¥${Math.abs(currentPnl).toLocaleString(undefined, { minimumFractionDigits: 2, maximumFractionDigits: 2 })}`} positive={currentPnl >= 0} negative={currentPnl < 0} />
      <StatCard label="历史累计盈亏" value={`${histPnlAccurate >= 0 ? '+' : ''}¥${Math.abs(histPnlAccurate).toLocaleString(undefined, { minimumFractionDigits: 2, maximumFractionDigits: 2 })}`} positive={histPnlAccurate >= 0} negative={histPnlAccurate < 0} />
      <StatCard label="总交易次数" value={String(totalTrades)} />
      <StatCard label="胜率" value={`${winRate}%`} />
    </div>
  )
}

function StatCard({ label, value, positive, negative }: { label: string; value: string; positive?: boolean; negative?: boolean }) {
  return (
    <div className="bg-bg-surface border border-hairline rounded-[10px] px-4 py-3.5">
      <div className="text-[11px] text-ink-muted uppercase tracking-wider">{label}</div>
      <div className={`font-mono text-lg font-semibold mt-1 ${positive ? 'text-down' : negative ? 'text-up' : 'text-white'}`}>
        {value}
      </div>
    </div>
  )
}

// ── P&L Chart (SVG) ────────────────────────────────────

function PnlChart({ history }: { history: ClosedPosition[] }) {
  const sorted = [...history].sort((a, b) => a.entry_date.localeCompare(b.entry_date))
  if (sorted.length === 0) {
    return <div className="text-center py-12 text-xs text-ink-muted">暂无交易数据，无法生成盈利曲线</div>
  }
  let cumulative = 0
  const points: { date: string; value: number }[] = [{ date: sorted[0].entry_date, value: 0 }]
  sorted.forEach((t) => {
    cumulative += (t.exit_price - t.entry_price) * t.shares
    points.push({ date: t.exit_date, value: cumulative })
  })
  const values = points.map((p) => p.value)
  const minVal = Math.min(0, ...values) - 100
  const maxVal = Math.max(0, ...values) + 100
  const range = maxVal - minVal || 1
  const W = 700; const H = 260; const padL = 50; const padR = 20; const padT = 20; const padB = 30
  const plotW = W - padL - padR; const plotH = H - padT - padB
  const x = (i: number) => padL + (i / (points.length - 1)) * plotW
  const y = (v: number) => padT + (1 - (v - minVal) / range) * plotH
  const zeroY = y(0)

  let pathD = ''; let areaD = ''
  points.forEach((p, i) => {
    const cmd = i === 0 ? 'M' : 'L'
    const xv = x(i).toFixed(1); const yv = y(p.value).toFixed(1)
    pathD += `${cmd}${xv},${yv} `
    areaD += `${cmd}${xv},${yv} `
  })
  areaD += `L${x(points.length - 1).toFixed(1)},${zeroY} L${x(0).toFixed(1)},${zeroY} Z`

  const circles = points.filter((_p, idx) => idx > 0 || _p.value !== 0).map((p) => {
    const idx = points.indexOf(p)
    return `<circle cx="${x(idx).toFixed(1)}" cy="${y(p.value).toFixed(1)}" r="3" fill="var(--color-accent)" stroke="#fff" stroke-width="1.5"/>`
  }).join('')

  // Y labels
  let yLabels = ''
  for (let i = 0; i <= 4; i++) {
    const val = minVal + (range * i / 4)
    const ly = y(val)
    yLabels += `<text x="${padL - 8}" y="${ly + 3}" text-anchor="end" font-size="9" fill="#888780">¥${val.toFixed(0)}</text>`
    yLabels += `<line x1="${padL}" y1="${ly}" x2="${W - padR}" y2="${ly}" stroke="rgba(255,255,255,0.06)" stroke-width="0.5" stroke-dasharray="3,3"/>`
  }

  const labelStep = Math.max(1, Math.floor(points.length / 5))
  const xLabels = points.filter((_, i) => i % labelStep === 0 || i === points.length - 1).map((p) => {
    const idx = points.indexOf(p)
    return `<text x="${x(idx).toFixed(1)}" y="${H - 8}" text-anchor="middle" font-size="9" fill="#888780">${p.date.slice(5)}</text>`
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
  const [search, setSearch] = useState('')
  const [tab, setTab] = useState('strategy')
  const [holdingView, setHoldingView] = useState<'current' | 'history'>('current')
  const [showCreate, setShowCreate] = useState(false)
  const [showDeleteConfirm, setShowDeleteConfirm] = useState<string | null>(null)
  const [showResetConfirm, setShowResetConfirm] = useState<string | null>(null)
  const [newName, setNewName] = useState('')
  const [editStrategyText, setEditStrategyText] = useState('')
  const [saveFeedback, setSaveFeedback] = useState(false)
  const mdEditorRef = useRef<HTMLTextAreaElement>(null)

  const { data: roles = [] } = useRoleList()
  const { data: positions = [] } = useRolePositions(selectedRole ?? '')
  const { data: history = [] } = useRoleHistory(selectedRole ?? '')

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

  const rolePnl = useMemo(() => {
    const result: Record<string, number> = {}
    roles.forEach((r, i) => {
      const pos = positionQueries[i]?.data
      if (!pos?.length) { result[r.name] = 0; return }
      const totalValue = pos.reduce((s, p) => s + p.current_price * p.shares, 0)
      const totalCost = pos.reduce((s, p) => s + p.cost_price * p.shares, 0)
      result[r.name] = totalCost > 0 ? ((totalValue - totalCost) / totalCost) * 100 : 0
    })
    return result
  }, [roles, positionQueries])

  const rolePositionCounts = useMemo(() => {
    const result: Record<string, number> = {}
    roles.forEach((r, i) => { result[r.name] = positionQueries[i]?.data?.length || 0 })
    return result
  }, [roles, positionQueries])

  const createRole = useCallback(async () => {
    const name = newName.trim()
    if (!name) return
    const id = 'r_' + Date.now().toString(36)
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
    <div className="min-h-screen bg-bg pt-11 flex flex-col" style={{ height: 'calc(100vh - 44px)' }}>
      <div className="flex flex-1 overflow-hidden">
        {/* Sidebar */}
        <Sidebar
          roles={roles}
          selectedRole={selectedRole}
          onSelect={setSelectedRole}
          onDelete={(name) => setShowDeleteConfirm(name)}
          onCreate={() => setShowCreate(true)}
          rolePnl={rolePnl}
          rolePositionCounts={rolePositionCounts}
          search={search}
          setSearch={setSearch}
          totalCount={roles.length}
        />

        {/* Main Content */}
        <main className="flex-1 overflow-y-auto px-7 py-6 flex flex-col gap-5">
          {!selectedRole ? (
            <div className="flex-1 flex items-center justify-center text-xs text-ink-muted">
              ← 选择或创建一个角色
            </div>
          ) : (
            <>
              {/* Header + Tabs */}
              <div className="flex items-center justify-between flex-wrap gap-3">
                <h1 className="text-2xl font-medium text-white leading-tight">{currentRole?.display_name || currentRole?.name}</h1>
                <div className="flex gap-1 bg-bg-hover rounded-[10px] p-1">
                  {TABS.map((t) => (
                    <button
                      key={t.id}
                      onClick={() => setTab(t.id)}
                      className={`px-4 py-1.5 rounded-lg text-xs font-medium transition-colors cursor-pointer ${
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
                <div className="bg-bg-surface border border-hairline rounded-xl overflow-hidden">
                  <div className="px-5 py-3.5 border-b border-hairline flex items-center justify-between gap-3 flex-wrap">
                    <h3 className="text-base font-medium text-white">策略文档（Markdown）</h3>
                    <button
                      onClick={saveStrategy}
                      className={`inline-flex items-center gap-1.5 px-2.5 py-1 rounded-md text-[11px] font-medium cursor-pointer transition-colors ${
                        saveFeedback ? 'bg-down text-white border-none' : 'bg-ink-dim text-ink-muted hover:text-white border border-hairline'
                      }`}
                    >
                      {saveFeedback ? '✓ 已保存' : '保存策略'}
                    </button>
                  </div>
                  <div className="grid grid-cols-1 lg:grid-cols-2 gap-0" style={{ minHeight: 300 }}>
                    <textarea
                      ref={mdEditorRef}
                      value={editStrategyText}
                      onChange={(e) => setEditStrategyText(e.target.value)}
                      className="w-full min-h-[300px] p-5 border-0 border-r lg:border-r border-b lg:border-b-0 border-hairline font-mono text-xs leading-relaxed resize-y outline-none bg-bg text-white"
                      placeholder="在此编写策略文档，支持 Markdown 格式…"
                      style={{ background: 'rgb(var(--color-bg-rgb) / 0.8)' }}
                    />
                    <div className="p-5 overflow-y-auto min-h-[300px] text-[13px] leading-relaxed markdown-preview">
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
                <div className="bg-bg-surface border border-hairline rounded-xl overflow-hidden">
                  <div className="px-5 py-3.5 border-b border-hairline flex items-center justify-between gap-3 flex-wrap">
                    <h3 className="text-base font-medium text-white">持仓明细</h3>
                    <div className="flex gap-2">
                      <button
                        onClick={() => setHoldingView('current')}
                        className={`px-2.5 py-1 rounded-md text-[11px] font-medium cursor-pointer transition-colors ${
                          holdingView === 'current' ? 'bg-ai/20 text-ai' : 'text-ink-muted hover:text-white'
                        }`}
                      >
                        当前持仓
                      </button>
                      <button
                        onClick={() => setHoldingView('history')}
                        className={`px-2.5 py-1 rounded-md text-[11px] font-medium cursor-pointer transition-colors ${
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
                                  <Td bold>{p.stock_name}</Td>
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
                      ) : <div className="text-center py-12 text-xs text-ink-muted">空仓</div>
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
                                  <Td bold>{h.stock_name}</Td>
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
                      ) : <div className="text-center py-12 text-xs text-ink-muted">暂无历史持仓记录</div>
                    )}
                  </div>
                </div>
              )}

              {/* Tab: Performance */}
              {tab === 'performance' && (
                <div className="bg-bg-surface border border-hairline rounded-xl overflow-hidden">
                  <div className="px-5 py-3.5 border-b border-hairline">
                    <h3 className="text-base font-medium text-white">累计盈利曲线</h3>
                  </div>
                  <PnlChart history={history} />
                  <div className="flex gap-5 px-5 py-3 border-t border-hairline text-[11px] text-ink-muted">
                    <span className="flex items-center gap-1.5">
                      <span className="w-2 h-2 rounded-full inline-block bg-ai" />
                      累计盈亏
                    </span>
                    <span className="flex items-center gap-1.5">
                      <span className="w-2 h-2 rounded-full inline-block" style={{ background: 'oklch(70% 0.02 250)' }} />
                      基准线
                    </span>
                  </div>
                </div>
              )}

              {/* Action Buttons */}
              <div className="flex gap-2 mt-auto">
                <button
                  onClick={() => setShowResetConfirm(selectedRole)}
                  className="px-2.5 py-1 rounded-md border border-hairline text-[11px] text-ink-muted hover:text-up hover:border-up/30 transition-colors cursor-pointer"
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
          <div className="bg-bg-surface rounded-[14px] border border-hairline p-6 w-[380px] max-w-[90vw] shadow-xl" onClick={(e) => e.stopPropagation()}>
            <h3 className="text-lg font-medium text-white mb-4">新建角色</h3>
            <label className="text-xs text-ink-muted block mb-1">角色名称</label>
            <input
              type="text"
              value={newName}
              onChange={(e) => setNewName(e.target.value)}
              className="w-full px-3 py-2.5 rounded-lg border border-hairline text-sm bg-bg text-white outline-none focus:border-ai transition-colors mb-3"
              placeholder="例如：套利策略 v2"
              onKeyDown={(e) => e.key === 'Enter' && createRole()}
              autoFocus
            />
            <div className="flex justify-end gap-2 mt-4">
              <button onClick={() => setShowCreate(false)} className="px-3.5 py-1.5 rounded-lg bg-ink-dim text-xs text-ink-muted hover:text-white cursor-pointer transition-colors">取消</button>
              <button onClick={createRole} className="px-3.5 py-1.5 rounded-lg bg-ai text-white text-xs font-medium hover:opacity-90 cursor-pointer transition-opacity">创建</button>
            </div>
          </div>
        </div>
      )}

      {/* Delete Confirm */}
      {showDeleteConfirm && (
        <div className="fixed inset-0 z-50 flex items-center justify-center" style={{ background: 'rgba(0,0,0,0.3)' }} onClick={() => setShowDeleteConfirm(null)}>
          <div className="bg-bg-surface rounded-[14px] border border-hairline p-6 w-[360px] max-w-[90vw] shadow-xl" onClick={(e) => e.stopPropagation()}>
            <h3 className="text-lg font-medium text-white mb-2">删除角色</h3>
            <p className="text-xs text-ink-muted mb-4">确定删除此角色？此操作不可恢复。</p>
            <div className="flex justify-end gap-2">
              <button onClick={() => setShowDeleteConfirm(null)} className="px-3.5 py-1.5 rounded-lg bg-ink-dim text-xs text-ink-muted hover:text-white cursor-pointer transition-colors">取消</button>
              <button onClick={() => deleteRole(showDeleteConfirm)} className="px-3.5 py-1.5 rounded-lg border border-up text-xs text-up hover:bg-up hover:text-white cursor-pointer transition-colors">删除</button>
            </div>
          </div>
        </div>
      )}

      {/* Reset Confirm */}
      {showResetConfirm && (
        <div className="fixed inset-0 z-50 flex items-center justify-center" style={{ background: 'rgba(0,0,0,0.3)' }} onClick={() => setShowResetConfirm(null)}>
          <div className="bg-bg-surface rounded-[14px] border border-hairline p-6 w-[360px] max-w-[90vw] shadow-xl" onClick={(e) => e.stopPropagation()}>
            <h3 className="text-lg font-medium text-white mb-2">重置账户</h3>
            <p className="text-xs text-ink-muted mb-4">重置将清空该角色的所有持仓和历史记录。</p>
            <div className="flex justify-end gap-2">
              <button onClick={() => setShowResetConfirm(null)} className="px-3.5 py-1.5 rounded-lg bg-ink-dim text-xs text-ink-muted hover:text-white cursor-pointer transition-colors">取消</button>
              <button onClick={() => resetRole(showResetConfirm)} className="px-3.5 py-1.5 rounded-lg border border-up text-xs text-up hover:bg-up hover:text-white cursor-pointer transition-colors">重置</button>
            </div>
          </div>
        </div>
      )}
    </div>
  )
}

// ── Table helpers ──────────────────────────────────────

function Th({ children }: { children: React.ReactNode }) {
  return <th className="text-left px-3.5 py-2.5 font-medium text-[11px] uppercase tracking-wider text-ink-muted border-b border-hairline bg-ink-dim whitespace-nowrap">{children}</th>
}
function Td({ children, className, bold, muted, reason }: { children: React.ReactNode; className?: string; bold?: boolean; muted?: boolean; reason?: boolean }) {
  let cls = 'px-3.5 py-2.5 border-b border-hairline text-white whitespace-nowrap'
  if (bold) cls += ' font-semibold font-mono text-xs'
  if (muted) cls += ' text-[11px] text-ink-muted'
  if (reason) cls += ' max-w-[260px] whitespace-normal text-[11px] text-ink-muted leading-relaxed'
  if (className) cls += ' ' + className
  return <td className={cls}>{children}</td>
}
