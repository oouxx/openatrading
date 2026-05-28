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

  const positions = selectedRole ? store.rolePositions[selectedRole]?.data || [] : []
  const logs = selectedRole ? store.roleLogs[selectedRole]?.data || [] : []
  const pnlHistory = selectedRole ? store.rolePnlHistory[selectedRole]?.data || [] : []
  const history = selectedRole ? store.roleHistory[selectedRole]?.data || [] : []

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
          <div className="w-[260px] flex-shrink-0">
            <div className="bg-bg-surface rounded-card border border-hairline overflow-hidden">
              {roles.map((role: RoleEntry) => (
                <div
                  key={role.name}
                  className={`px-4 py-3 cursor-pointer transition-colors border-b border-hairline last:border-b-0 ${
                    selectedRole === role.name ? 'bg-ai/10 border-l-2 border-l-ai' : 'hover:bg-bg-hover'
                  }`}
                  onClick={() => setSelectedRole(role.name)}
                >
                  <div className="flex items-center justify-between">
                    <span className="text-sm text-white font-semibold">{role.display_name}</span>
                    <button
                      onClick={(e) => {
                        e.stopPropagation()
                        setShowDeleteConfirm(role.name)
                      }}
                      className="text-ink-muted hover:text-up text-xs opacity-0 group-hover:opacity-100 transition-opacity"
                    >
                      ×
                    </button>
                  </div>
                  <p className="text-caption text-ink-muted mt-1 line-clamp-2">{role.strategy.substring(0, 60)}...</p>
                </div>
              ))}
              {roles.length === 0 && (
                <div className="p-6 text-center text-caption text-ink-muted">暂无角色，点击右上角创建</div>
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
    </div>
  )
}