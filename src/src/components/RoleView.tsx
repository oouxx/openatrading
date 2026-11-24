import { useState, useMemo, useCallback } from "react"
import { AnimatePresence } from "motion/react"
import { useQueries } from "@tanstack/react-query"
import { Plus, Trash2, Users } from "lucide-react"
import { Button } from "@/components/ui/button"
import { PnlPanel } from "@/components/PnlPanel"
import { StatusBanner } from "@/components/status-banner"
import { DataEmpty, DataError, DataLoading } from "@/components/data-state"
import { Input } from "@/components/ui/input"
import { Skeleton } from "@/components/ui/skeleton"
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from "@/components/ui/dialog"
import {
  fetchJson,
  type RoleEntry,
  type RolePosition,
  type RoleAccount,
  type RoleTrade,
  type ClosedPosition,
} from "@/stores/data"
import { cn } from "@/lib/utils"
import { roleKeys } from "@/hooks/query/queryKeys"
import {
  useRoleList,
  useRolePositions,
  useRoleAccount,
  useRoleTrades,
  useRoleHistory,
  ROLE_LIVE_REFRESH_MS,
  ROLE_SIDEBAR_REFRESH_MS,
} from "@/hooks/query/useRoleQueries"
import { useCreateRole, useDeleteRole, useResetRole } from "@/hooks/query/useRoleMutations"
import KlineDrawer from "@/components/KlineDrawer"
import { OrderBookPanel } from "@/components/OrderBookPanel"
import { BuyTicketForm, SellTicketForm } from "@/components/TradeTicket"
import { formatMoney, formatPct, pctClass } from "@/lib/format"
import { extractStockCode } from "@/lib/trade"

/** 行情不可用时的占位符 —— 后端返回 null 时用它，绝不能显示 0 */
const DASH = "—"

/**
 * 成交/平仓时间列：统一到分钟。
 *
 * 后端是 `YYYY-MM-DD HH:MM:SS`（SQLite localtime），到秒的精度在流水表里只是
 * 噪音；同一天的多笔成交看时分就够定位。
 */
function formatTradeTime(value: string): string {
  return value.replace("T", " ").slice(0, 16)
}

/** 带正负号的金额，用于盈亏展示 */
function formatSignedMoney(v: number): string {
  return `${v >= 0 ? "+" : "-"}¥${formatMoney(Math.abs(v))}`
}

/**
 * 后端所有 `*_pct` 字段已经是百分数（`2.9` 表示 2.9%），不要再次乘 100。
 * 旧版 `pnl_pct` 是小数比例，字段已随接口改版一起换掉。
 */
function errorText(error: unknown): string {
  if (error instanceof Error) return error.message
  return error ? String(error) : "未知错误"
}

/** 表格区域的加载/错误状态，避免把请求失败渲染成「空仓 / 暂无数据」 */
function TableStatus({
  kind,
  message,
  onRetry,
}: {
  kind: "loading" | "error"
  message?: string
  onRetry?: () => void
}) {
  if (kind === "loading") {
    return <DataLoading size="table" rows={4} label="加载中…" />
  }
  return <DataError size="table" title={message ?? "加载失败"} onRetry={onRetry} />
}

/** 表格空态：一句「现在没有」+ 一句「什么情况下会有」，避免空表格像坏了 */
function TableEmpty({ message, hint }: { message: string; hint?: string }) {
  return <DataEmpty size="table" title={message} description={hint} />
}

interface RoleStat {
  count: number
  pnl: number
  loading: boolean
  error: string | null
  incomplete: boolean
}

// ── Role switcher ─────────────────────────────────────
//
// 角色是「账户选择器」，不是第二层导航：原来用 260px 固定侧栏，在全局导航
// 之外又吃掉一条竖向空间，数据表只能横向挤。改成一行 chip——每个 chip 带
// 该角色的盈亏%，比侧栏更适合并排比较，且不再占宽。

/** 一个角色 chip 里的盈亏读数：加载/失败/空仓各有明确表示，不给假数字 */
function RolePnl({ stat }: { stat: RoleStat | undefined }) {
  if (!stat || stat.loading) return <span className="text-tertiary font-number">…</span>
  if (stat.error) return <span className="text-tertiary font-number">—</span>
  if (stat.count === 0) return <span className="text-tertiary">空仓</span>
  if (stat.incomplete || Number.isNaN(stat.pnl)) {
    return <span className="text-tertiary font-number">N/A</span>
  }
  return (
    <span
      className={`font-number ${stat.pnl > 0 ? "text-up" : stat.pnl < 0 ? "text-down" : "text-secondary"}`}
    >
      {stat.pnl >= 0 ? "+" : ""}
      {stat.pnl.toFixed(1)}%
    </span>
  )
}

function RoleChips({
  roles,
  selectedRole,
  roleStats,
  loading,
  error,
  onSelect,
  onCreate,
  onRetry,
}: {
  roles: RoleEntry[]
  selectedRole: string | null
  roleStats: Record<string, RoleStat>
  loading: boolean
  error: string | null
  onSelect: (name: string) => void
  onCreate: () => void
  onRetry: () => void
}) {
  if (loading) {
    return (
      <div className="flex flex-wrap gap-1.5">
        {Array.from({ length: 3 }).map((_, i) => (
          <Skeleton key={i} className="h-7 w-28 rounded-full" />
        ))}
      </div>
    )
  }

  if (error && roles.length === 0) {
    return (
      <div className="flex items-center gap-2">
        <span className="text-danger text-caption">角色列表加载失败</span>
        <Button variant="outline" size="xs" onClick={onRetry}>
          重试
        </Button>
      </div>
    )
  }

  return (
    <div className="flex flex-wrap items-center gap-1.5">
      {roles.map((role) => {
        const displayName = role.display_name || role.name
        const stat = roleStats[role.name]
        const active = selectedRole === role.name
        return (
          <Button
            key={role.name}
            type="button"
            variant="outline"
            aria-pressed={active}
            onClick={() => onSelect(role.name)}
            title={
              stat?.error ??
              (stat?.incomplete ? "部分持仓行情缺失，盈亏为不完整值" : `${stat?.count ?? 0} 个持仓`)
            }
            className={`text-caption h-7 gap-2 rounded-full font-normal ${
              active
                ? "border-brand bg-brand/10 text-primary hover:border-brand hover:bg-brand/10"
                : "border-hairline text-secondary hover:bg-hover"
            }`}
          >
            <span className="max-w-32 truncate">{displayName}</span>
            <RolePnl stat={stat} />
          </Button>
        )
      })}
      <Button
        type="button"
        variant="outline"
        size="xs"
        onClick={onCreate}
        className="border-hairline text-secondary hover:text-primary h-7 gap-1 rounded-full border-dashed px-3"
      >
        <Plus className="size-3.5" />
        新建
      </Button>
    </div>
  )
}

// ── Account overview ───────────────────────────────────

function StatCard({
  label,
  value,
  sub,
  tone = "neutral",
  action,
}: {
  label: string
  value: string
  sub?: string
  tone?: "up" | "down" | "neutral"
  action?: React.ReactNode
}) {
  const toneCls = tone === "up" ? "text-up" : tone === "down" ? "text-down" : "text-primary"
  return (
    <div className="bg-surface border-hairline min-w-0 rounded-lg border px-3 py-2.5">
      <div className="text-secondary text-caption flex items-center justify-between gap-2">
        <span className="truncate">{label}</span>
        {action}
      </div>
      {/*
        金额位数随本金增长，卡宽却由列数决定：text-h1（20px）下 `¥1,000,000.00`
        在本页任何常见窗口宽度都会顶出卡片边框。降到 text-h2（16px）并配合
        truncate 收口——仍放不下时截断而不是溢出，完整值保留在 title 里。
      */}
      <div
        className={`font-number text-h2 mt-0.5 truncate leading-tight font-semibold ${toneCls}`}
        title={value}
      >
        {value}
      </div>
      {sub && <div className={`font-number text-caption font-medium ${toneCls}`}>{sub}</div>}
    </div>
  )
}

function AccountOverview({
  account,
  loading,
  error,
}: {
  account: RoleAccount | undefined
  loading: boolean
  error: string | null
}) {
  if (error) {
    return <StatusBanner tone="danger">账户信息加载失败：{error}</StatusBanner>
  }

  if (loading || !account) {
    return (
      <div className="grid grid-cols-2 gap-2 lg:grid-cols-4">
        {Array.from({ length: 4 }).map((_, i) => (
          <div key={i} className="bg-surface border-hairline rounded-lg border px-4 py-4">
            <Skeleton className="h-3 w-16" />
            <Skeleton className="mt-2 h-5 w-24" />
          </div>
        ))}
      </div>
    )
  }

  const pnlTone = account.total_pnl > 0 ? "up" : account.total_pnl < 0 ? "down" : "neutral"

  return (
    <div className="flex flex-col gap-2">
      {/* 行情不完整时账户数字是残缺值，必须显式提示，避免用户当成准确值 */}
      {!account.valuation_complete && (
        <StatusBanner tone="warning">
          部分持仓行情获取失败，持仓市值与权益为不完整值，仅供参考
        </StatusBanner>
      )}
      {/* 只放四个实时指标。初始资金是静态配置（在标题旁），权益 = 现金 + 持仓
          市值，五张卡并排时最右两张会被挤到换行，读起来也不成组。 */}
      <div className="grid grid-cols-2 gap-2 lg:grid-cols-4">
        <StatCard label="现金" value={`¥${formatMoney(account.cash)}`} />
        <StatCard label="持仓市值" value={`¥${formatMoney(account.market_value)}`} />
        <StatCard label="权益" value={`¥${formatMoney(account.equity)}`} />
        <StatCard
          label="总盈亏"
          value={formatSignedMoney(account.total_pnl)}
          sub={formatPct(account.total_pnl_pct)}
          tone={pnlTone}
        />
      </div>
    </div>
  )
}

function BuyDialog({
  roleName,
  accountLabel,
  onClose,
}: {
  roleName: string
  /** 账户显示名：多账户下用来确认订单记进哪个账，同时充当弹窗的可见描述 */
  accountLabel?: string
  onClose: () => void
}) {
  // 右侧五档要跟着表单里选中的票走：表单的标的状态在组件内部，这里只镜像代码
  const [symbol, setSymbol] = useState("")

  return (
    <Dialog open onOpenChange={(open) => !open && onClose()}>
      <DialogContent className="max-h-[85vh] overflow-y-auto sm:max-w-2xl">
        <DialogHeader>
          <DialogTitle>买入</DialogTitle>
          <DialogDescription className="text-tertiary text-caption">
            手动记账，不撮合{accountLabel ? ` · 记入「${accountLabel}」` : ""}
          </DialogDescription>
        </DialogHeader>
        <div className="grid gap-4 md:grid-cols-[minmax(0,1fr)_15rem]">
          <BuyTicketForm roleName={roleName} onSymbolChange={setSymbol} onDone={onClose} />
          <OrderBookPanel symbol={extractStockCode(symbol) ?? ""} />
        </div>
      </DialogContent>
    </Dialog>
  )
}

// ── 卖出 Dialog ────────────────────────────────────────

function SellDialog({
  roleName,
  accountLabel,
  position,
  onClose,
}: {
  roleName: string
  /** 账户显示名：与买入弹窗同一处上下文，避免多账户下记错账 */
  accountLabel?: string
  position: RolePosition
  onClose: () => void
}) {
  return (
    <Dialog open onOpenChange={(open) => !open && onClose()}>
      <DialogContent className="max-h-[85vh] overflow-y-auto sm:max-w-lg">
        <DialogHeader>
          <DialogTitle>卖出 {position.stock_name || position.symbol}</DialogTitle>
          <DialogDescription className="text-tertiary text-caption">
            {position.symbol}
            {accountLabel ? ` · 记入「${accountLabel}」` : ""}
          </DialogDescription>
        </DialogHeader>
        <div className="grid gap-4 md:grid-cols-[minmax(0,1fr)_15rem]">
          <SellTicketForm roleName={roleName} position={position} onDone={onClose} />
          <OrderBookPanel symbol={extractStockCode(position.symbol) ?? ""} />
        </div>
      </DialogContent>
    </Dialog>
  )
}

type RoleTab = "current" | "trades" | "history" | "performance"

const TABS: { id: RoleTab; label: string }[] = [
  { id: "current", label: "当前持仓" },
  { id: "trades", label: "成交流水" },
  { id: "history", label: "平仓记录" },
  { id: "performance", label: "盈利分析" },
]

/** 每个视图的表格副标题：说清这份数据的新鲜度，而不是复述 tab 名 */
const TAB_HINTS: Record<Exclude<RoleTab, "performance">, string> = {
  current: `行情每 ${ROLE_LIVE_REFRESH_MS / 1000} 秒自动刷新`,
  trades: "仅显示最近 100 笔成交",
  history: "按平仓时间倒序，仅显示最近 100 笔",
}

export default function RoleView() {
  const [selectedRole, setSelectedRole] = useState<string | null>(null)
  const [tab, setTab] = useState<RoleTab>("current")
  const [showCreate, setShowCreate] = useState(false)
  const [showDeleteConfirm, setShowDeleteConfirm] = useState<string | null>(null)
  const [showResetConfirm, setShowResetConfirm] = useState<string | null>(null)
  const [buyRole, setBuyRole] = useState<string | null>(null)
  const [sellTarget, setSellTarget] = useState<{
    roleName: string
    position: RolePosition
  } | null>(null)
  const [newName, setNewName] = useState("")
  const [newCapital, setNewCapital] = useState("1000000")
  const [createError, setCreateError] = useState<string | null>(null)
  const [deleteError, setDeleteError] = useState<string | null>(null)
  const [resetError, setResetError] = useState<string | null>(null)
  const [selectedStock, setSelectedStock] = useState<{ symbol: string; name: string } | null>(null)
  // 已删除但列表 refetch 还没返回的角色。连续删除时只记一个名字不够，
  // 派生的 activeRole 可能在旧列表里选中更早删掉的角色。
  const [deletedRoles, setDeletedRoles] = useState<string[]>([])

  const {
    data: roles = [],
    isPending: rolesPending,
    error: rolesQueryError,
    refetch: refetchRoles,
  } = useRoleList()

  // 选中角色是用户意图；列表刷新后如果它不存在，就派生一个有效的 activeRole，
  // 而不是在 effect 里 setState（会多一轮渲染，也容易在删除后把旧角色选回来）。
  // visibleRoles 立即排除已删除、但列表 refetch 尚未返回的角色。
  const visibleRoles = useMemo(
    () => roles.filter((r) => !deletedRoles.includes(r.name)),
    [roles, deletedRoles]
  )
  const selectedRoleExists = !!selectedRole && visibleRoles.some((r) => r.name === selectedRole)
  const activeRole = selectedRoleExists
    ? selectedRole
    : visibleRoles.length > 0
      ? visibleRoles[0].name
      : selectedRole && (rolesPending || rolesQueryError)
        ? selectedRole
        : null

  const positionsQuery = useRolePositions(activeRole ?? "")
  const accountQuery = useRoleAccount(activeRole ?? "")
  const tradesQuery = useRoleTrades(activeRole ?? "")
  const historyQuery = useRoleHistory(activeRole ?? "")

  const positions = positionsQuery.data ?? []
  const account = accountQuery.data
  const trades = tradesQuery.data ?? []
  const history = historyQuery.data ?? []

  const createRoleMutation = useCreateRole()
  const deleteRoleMutation = useDeleteRole()
  const resetRoleMutation = useResetRole()

  const currentRole = visibleRoles.find((r) => r.name === activeRole)
  const currentDisplayName = currentRole?.display_name || currentRole?.name || activeRole || ""

  // Fetch all visible roles' positions for sidebar PnL stats
  const positionQueries = useQueries({
    queries: visibleRoles.map((r) => ({
      queryKey: roleKeys.positions(r.name),
      queryFn: () => fetchJson<RolePosition[]>(`/api/roles/${r.name}/positions`),
      staleTime: ROLE_LIVE_REFRESH_MS,
      refetchInterval: ROLE_SIDEBAR_REFRESH_MS,
      refetchOnWindowFocus: true,
      enabled: !!r.name,
    })),
  })

  const roleStats = useMemo(() => {
    const result: Record<string, RoleStat> = {}
    visibleRoles.forEach((r, i) => {
      const query = positionQueries[i]
      if (!query || query.isPending) {
        result[r.name] = {
          count: 0,
          pnl: Number.NaN,
          loading: true,
          error: null,
          incomplete: false,
        }
        return
      }
      if (query.isError) {
        result[r.name] = {
          count: 0,
          pnl: Number.NaN,
          loading: false,
          error: errorText(query.error),
          incomplete: false,
        }
        return
      }
      const pos = query.data ?? []
      let totalValue = 0
      let totalCost = 0
      let incomplete = false
      for (const p of pos) {
        // 任一持仓缺行情，整个角色盈亏都不完整；显示 N/A，不把残缺值当真实收益
        if (p.last_price === null || p.last_price <= 0) {
          incomplete = true
          continue
        }
        totalValue += p.last_price * p.shares
        totalCost += p.cost_price * p.shares
      }
      result[r.name] = {
        count: pos.length,
        pnl: totalCost > 0 ? ((totalValue - totalCost) / totalCost) * 100 : Number.NaN,
        loading: false,
        error: null,
        incomplete,
      }
    })
    return result
  }, [visibleRoles, positionQueries])

  const roleDisplayName = useCallback(
    (name: string | null) => {
      if (!name) return ""
      const role = roles.find((r) => r.name === name)
      return role?.display_name || role?.name || name
    },
    [roles]
  )

  const openCreate = useCallback(() => {
    setCreateError(null)
    setShowCreate(true)
  }, [])

  const openDelete = useCallback((name: string) => {
    setDeleteError(null)
    setShowDeleteConfirm(name)
  }, [])

  const openReset = useCallback((name: string) => {
    setResetError(null)
    setShowResetConfirm(name)
  }, [])

  const createRole = useCallback(async () => {
    if (createRoleMutation.isPending) return
    const name = newName.trim()
    if (!name) {
      setCreateError("请输入角色名称")
      return
    }
    const id =
      name
        .toLowerCase()
        .replace(/[^a-z0-9一-鿿_-]/g, "_")
        .replace(/_+/g, "_")
        .replace(/^_|_$/g, "")
        .slice(0, 64) || "role_" + Date.now().toString(36)
    if (visibleRoles.some((r) => r.name === id)) {
      setCreateError(`角色标识「${id}」已存在，请换一个名称`)
      return
    }
    const capital = Number(newCapital)
    if (newCapital.trim() === "" || !Number.isFinite(capital) || capital <= 0) {
      setCreateError("初始资金必须是大于 0 的数字")
      return
    }
    setCreateError(null)
    try {
      await createRoleMutation.mutateAsync({
        name: id,
        displayName: name,
        initialCapital: capital,
      })
      setShowCreate(false)
      setNewName("")
      setNewCapital("1000000")
      // 同名角色可能刚被删过；只解除这个新名字的排除，其他已删除角色继续排除
      setDeletedRoles((prev) => prev.filter((n) => n !== id))
      setSelectedRole(id)
    } catch (e) {
      setCreateError(errorText(e))
    }
  }, [newName, newCapital, visibleRoles, createRoleMutation])

  const deleteRole = useCallback(
    async (name: string) => {
      if (deleteRoleMutation.isPending) return
      setDeleteError(null)
      const remaining = visibleRoles.filter((r) => r.name !== name)
      try {
        await deleteRoleMutation.mutateAsync(name)
        setShowDeleteConfirm(null)
        if (buyRole === name) setBuyRole(null)
        if (sellTarget?.roleName === name) setSellTarget(null)
        setDeletedRoles((prev) => (prev.includes(name) ? prev : [...prev, name]))
        if (activeRole === name) {
          setSelectedRole(remaining[0]?.name ?? null)
        }
      } catch (e) {
        setDeleteError(errorText(e))
      }
    },
    [visibleRoles, activeRole, buyRole, sellTarget, deleteRoleMutation]
  )

  const resetRole = useCallback(
    async (name: string) => {
      if (resetRoleMutation.isPending) return
      setResetError(null)
      try {
        await resetRoleMutation.mutateAsync(name)
        setShowResetConfirm(null)
      } catch (e) {
        setResetError(errorText(e))
      }
    },
    [resetRoleMutation]
  )

  /**
   * 卡头两段文字：左边说数据新鲜度，右边说条数。条数只在数据到位后出现——
   * 加载中显示「0 个持仓」会被读成「空仓」。
   */
  const tabMeta = useMemo(() => {
    if (tab === "performance")
      return { hint: "历史按收盘价估值，最新点实时更新", count: null as string | null }
    const [query, length, unit] =
      tab === "current"
        ? [positionsQuery, positions.length, "个持仓"]
        : tab === "trades"
          ? [tradesQuery, trades.length, "笔成交"]
          : [historyQuery, history.length, "笔平仓"]
    return {
      hint: TAB_HINTS[tab],
      count: query.isPending || query.isError ? null : `${length} ${unit}`,
    }
  }, [
    tab,
    positions.length,
    trades.length,
    history.length,
    positionsQuery,
    tradesQuery,
    historyQuery,
  ])

  return (
    <>
      <div className="flex h-full flex-row">
        {/* 子项必须 shrink-0：卡片带 overflow-hidden，作为列向 flex item 会被压缩到
            低于内容高度，导致曲线底部（日期横轴）和脚注被裁掉，页面却不滚动。 */}
        <main className="readable-content flex flex-1 flex-col gap-5 overflow-y-auto px-7 py-6 [&>*]:shrink-0">
          {!activeRole ? (
            <div className="flex flex-1 items-center justify-center px-6">
              <div className="w-full max-w-md">
                {rolesPending && visibleRoles.length === 0 ? (
                  <DataLoading label="加载角色…" size="panel" />
                ) : rolesQueryError && visibleRoles.length === 0 ? (
                  <DataError
                    title="角色列表加载失败"
                    description={errorText(rolesQueryError)}
                    onRetry={() => void refetchRoles()}
                  />
                ) : (
                  <DataEmpty
                    icon={<Users className="size-4" />}
                    title="还没有模拟账户"
                    description="每个角色都是一个独立的模拟账户，买入后按实时行情估值。"
                    action={
                      <Button size="sm" onClick={openCreate}>
                        <Plus />
                        新建角色
                      </Button>
                    }
                  />
                )}
              </div>
            </div>
          ) : (
            <>
              {/* Header + Tabs + 主操作 */}
              <div className="group flex flex-wrap items-center justify-between gap-3">
                <div className="flex min-w-0 items-baseline gap-3">
                  <h1
                    className="text-h1 text-primary min-w-0 truncate font-semibold tracking-[-0.01em]"
                    title={currentRole?.name ?? activeRole}
                  >
                    {currentDisplayName}
                  </h1>
                  {currentRole?.readonly && (
                    <span className="bg-hover text-tertiary text-caption shrink-0 rounded px-2 py-0.5">
                      只读
                    </span>
                  )}
                  {/* 初始资金是静态配置，不是实时指标：放在标题旁做背景信息，
                      不再单独占一张和「权益」并排的卡片 */}
                  {account && (
                    <span className="text-tertiary text-caption shrink-0">
                      初始资金 ¥{formatMoney(account.initial_capital)}
                    </span>
                  )}
                  {currentRole && !currentRole.readonly && (
                    <Button
                      type="button"
                      variant="ghost"
                      size="icon-xs"
                      onClick={() => activeRole && openDelete(activeRole)}
                      aria-label={`删除角色 ${currentDisplayName}`}
                      title="删除角色"
                      className="hover:bg-danger-bg hover:text-danger text-tertiary can-hover:opacity-0 shrink-0 self-center transition-opacity group-hover:opacity-100 focus-visible:opacity-100"
                    >
                      <Trash2 className="size-3.5" />
                    </Button>
                  )}
                </div>
                <div className="flex flex-wrap items-center gap-2">
                  <div className="bg-hover flex gap-1 rounded-lg p-1">
                    {TABS.map((t) => (
                      <Button
                        key={t.id}
                        variant="ghost"
                        size="sm"
                        aria-pressed={tab === t.id}
                        onClick={() => setTab(t.id)}
                        className={`text-caption font-medium ${
                          tab === t.id
                            ? "bg-surface text-primary shadow-card hover:bg-surface hover:text-primary"
                            : "text-secondary hover:text-primary"
                        }`}
                      >
                        {t.label}
                      </Button>
                    ))}
                  </div>
                  <Button size="sm" onClick={() => activeRole && setBuyRole(activeRole)}>
                    买入
                  </Button>
                </div>
              </div>

              <RoleChips
                roles={visibleRoles}
                selectedRole={activeRole}
                roleStats={roleStats}
                loading={rolesPending && visibleRoles.length === 0}
                error={
                  visibleRoles.length === 0 && rolesQueryError ? errorText(rolesQueryError) : null
                }
                onSelect={(name) => {
                  setSelectedRole(name)
                  setShowCreate(false)
                }}
                onCreate={openCreate}
                onRetry={() => void refetchRoles()}
              />

              {/* Account overview */}
              <AccountOverview
                account={account}
                loading={accountQuery.isPending}
                error={accountQuery.error ? errorText(accountQuery.error) : null}
              />

              {/* 数据卡片：视图由上方 segmented 决定，卡头只说数据新鲜度与条数 */}
              <div className="bg-surface border-hairline overflow-hidden rounded-lg border">
                <div className="border-hairline text-tertiary text-caption flex flex-wrap items-center justify-between gap-2 border-b px-5 py-2.5">
                  <span>{tabMeta.hint}</span>
                  {tabMeta.count && <span>{tabMeta.count}</span>}
                </div>
                {tab === "performance" ? (
                  activeRole ? (
                    <PnlPanel roleName={activeRole} />
                  ) : (
                    <TableStatus kind="error" message="未选择角色" />
                  )
                ) : (
                  /* 只有表格需要横向滚动；曲线/日历不能被放进 overflow-auto，
                     否则内部的宽度测量会按滚动容器的 min-content 计算 */
                  <div className="overflow-x-auto">
                    {tab === "current" &&
                      (positionsQuery.isPending ? (
                        <TableStatus kind="loading" />
                      ) : positionsQuery.isError ? (
                        <TableStatus
                          kind="error"
                          message={`持仓加载失败：${errorText(positionsQuery.error)}`}
                          onRetry={() => void positionsQuery.refetch()}
                        />
                      ) : positions.length > 0 ? (
                        <table className="data-table">
                          <thead>
                            <tr>
                              <Th>股票</Th>
                              <Th className="num">成本价</Th>
                              <Th className="num">现价</Th>
                              <Th className="num">数量</Th>
                              <Th className="num">市值</Th>
                              <Th className="num">浮动盈亏</Th>
                              <Th className="num">盈亏%</Th>
                              <Th>入场日期</Th>
                              <Th className="col-sticky-actions">操作</Th>
                            </tr>
                          </thead>
                          <tbody>
                            {positions.map((p: RolePosition) => (
                              <tr key={p.symbol}>
                                <Td bold>
                                  <Button
                                    variant="ghost"
                                    size="xs"
                                    className="hover:text-brand h-auto justify-start p-0 font-normal hover:bg-transparent"
                                    onClick={() =>
                                      setSelectedStock({ symbol: p.symbol, name: p.stock_name })
                                    }
                                  >
                                    {p.stock_name}
                                  </Button>
                                  <span className="text-secondary font-number text-caption ml-2">
                                    {p.symbol}
                                  </span>
                                </Td>
                                <Td className="num">{formatMoney(p.cost_price)}</Td>
                                <Td className="num">
                                  {p.last_price === null ? DASH : formatMoney(p.last_price)}
                                </Td>
                                <Td className="num">{p.shares}</Td>
                                <Td className="num">
                                  {p.market_value === null ? DASH : formatMoney(p.market_value)}
                                </Td>
                                <Td className={cn("num", pctClass(p.unrealized_pnl))}>
                                  {p.unrealized_pnl === null
                                    ? DASH
                                    : formatSignedMoney(p.unrealized_pnl)}
                                </Td>
                                <Td className={cn("num", pctClass(p.unrealized_pct))}>
                                  {p.unrealized_pct === null ? DASH : formatPct(p.unrealized_pct)}
                                </Td>
                                <Td muted>{p.opened_at.slice(0, 10)}</Td>
                                <Td className="col-sticky-actions">
                                  <Button
                                    variant="outline"
                                    size="xs"
                                    onClick={() =>
                                      activeRole &&
                                      setSellTarget({ roleName: activeRole, position: p })
                                    }
                                  >
                                    卖出
                                  </Button>
                                </Td>
                              </tr>
                            ))}
                          </tbody>
                        </table>
                      ) : (
                        <TableEmpty
                          message="当前空仓"
                          hint="点右上角「买入」记录第一笔成交，持仓与卖出按钮会出现在这里"
                        />
                      ))}

                    {tab === "trades" &&
                      (tradesQuery.isPending ? (
                        <TableStatus kind="loading" />
                      ) : tradesQuery.isError ? (
                        <TableStatus
                          kind="error"
                          message={`成交流水加载失败：${errorText(tradesQuery.error)}`}
                          onRetry={() => void tradesQuery.refetch()}
                        />
                      ) : trades.length > 0 ? (
                        <table className="data-table">
                          <thead>
                            <tr>
                              <Th>成交时间</Th>
                              <Th>股票</Th>
                              <Th>方向</Th>
                              <Th className="num">价格</Th>
                              <Th className="num">数量</Th>
                              <Th className="num">成本价</Th>
                              <Th className="num">已实现盈亏</Th>
                              <Th>备注</Th>
                            </tr>
                          </thead>
                          <tbody>
                            {trades.map((t: RoleTrade) => (
                              <tr key={t.id}>
                                <Td muted>{formatTradeTime(t.traded_at)}</Td>
                                <Td bold>
                                  <Button
                                    variant="ghost"
                                    size="xs"
                                    className="hover:text-brand h-auto justify-start p-0 font-normal hover:bg-transparent"
                                    onClick={() =>
                                      setSelectedStock({ symbol: t.symbol, name: t.stock_name })
                                    }
                                  >
                                    {t.stock_name}
                                  </Button>
                                  <span className="text-secondary font-number text-caption ml-2">
                                    {t.symbol}
                                  </span>
                                </Td>
                                <Td>
                                  <span className={t.side === "buy" ? "text-up" : "text-down"}>
                                    {t.side === "buy" ? "买入" : "卖出"}
                                  </span>
                                </Td>
                                <Td className="num">{formatMoney(t.price)}</Td>
                                <Td className="num">{t.shares}</Td>
                                <Td className="num">{formatMoney(t.cost_price)}</Td>
                                <Td className={cn("num", pctClass(t.realized_pnl))}>
                                  {t.realized_pnl === null
                                    ? DASH
                                    : formatSignedMoney(t.realized_pnl)}
                                </Td>
                                <Td reason>{t.reason}</Td>
                              </tr>
                            ))}
                          </tbody>
                        </table>
                      ) : (
                        <TableEmpty
                          message="暂无成交流水"
                          hint="买入或卖出后，每一笔都会按时间倒序记在这里"
                        />
                      ))}

                    {tab === "history" &&
                      (historyQuery.isPending ? (
                        <TableStatus kind="loading" />
                      ) : historyQuery.isError ? (
                        <TableStatus
                          kind="error"
                          message={`平仓记录加载失败：${errorText(historyQuery.error)}`}
                          onRetry={() => void historyQuery.refetch()}
                        />
                      ) : history.length > 0 ? (
                        <table className="data-table">
                          <thead>
                            <tr>
                              <Th>股票</Th>
                              <Th className="num">成本价</Th>
                              <Th className="num">出场价</Th>
                              <Th className="num">数量</Th>
                              <Th className="num">已实现盈亏</Th>
                              <Th className="num">盈亏%</Th>
                              <Th>平仓时间</Th>
                              <Th>出场理由</Th>
                            </tr>
                          </thead>
                          <tbody>
                            {history.map((h: ClosedPosition) => (
                              <tr key={h.id}>
                                <Td bold>
                                  <Button
                                    variant="ghost"
                                    size="xs"
                                    className="hover:text-brand h-auto justify-start p-0 font-normal hover:bg-transparent"
                                    onClick={() =>
                                      setSelectedStock({ symbol: h.symbol, name: h.stock_name })
                                    }
                                  >
                                    {h.stock_name}
                                  </Button>
                                  <span className="text-secondary font-number text-caption ml-2">
                                    {h.symbol}
                                  </span>
                                </Td>
                                <Td className="num">{formatMoney(h.cost_price)}</Td>
                                <Td className="num">{formatMoney(h.exit_price)}</Td>
                                <Td className="num">{h.shares}</Td>
                                <Td className={cn("num", pctClass(h.realized_pnl))}>
                                  {formatSignedMoney(h.realized_pnl)}
                                </Td>
                                <Td className={cn("num", pctClass(h.realized_pct))}>
                                  {formatPct(h.realized_pct)}
                                </Td>
                                <Td muted>{formatTradeTime(h.closed_at)}</Td>
                                <Td reason>{h.reason}</Td>
                              </tr>
                            ))}
                          </tbody>
                        </table>
                      ) : (
                        <TableEmpty
                          message="暂无平仓记录"
                          hint="卖出后，平仓结果与出场理由会归档到这里"
                        />
                      ))}
                  </div>
                )}
              </div>

              {/* 危险动作留在页尾、离主操作最远；配色用危险语义色，
                  不用 `up`（那是 A 股涨的颜色，不是「危险」）。 */}
              <div className="mt-auto flex gap-2">
                <Button
                  variant="outline"
                  size="xs"
                  onClick={() => activeRole && openReset(activeRole)}
                  className="hover:border-danger/40 hover:text-danger text-secondary"
                >
                  重置账户
                </Button>
              </div>
            </>
          )}
        </main>
      </div>

      {/* Create Modal */}
      <Dialog
        open={showCreate}
        onOpenChange={(open) => {
          setShowCreate(open)
          if (!open) setCreateError(null)
        }}
      >
        <DialogContent className="sm:max-w-md">
          <DialogHeader>
            <DialogTitle>新建角色</DialogTitle>
            <DialogDescription className="sr-only">
              创建一个新的模拟交易角色并设置初始资金
            </DialogDescription>
          </DialogHeader>
          <form
            className="flex flex-col gap-3"
            onSubmit={(e) => {
              e.preventDefault()
              void createRole()
            }}
            onKeyDown={(e) => {
              // 中文输入法用 Enter 选词时不要顺带创建角色
              if (e.key === "Enter" && e.nativeEvent.isComposing) e.preventDefault()
            }}
          >
            <div className="flex flex-col gap-1.5">
              <label htmlFor="new-role-name" className="text-secondary text-caption">
                角色名称
              </label>
              <Input
                id="new-role-name"
                type="text"
                value={newName}
                onChange={(e) => {
                  setNewName(e.target.value)
                  setCreateError(null)
                }}
                className="text-caption"
                placeholder="例如：套利策略 v2"
                onKeyDown={(e) => {
                  if (e.key === "Enter" && e.nativeEvent.isComposing) e.preventDefault()
                }}
                autoFocus
              />
            </div>
            <div className="flex flex-col gap-1.5">
              <label htmlFor="new-role-capital" className="text-secondary text-caption">
                初始资金（元）
              </label>
              <Input
                id="new-role-capital"
                type="number"
                min="0"
                step="0.01"
                inputMode="decimal"
                value={newCapital}
                onChange={(e) => {
                  setNewCapital(e.target.value)
                  setCreateError(null)
                }}
                className="font-number text-caption"
                placeholder="1000000"
              />
              <p className="text-tertiary text-caption">创建后不可修改；重置账户时保留该本金。</p>
            </div>
            {createError && <p className="text-danger text-caption">{createError}</p>}
            <DialogFooter>
              <Button type="button" variant="ghost" onClick={() => setShowCreate(false)}>
                取消
              </Button>
              <Button type="submit" disabled={createRoleMutation.isPending}>
                {createRoleMutation.isPending ? "创建中…" : "创建"}
              </Button>
            </DialogFooter>
          </form>
        </DialogContent>
      </Dialog>

      {/* Delete Confirm */}
      <Dialog
        open={showDeleteConfirm !== null}
        onOpenChange={(open) => {
          if (!open) {
            setShowDeleteConfirm(null)
            setDeleteError(null)
          }
        }}
      >
        <DialogContent className="sm:max-w-sm">
          <DialogHeader>
            <DialogTitle>
              删除角色{showDeleteConfirm ? `「${roleDisplayName(showDeleteConfirm)}」` : ""}
            </DialogTitle>
            <DialogDescription className="sr-only">确认删除角色及其全部交易数据</DialogDescription>
          </DialogHeader>
          <p className="text-body text-secondary">
            删除将清空该角色的全部持仓与成交流水，历史无法恢复；角色本身与初始资金设置一并删除。
            只想清空交易记录、保留角色请用重置。
          </p>
          {deleteError && <p className="text-danger text-caption">{deleteError}</p>}
          <DialogFooter>
            <Button
              variant="ghost"
              disabled={deleteRoleMutation.isPending}
              onClick={() => setShowDeleteConfirm(null)}
            >
              取消
            </Button>
            <Button
              variant="destructive"
              disabled={deleteRoleMutation.isPending}
              onClick={() => showDeleteConfirm && void deleteRole(showDeleteConfirm)}
            >
              {deleteRoleMutation.isPending ? "删除中…" : "删除"}
            </Button>
          </DialogFooter>
        </DialogContent>
      </Dialog>

      {/* Reset Confirm */}
      <Dialog
        open={showResetConfirm !== null}
        onOpenChange={(open) => {
          if (!open) {
            setShowResetConfirm(null)
            setResetError(null)
          }
        }}
      >
        <DialogContent className="sm:max-w-sm">
          <DialogHeader>
            <DialogTitle>
              重置账户{showResetConfirm ? `「${roleDisplayName(showResetConfirm)}」` : ""}
            </DialogTitle>
            <DialogDescription className="sr-only">确认清空角色的持仓与成交流水</DialogDescription>
          </DialogHeader>
          <p className="text-body text-secondary">
            重置将清空该角色的全部持仓与成交流水，历史无法恢复；初始资金不变。
            要换一个本金请新建角色。
          </p>
          {resetError && <p className="text-danger text-caption">{resetError}</p>}
          <DialogFooter>
            <Button
              variant="ghost"
              disabled={resetRoleMutation.isPending}
              onClick={() => setShowResetConfirm(null)}
            >
              取消
            </Button>
            <Button
              variant="destructive"
              disabled={resetRoleMutation.isPending}
              onClick={() => showResetConfirm && void resetRole(showResetConfirm)}
            >
              {resetRoleMutation.isPending ? "重置中…" : "重置"}
            </Button>
          </DialogFooter>
        </DialogContent>
      </Dialog>

      {/* Buy / Sell dialogs — 关闭即卸载，表单状态自然重置 */}
      {buyRole && (
        <BuyDialog
          roleName={buyRole}
          accountLabel={roleDisplayName(buyRole)}
          onClose={() => setBuyRole(null)}
        />
      )}
      {sellTarget && (
        <SellDialog
          roleName={sellTarget.roleName}
          accountLabel={roleDisplayName(sellTarget.roleName)}
          position={sellTarget.position}
          onClose={() => setSellTarget(null)}
        />
      )}

      {/* Stock Detail Drawer */}
      <AnimatePresence>
        {selectedStock && (
          <KlineDrawer
            symbol={selectedStock.symbol}
            name={selectedStock.name}
            onClose={() => setSelectedStock(null)}
          />
        )}
      </AnimatePresence>
    </>
  )
}

// ── Table helpers ──────────────────────────────────────

function Th({ children, className }: { children: React.ReactNode; className?: string }) {
  return <th className={className}>{children}</th>
}
function Td({
  children,
  className,
  bold,
  muted,
  reason,
}: {
  children: React.ReactNode
  className?: string
  bold?: boolean
  muted?: boolean
  reason?: boolean
}) {
  let cls = "whitespace-nowrap"
  if (bold) cls += " font-medium"
  if (muted) cls += " text-secondary"
  if (reason) cls += " max-w-sidebar whitespace-normal text-secondary leading-relaxed"
  if (className) cls += " " + className
  return <td className={cls}>{children}</td>
}
