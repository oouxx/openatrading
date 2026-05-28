import { useState, useEffect, useMemo, useCallback } from 'react'
import { motion, AnimatePresence } from 'framer-motion'
import { useDataStore, type RoleEntry, type RolePosition, type RoleLog, type ClosedPosition } from '@/stores/data'
import Sparkline from '@/components/Sparkline'

function formatPct(v: number) {
  return (v >= 0 ? '+' : '') + (v * 100).toFixed(2) + '%'
}

function pctClass(v: number) {
  return v >= 0 ? 'text-up' : 'text-down'
}

function formatMoney(v: number) {
  if (v >= 1e8) return (v / 1e8).toFixed(2) + '亿'
  if (v >= 1e4) return (v / 1e4).toFixed(2) + '万'
  return v.toFixed(0)
}

export default function RoleView() {
  const store = useDataStore()
  const [selectedRole, setSelectedRole] = useState<string | null>(null)
  const [showCreate, setShowCreate] = useState(false)
  const [newName, setNewName] = useState('')
  const [newDisplayName, setNewDisplayName] = useState('')
  const [newStrategy, setNewStrategy] = useState('')
  const [showDeleteConfirm, setShowDeleteConfirm] = useState<string | null>(null)
  const [showResetConfirm, setShowResetConfirm] = useState<string | null>(null)
  const [editingStrategy, setEditingStrategy] = useState(false)
  const [editStrategyText, setEditStrategyText] = useState('')
  const [pnlDays, setPnlDays] = useState(30)

  useEffect(() => {
    store.fetchRoles()
  }, [])

  const roles = store.roles.data

  useEffect(() => {
    if (selectedRole) {
      store.fetchRolePositions(selectedRole)
      store.fetchRoleLogs(selectedRole)
      store.fetchRolePnlHistory(selectedRole)
      store.fetchRoleHistory(selectedRole)
    }
  }, [selectedRole])

  const createRole = useCallback(async () => {
    if (!newName.trim()) return
    await store.createRole(newName.trim(), newDisplayName.trim() || newName.trim(), newStrategy.trim())
    setShowCreate(false)
    setNewName('')
    setNewDisplayName('')
    setNewStrategy('')
    store.fetchRoles()
  }, [newName, newDisplayName, newStrategy])

  const deleteRole = useCallback(async (name: string) => {
    await store.deleteRole(name)
    setShowDeleteConfirm(null)
    if (selectedRole === name) setSelectedRole(null)
    store.fetchRoles()
  }, [selectedRole])

  const resetRole = useCallback(async (name: string) => {
    await store.resetRole(name)
    setShowResetConfirm(null)
    if (selectedRole === name) {
      store.refreshRole(name)
    }
  }, [selectedRole])

  const saveStrategy = useCallback(async () => {
    if (!selectedRole) return
    await store.updateRoleStrategy(selectedRole, editStrategyText)
    setEditingStrategy(false)
    store.fetchRoles()
  }, [selectedRole, editStrategyText])

  const startEditStrategy = useCallback(() => {
    const role = roles.find((r) => r.name === selectedRole)
    if (role) {
      setEditStrategyText(role.strategy)
      setEditingStrategy(true)
    }
  }, [roles, selectedRole])

  const positions = selectedRole ? store.rolePositions[selectedRole]?.data || [] : []
  const logs = selectedRole ? store.roleLogs[selectedRole]?.data || [] : []
  const pnlHistory = selectedRole ? store.rolePnlHistory[selectedRole]?.data || [] : []
  const history = selectedRole ? store.roleHistory[selectedRole]?.data || [] : []

  const currentRole = useMemo(() => roles.find((r) => r.name === selectedRole), [roles, selectedRole])

  const rolePnl = useMemo(() => {
    const result: Record<string, number> = {}
    roles.forEach((r) => {
      const pos = store.rolePositions[r.name]?.data
      if (!pos?.length) { result[r.name] = 0; return }
      const totalValue = pos.reduce((s, p) => s + p.current_price * p.shares, 0)
      const totalCost = pos.reduce((s, p) => s + p.cost_price * p.shares, 0)
      result[r.name] = totalCost > 0 ? ((totalValue - totalCost) / totalCost) * 100 : 0
    })
    return result
  }, [roles, store.rolePositions])

  const rolePositionCounts = useMemo(() => {
    const result: Record<string, number> = {}
    roles.forEach((r) => {
      result[r.name] = store.rolePositions[r.name]?.data?.length || 0
    })
    return result
  }, [roles, store.rolePositions])

  // Pre-fetch data for all roles to show in sidebar
  useEffect(() => {
    roles.forEach((r) => {
      store.fetchRolePositions(r.name)
      store.fetchRolePnlHistory(r.name)
    })
  }, [roles.length])

  const filteredPnlHistory = useMemo(() => {
    if (pnlDays === 0) return pnlHistory
    return pnlHistory.slice(-pnlDays)
  }, [pnlHistory, pnlDays])

  const totalPnl = useMemo(() => {
    if (!positions.length) return 0
    return positions.reduce((sum, p) => {
      const pnl = (p.current_price - p.cost_price) * p.shares
      return sum + pnl
    }, 0)
  }, [positions])

  const totalValue = useMemo(() => {
    if (!positions.length) return 0
    return positions.reduce((sum, p) => sum + p.current_price * p.shares, 0)
  }, [positions])

  return (
    <div className="min-h-screen bg-bg pt-14">
      <div className="max-w-[1600px] mx-auto px-6 py-4">
        <div className="flex items-center justify-between mb-6">
          <h1 className="text-display-md text-white">角色策略</h1>
          <button
            onClick={() => setShowCreate(true)}
            className="px-4 py-2 bg-ai/20 text-ai rounded-button text-caption font-mono hover:bg-ai/30 transition-colors cursor-pointer"
          >
            + 新建角色
          </button>
        </div>

        <div className="flex gap-6">
          <div className="w-[320px] flex-shrink-0">
            <div className="bg-bg-surface rounded-card border border-hairline overflow-hidden">
              {store.roles.loading ? (
                <div className="flex items-center gap-2 px-4 py-8 justify-center">
                  <div className="w-2 h-2 rounded-full bg-ai animate-pulse" />
                  <span className="text-caption text-ink-muted">加载中...</span>
                </div>
              ) : store.roles.error ? (
                <div className="text-up text-caption px-4 py-8 text-center">{store.roles.error}</div>
              ) : (
                <>
              {roles.map((role: RoleEntry) => (
                <div
                  key={role.name}
                  className={`px-4 py-3 cursor-pointer transition-colors border-b border-hairline last:border-b-0 ${
                    selectedRole === role.name ? 'bg-ai/10 border-l-2 border-l-ai' : 'hover:bg-bg-hover'
                  }`}
                  onClick={() => setSelectedRole(role.name)}
                >
                  <div className="flex items-center justify-between mb-1">
                    <div className="flex items-center gap-2">
                      <span className="w-2 h-2 rounded-full bg-ai flex-shrink-0" />
                      <span className="text-sm text-white font-semibold truncate">{role.display_name}</span>
                    </div>
                  </div>
                  <div className="flex items-center justify-between text-caption">
                    <span className={`font-mono ${(rolePnl[role.name] || 0) >= 0 ? 'text-up' : 'text-down'}`}>
                      {rolePnl[role.name] !== undefined && rolePositionCounts[role.name] > 0
                        ? `${rolePnl[role.name] >= 0 ? '+' : ''}${rolePnl[role.name].toFixed(1)}%`
                        : '空仓'}
                    </span>
                    <span className="text-ink-muted">{rolePositionCounts[role.name] || 0}支</span>
                  </div>
                  <div className="mt-2 flex justify-center">
                    {store.rolePnlHistory[role.name]?.data?.length > 0 ? (
                      <Sparkline
                        points={store.rolePnlHistory[role.name].data}
                        width={220}
                        height={28}
                      />
                    ) : (
                      <span className="text-ink-muted text-xs">暂无数据</span>
                    )}
                  </div>
                </div>
              ))}
              <button
                onClick={() => setShowCreate(true)}
                className="w-full p-3 border border-dashed border-hairline text-ink-muted hover:text-ai hover:border-ai/30 transition-colors text-sm text-center cursor-pointer"
              >
                + 新增角色
              </button>
              </>
              )}
            </div>
          </div>

          <div className="flex-1 min-w-0">
            {!selectedRole ? (
              <div className="bg-bg-surface rounded-card border border-hairline p-12 text-center">
                <p className="text-ink-muted text-caption">← 选择一个角色查看详情</p>
              </div>
            ) : (
              <div className="space-y-4">
                {/* 操作栏 */}
                <div className="flex items-center justify-between">
                  <div>
                    <h2 className="text-xl font-bold text-white">{currentRole?.display_name}</h2>
                    <div className="flex items-center gap-4 mt-1 text-caption text-ink-muted">
                      <span>累计 <span className={`font-mono ${(rolePnl[selectedRole] || 0) >= 0 ? 'text-up' : 'text-down'}`}>
                        {(rolePnl[selectedRole] || 0) >= 0 ? '+' : ''}{(rolePnl[selectedRole] || 0).toFixed(1)}%
                      </span></span>
                      <span>持仓 {rolePositionCounts[selectedRole] || 0} 支</span>
                      <span>初始资金 ¥1,000,000</span>
                    </div>
                  </div>
                  <div className="flex gap-2">
                    <button
                      onClick={() => editingStrategy ? setEditingStrategy(false) : startEditStrategy()}
                      className="px-3 py-1.5 border border-hairline rounded text-sm text-ink-muted hover:text-white hover:border-white/20 transition-colors cursor-pointer"
                    >
                      {editingStrategy ? '取消编辑' : '编辑策略'}
                    </button>
                    <button
                      onClick={() => setShowResetConfirm(selectedRole)}
                      className="px-3 py-1.5 border border-hairline rounded text-sm text-ink-muted hover:text-up hover:border-up/30 transition-colors cursor-pointer"
                    >
                      重置账户
                    </button>
                    <button
                      onClick={() => setShowDeleteConfirm(selectedRole)}
                      className="px-3 py-1.5 border border-hairline rounded text-sm text-ink-muted hover:text-up hover:border-up/30 transition-colors cursor-pointer"
                    >
                      删除角色
                    </button>
                  </div>
                </div>

                {/* 盈利曲线 */}
                <div className="bg-bg-surface border border-hairline rounded-card p-5">
                  <div className="flex items-center justify-between mb-3">
                    <h3 className="text-white font-semibold text-sm">盈利曲线</h3>
                    <div className="flex gap-1">
                      {[7, 30, 0].map((d) => (
                        <button
                          key={d}
                          onClick={() => {
                            setPnlDays(d)
                            if (selectedRole) store.fetchRolePnlHistory(selectedRole, d || 30)
                          }}
                          className={`px-2 py-0.5 rounded text-xs transition-colors cursor-pointer ${
                            pnlDays === d ? 'bg-ai/20 text-ai' : 'text-ink-muted hover:text-white'
                          }`}
                        >
                          {d === 0 ? '全部' : d + '日'}
                        </button>
                      ))}
                    </div>
                  </div>
                  <Sparkline points={filteredPnlHistory} width={600} height={60} />
                </div>

                {/* 编辑策略 */}
                {editingStrategy && (
                  <div className="bg-bg-surface border border-ai/30 rounded-card p-5">
                    <textarea
                      value={editStrategyText}
                      onChange={(e) => setEditStrategyText(e.target.value)}
                      className="w-full bg-bg border border-hairline rounded px-3 py-2 text-caption text-white font-mono outline-none focus:border-ai/50 resize-y"
                      rows={8}
                    />
                    <div className="flex justify-end gap-2 mt-3">
                      <button
                        onClick={() => setEditingStrategy(false)}
                        className="px-4 py-1.5 text-caption text-ink-muted hover:text-white transition-colors cursor-pointer"
                      >
                        取消
                      </button>
                      <button
                        onClick={saveStrategy}
                        className="px-4 py-1.5 bg-ai/20 text-ai rounded text-sm hover:bg-ai/30 transition-colors cursor-pointer"
                      >
                        保存策略
                      </button>
                    </div>
                  </div>
                )}

                <div className="grid grid-cols-3 gap-4">
                  <div className="bg-bg-surface rounded-card border border-hairline p-card-inner">
                    <p className="text-caption text-ink-muted mb-2">持仓市值</p>
                    <p className="font-mono text-lg text-white">{formatMoney(totalValue)}</p>
                  </div>
                  <div className="bg-bg-surface rounded-card border border-hairline p-card-inner">
                    <p className="text-caption text-ink-muted mb-2">浮动盈亏</p>
                    <p className={`font-mono text-lg ${pctClass(totalPnl)}`}>{totalPnl >= 0 ? '+' : ''}{formatMoney(totalPnl)}</p>
                  </div>
                  <div className="bg-bg-surface rounded-card border border-hairline p-card-inner">
                    <p className="text-caption text-ink-muted mb-2">净值曲线</p>
                    {pnlHistory.length > 0 ? (
                      <Sparkline points={pnlHistory} width={200} height={32} />
                    ) : (
                      <p className="text-caption text-ink-muted">暂无数据</p>
                    )}
                  </div>
                </div>

                <div className="bg-bg-surface rounded-card border border-hairline overflow-hidden">
                  <div className="px-4 py-3 border-b border-hairline">
                    <span className="text-caption-strong text-ink-muted font-mono uppercase tracking-wider">
                      当前持仓 · {positions.length}
                    </span>
                  </div>
                  {positions.length > 0 ? (
                    <div className="overflow-x-auto">
                      <table className="data-table">
                        <thead>
                          <tr>
                            <th>股票</th>
                            <th>成本</th>
                            <th>现价</th>
                            <th>盈亏</th>
                            <th>数量</th>
                            <th>入场日期</th>
                            <th>入场理由</th>
                          </tr>
                        </thead>
                        <tbody>
                          {positions.map((p: RolePosition) => {
                            const pnlPct = (p.current_price - p.cost_price) / p.cost_price
                            return (
                              <tr key={p.id}>
                                <td>
                                  <span className="text-white">{p.stock_name}</span>
                                  <span className="text-ink-muted ml-2 text-xs">{p.symbol.replace(/\.[A-Z]+$/, '')}</span>
                                </td>
                                <td>{p.cost_price.toFixed(2)}</td>
                                <td className="text-white">{p.current_price.toFixed(2)}</td>
                                <td className={pctClass(pnlPct)}>{formatPct(pnlPct)}</td>
                                <td>{p.shares}</td>
                                <td className="text-ink-muted">{p.entry_date}</td>
                                <td className="max-w-[200px] truncate text-ink-muted text-xs">{p.entry_reason}</td>
                              </tr>
                            )
                          })}
                        </tbody>
                      </table>
                    </div>
                  ) : (
                    <div className="p-6 text-center text-caption text-ink-muted">空仓</div>
                  )}
                </div>

                <div className="bg-bg-surface rounded-card border border-hairline overflow-hidden">
                  <div className="px-4 py-3 border-b border-hairline">
                    <span className="text-caption-strong text-ink-muted font-mono uppercase tracking-wider">
                      交易日志 · {logs.length}
                    </span>
                  </div>
                  {logs.length > 0 ? (
                    <div className="max-h-[300px] overflow-y-auto">
                      {logs.map((log: RoleLog, idx: number) => (
                        <div key={idx} className="px-4 py-2 border-b border-hairline-dark last:border-b-0">
                          <div className="flex items-center gap-3">
                            <span className="text-caption text-ink-muted font-mono">{log.date}</span>
                            <span className={`text-caption font-mono ${log.action === 'buy' ? 'text-up' : 'text-down'}`}>
                              {log.action === 'buy' ? '买入' : '卖出'}
                            </span>
                            <span className="text-caption text-white">{log.symbol.replace(/\.[A-Z]+$/, '')}</span>
                            <span className="text-caption font-mono text-ink-muted">@ {log.price.toFixed(2)} × {log.shares}</span>
                          </div>
                          <p className="text-caption text-ink-muted mt-1 text-xs">{log.reason}</p>
                        </div>
                      ))}
                    </div>
                  ) : (
                    <div className="p-6 text-center text-caption text-ink-muted">暂无交易记录</div>
                  )}
                </div>

                {history.length > 0 && (
                  <div className="bg-bg-surface rounded-card border border-hairline overflow-hidden">
                    <div className="px-4 py-3 border-b border-hairline">
                      <span className="text-caption-strong text-ink-muted font-mono uppercase tracking-wider">
                        已平仓 · {history.length}
                      </span>
                    </div>
                    <div className="overflow-x-auto">
                      <table className="data-table">
                        <thead>
                          <tr>
                            <th>股票</th>
                            <th>入场价</th>
                            <th>出场价</th>
                            <th>盈亏</th>
                            <th>持仓天数</th>
                            <th>出场理由</th>
                          </tr>
                        </thead>
                        <tbody>
                          {history.map((h: ClosedPosition) => (
                            <tr key={h.position_id}>
                              <td>
                                <span className="text-white">{h.stock_name}</span>
                              </td>
                              <td>{h.entry_price.toFixed(2)}</td>
                              <td>{h.exit_price.toFixed(2)}</td>
                              <td className={pctClass(h.pnl_pct)}>{formatPct(h.pnl_pct)}</td>
                              <td className="text-ink-muted">
                                {Math.ceil((new Date(h.exit_date).getTime() - new Date(h.entry_date).getTime()) / 86400000)}天
                              </td>
                              <td className="max-w-[200px] truncate text-ink-muted text-xs">{h.exit_reason}</td>
                            </tr>
                          ))}
                        </tbody>
                      </table>
                    </div>
                  </div>
                )}
              </div>
            )}
          </div>
        </div>
      </div>

      <AnimatePresence>
        {showCreate && (
          <>
            <div className="fixed inset-0 z-40" style={{ background: 'rgba(0,0,0,0.5)' }} onClick={() => setShowCreate(false)} />
            <motion.div
              className="fixed inset-0 z-50 flex items-center justify-center pointer-events-none"
              initial={{ opacity: 0, scale: 0.95 }}
              animate={{ opacity: 1, scale: 1 }}
              exit={{ opacity: 0, scale: 0.95 }}
              transition={{ duration: 0.2 }}
            >
              <div className="pointer-events-auto bg-bg-surface border border-hairline rounded-card p-6 w-[480px] max-w-[90vw]">
                <h2 className="text-lg font-bold text-white mb-4">新建角色</h2>
                <div className="space-y-3">
                  <div>
                    <label className="text-caption text-ink-muted block mb-1">角色标识（英文）</label>
                    <input
                      value={newName}
                      onChange={(e) => setNewName(e.target.value)}
                      className="w-full bg-bg border border-hairline rounded-input px-3 py-2 text-caption text-white outline-none focus:border-ai/50 font-mono"
                      placeholder="e.g. momentum_trader"
                    />
                  </div>
                  <div>
                    <label className="text-caption text-ink-muted block mb-1">显示名称</label>
                    <input
                      value={newDisplayName}
                      onChange={(e) => setNewDisplayName(e.target.value)}
                      className="w-full bg-bg border border-hairline rounded-input px-3 py-2 text-caption text-white outline-none focus:border-ai/50"
                      placeholder="e.g. 动量交易者"
                    />
                  </div>
                  <div>
                    <label className="text-caption text-ink-muted block mb-1">策略描述</label>
                    <textarea
                      value={newStrategy}
                      onChange={(e) => setNewStrategy(e.target.value)}
                      className="w-full bg-bg border border-hairline rounded-input px-3 py-2 text-caption text-white outline-none focus:border-ai/50 resize-y"
                      rows={4}
                      placeholder="描述该角色的交易策略..."
                    />
                  </div>
                </div>
                <div className="flex justify-end gap-3 mt-4">
                  <button
                    onClick={() => setShowCreate(false)}
                    className="px-4 py-2 text-caption text-ink-muted hover:text-white transition-colors cursor-pointer"
                  >
                    取消
                  </button>
                  <button
                    onClick={createRole}
                    className="px-4 py-2 bg-ai/20 text-ai rounded-button text-caption font-mono hover:bg-ai/30 transition-colors cursor-pointer"
                  >
                    创建
                  </button>
                </div>
              </div>
            </motion.div>
          </>
        )}
      </AnimatePresence>

      <AnimatePresence>
        {showDeleteConfirm && (
          <>
            <div className="fixed inset-0 z-40" style={{ background: 'rgba(0,0,0,0.5)' }} onClick={() => setShowDeleteConfirm(null)} />
            <motion.div
              className="fixed inset-0 z-50 flex items-center justify-center pointer-events-none"
              initial={{ opacity: 0, scale: 0.95 }}
              animate={{ opacity: 1, scale: 1 }}
              exit={{ opacity: 0, scale: 0.95 }}
              transition={{ duration: 0.2 }}
            >
              <div className="pointer-events-auto bg-bg-surface border border-hairline rounded-card p-6 w-[360px]">
                <h2 className="text-lg font-bold text-white mb-2">确认删除</h2>
                <p className="text-caption text-ink-muted mb-4">删除角色 <b className="text-white">{showDeleteConfirm}</b>？此操作不可撤销。</p>
                <div className="flex justify-end gap-3">
                  <button
                    onClick={() => setShowDeleteConfirm(null)}
                    className="px-4 py-2 text-caption text-ink-muted hover:text-white transition-colors cursor-pointer"
                  >
                    取消
                  </button>
                  <button
                    onClick={() => deleteRole(showDeleteConfirm)}
                    className="px-4 py-2 bg-up/20 text-up rounded-button text-caption font-mono hover:bg-up/30 transition-colors cursor-pointer"
                  >
                    删除
                  </button>
                </div>
              </div>
            </motion.div>
          </>
        )}
      </AnimatePresence>

      <AnimatePresence>
        {showResetConfirm && (
          <>
            <div className="fixed inset-0 z-40" style={{ background: 'rgba(0,0,0,0.5)' }} onClick={() => setShowResetConfirm(null)} />
            <motion.div
              className="fixed inset-0 z-50 flex items-center justify-center pointer-events-none"
              initial={{ opacity: 0, scale: 0.95 }}
              animate={{ opacity: 1, scale: 1 }}
              exit={{ opacity: 0, scale: 0.95 }}
              transition={{ duration: 0.2 }}
            >
              <div className="pointer-events-auto bg-bg-surface border border-hairline rounded-card p-6 w-[360px]">
                <h2 className="text-lg font-bold text-white mb-2">确认重置</h2>
                <p className="text-caption text-ink-muted mb-4">重置角色 <b className="text-white">{showResetConfirm}</b>？此操作将清空该角色的所有持仓和历史记录。</p>
                <div className="flex justify-end gap-3">
                  <button
                    onClick={() => setShowResetConfirm(null)}
                    className="px-4 py-2 text-caption text-ink-muted hover:text-white transition-colors cursor-pointer"
                  >
                    取消
                  </button>
                  <button
                    onClick={() => resetRole(showResetConfirm)}
                    className="px-4 py-2 bg-up/20 text-up rounded-button text-caption font-mono hover:bg-up/30 transition-colors cursor-pointer"
                  >
                    重置
                  </button>
                </div>
              </div>
            </motion.div>
          </>
        )}
      </AnimatePresence>
    </div>
  )
}