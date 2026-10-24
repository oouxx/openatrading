import { useState, useEffect, useMemo, useCallback, useRef } from "react"
import { AnimatePresence } from "motion/react"
import { useQueries } from "@tanstack/react-query"
import { AlertTriangle, Users } from "lucide-react"
import { Button } from "@/components/ui/button"
import { PnlPanel } from "@/components/PnlPanel"
import { StatusBanner } from "@/components/status-banner"
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
  Sheet,
  SheetContent,
  SheetDescription,
  SheetHeader,
  SheetTitle,
} from "@/components/ui/sheet"
import { Command, CommandEmpty, CommandItem, CommandList } from "@/components/ui/command"
import {
  fetchJson,
  type RoleEntry,
  type RolePosition,
  type RoleAccount,
  type RoleTrade,
  type ClosedPosition,
  type StockSearchItem,
} from "@/stores/data"
import { searchStocks } from "@/lib/stock-utils"
import { roleKeys } from "@/hooks/query/queryKeys"
import { useRealtimeQuote } from "@/hooks/query/useRealtimeQuote"
import {
  useRoleList,
  useRolePositions,
  useRoleAccount,
  useRoleTrades,
  useRoleHistory,
  ROLE_LIVE_REFRESH_MS,
  ROLE_SIDEBAR_REFRESH_MS,
} from "@/hooks/query/useRoleQueries"
import {
  useCreateRole,
  useDeleteRole,
  useResetRole,
  usePlaceOrder,
} from "@/hooks/query/useRoleMutations"
import KlineDrawer from "@/components/KlineDrawer"
import { formatPct, pctClass } from "@/lib/format"

/** 行情不可用时的占位符 —— 后端返回 null 时用它，绝不能显示 0 */
const DASH = "—"

function formatMoney(v: number): string {
  return v.toLocaleString("zh-CN", { minimumFractionDigits: 2, maximumFractionDigits: 2 })
}

/** 带正负号的金额，用于盈亏展示 */
function formatSignedMoney(v: number): string {
  return `${v >= 0 ? "+" : "-"}¥${formatMoney(Math.abs(v))}`
}

/**
 * 后端所有 `*_pct` 字段已经是百分数（`2.9` 表示 2.9%），不要再次乘 100。
 * 旧版 `pnl_pct` 是小数比例，字段已随接口改版一起换掉。
 */
function FieldError({ children }: { children: string }) {
  return <p className="text-danger text-caption mt-1">{children}</p>
}

/** A 股整手股数。买入必须是它的整数倍（与后端 `Shares::require_board_lot` 一致）。 */
const BOARD_LOT = 100


/**
 * 只接受不会被后端 `normalize_code` 误解析的输入：
 * 裸 6 位代码，或带 sh/sz/ss 前缀/后缀的 6 位代码。
 * 像 "360" 这种 3 位数字后端会补成 000360，必须在前端直接拒绝。
 */
const SYMBOL_CODE_RE = /^(?:(?:sh|sz|ss)[.\-_]?(\d{6})|(\d{6})(?:\.(?:ss|sz))?)$/i

function extractStockCode(raw: string): string | null {
  const match = SYMBOL_CODE_RE.exec(raw.trim())
  return match ? (match[1] ?? match[2] ?? null) : null
}

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
    return (
      <div className="space-y-2 px-5 py-6" aria-label="加载中">
        <Skeleton className="h-4 w-full" />
        <Skeleton className="h-4 w-5/6" />
        <Skeleton className="h-4 w-2/3" />
      </div>
    )
  }
  return (
    <div className="flex flex-col items-center gap-2 px-5 py-10 text-center">
      <AlertTriangle className="text-danger size-4" />
      <p className="text-danger text-caption">{message ?? "加载失败"}</p>
      {onRetry && (
        <Button variant="outline" size="xs" onClick={onRetry}>
          重试
        </Button>
      )}
    </div>
  )
}

interface RoleStat {
  count: number
  pnl: number
  loading: boolean
  error: string | null
  incomplete: boolean
}

/**
 * 按可用资金与价格折算买入股数，向下取整到整手。
 * 资金/价格不可用时返回 0，调用方据此隐藏快捷仓位。
 */
function maxBuyableShares(cash: number | null, price: number): number {
  if (cash === null || !Number.isFinite(price) || price <= 0 || cash <= 0) return 0
  return Math.floor(cash / price / BOARD_LOT) * BOARD_LOT
}

/** 买入股数快捷比例 —— 按可用资金折算成「几成仓」 */
const BUY_SHARE_RATIOS: { label: string; ratio: number }[] = [
  { label: "1/3 仓", ratio: 1 / 3 },
  { label: "半仓", ratio: 1 / 2 },
  { label: "全仓", ratio: 1 },
]

/** 卖出股数快捷比例 —— 「全部」取实际持仓，其余按整手折算 */
function sellShareOptions(held: number): { label: string; shares: number }[] {
  const options = [{ label: "全部", shares: held }]
  for (const ratio of [1 / 2, 1 / 3]) {
    const shares = Math.floor((held * ratio) / BOARD_LOT) * BOARD_LOT
    if (shares >= BOARD_LOT && shares < held) {
      options.push({ label: ratio === 1 / 2 ? "半仓" : "1/3 仓", shares })
    }
  }
  return options
}

// ── Sidebar ───────────────────────────────────────────

interface RoleListProps {
  roles: RoleEntry[]
  selectedRole: string | null
  onSelect: (name: string) => void
  onDelete: (name: string) => void
  onCreate: () => void
  roleStats: Record<string, RoleStat>
  search: string
  setSearch: (s: string) => void
  totalCount: number
  rolesLoading: boolean
  rolesError: string | null
  onRetryRoles: () => void
}

/** 侧边栏和移动端抽屉共用同一份角色列表，避免两套实现漂移。 */
function RoleListBody({
  roles,
  selectedRole,
  onSelect,
  onDelete,
  onCreate,
  roleStats,
  search,
  setSearch,
  totalCount,
  rolesLoading,
  rolesError,
  onRetryRoles,
}: RoleListProps) {
  const filtered = roles.filter((r) => {
    const displayName = r.display_name || r.name
    return displayName.toLowerCase().includes(search.toLowerCase())
  })

  return (
    <>
      <div className="border-hairline flex items-center justify-between gap-2 border-b p-4 pr-10">
        <h2 className="text-h2 text-primary font-medium">角色列表</h2>
        <span className="text-secondary text-caption">{totalCount} 个</span>
      </div>
      <div className="border-hairline border-b px-4 py-3">
        <Input
          type="text"
          placeholder="搜索角色…"
          value={search}
          onChange={(e) => setSearch(e.target.value)}
          className="bg-canvas border-hairline focus-visible:border-brand text-caption"
        />
      </div>
      <div className="flex-1 overflow-y-auto py-2">
        {rolesLoading ? (
          <div className="space-y-2 px-4 py-2">
            {Array.from({ length: 4 }).map((_, i) => (
              <Skeleton key={i} className="h-10 w-full" />
            ))}
          </div>
        ) : rolesError ? (
          <div className="flex flex-col items-center gap-2 px-4 py-6 text-center">
            <p className="text-danger text-caption">角色列表加载失败</p>
            <p className="text-tertiary text-caption break-all">{rolesError}</p>
            <Button variant="outline" size="xs" onClick={onRetryRoles}>
              重试
            </Button>
          </div>
        ) : filtered.length === 0 ? (
          <p className="text-secondary text-caption px-4 py-6 text-center">
            {roles.length === 0 ? "暂无角色，创建一个开始模拟交易" : "没有匹配的角色"}
          </p>
        ) : (
          filtered.map((role) => {
            const displayName = role.display_name || role.name
            const stat = roleStats[role.name]
            const loading = stat?.loading ?? true
            const statError = stat?.error ?? null
            const count = stat?.count ?? 0
            const pnl = stat?.pnl ?? Number.NaN
            const pnlUnavailable = !!statError || !!stat?.incomplete || Number.isNaN(pnl)
            return (
              <div
                key={role.name}
                className={`group flex items-center justify-between gap-3 border-l-[3px] px-4 py-3 transition-colors ${
                  selectedRole === role.name
                    ? "border-l-ai bg-brand/10"
                    : "hover:bg-hover border-l-transparent"
                }`}
              >
                <button
                  type="button"
                  aria-pressed={selectedRole === role.name}
                  className="flex min-w-0 flex-1 cursor-pointer items-center justify-between gap-3 text-left"
                  onClick={() => onSelect(role.name)}
                >
                  <span className="min-w-0 flex-1">
                    <span className="text-caption text-primary block truncate font-medium">
                      {displayName}
                    </span>
                    <span className="text-secondary text-caption mt-0.5 flex gap-3">
                      <span>
                        {loading ? "… 持仓" : statError ? "持仓加载失败" : `${count} 持仓`}
                      </span>
                      <span
                        className={`font-number text-caption font-semibold ${
                          loading || statError
                            ? "text-secondary"
                            : pnl > 0
                              ? "text-up"
                              : pnl < 0
                                ? "text-down"
                                : "text-primary"
                        }`}
                        title={
                          statError ??
                          (stat?.incomplete ? "部分持仓行情缺失，盈亏为不完整值" : undefined)
                        }
                      >
                        {loading
                          ? "…"
                          : statError
                            ? "—"
                            : count === 0
                              ? "空仓"
                              : pnlUnavailable
                                ? "N/A"
                                : `${pnl >= 0 ? "+" : ""}${pnl.toFixed(1)}%`}
                      </span>
                    </span>
                  </span>
                </button>
                {!role.readonly && (
                  <Button
                    type="button"
                    variant="ghost"
                    size="icon-xs"
                    aria-label={`删除角色 ${displayName}`}
                    onClick={() => onDelete(role.name)}
                    className="hover:bg-up/10 hover:text-up text-secondary text-h2 size-6 opacity-0 transition-opacity group-hover:opacity-100 focus-visible:opacity-100"
                    style={selectedRole === role.name ? { opacity: 1 } : undefined}
                    title="删除角色"
                  >
                    ×
                  </Button>
                )}
              </div>
            )
          })
        )}
        {!rolesLoading && (
          <Button
            variant="ghost"
            size="xs"
            onClick={onCreate}
            className="hover:text-brand text-secondary h-auto w-full justify-start rounded-none px-4 py-3"
          >
            + 新建角色
          </Button>
        )}
      </div>
    </>
  )
}

function Sidebar(props: RoleListProps) {
  return (
    <aside className="bg-surface border-hairline w-sidebar min-w-sidebar hidden flex-col overflow-y-auto border-r md:flex">
      <RoleListBody {...props} />
    </aside>
  )
}

function MobileRoleSheet({
  open,
  onOpenChange,
  ...props
}: RoleListProps & { open: boolean; onOpenChange: (open: boolean) => void }) {
  return (
    <Sheet open={open} onOpenChange={onOpenChange}>
      <SheetContent side="left" className="w-80 max-w-[85vw] gap-0 p-0">
        <SheetHeader className="sr-only">
          <SheetTitle>角色列表</SheetTitle>
          <SheetDescription>选择或管理模拟交易角色</SheetDescription>
        </SheetHeader>
        <RoleListBody {...props} />
      </SheetContent>
    </Sheet>
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
      <div className={`font-number mt-0.5 truncate text-[15px] leading-tight font-semibold ${toneCls}`} title={value}>
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
      <div className="grid grid-cols-2 gap-3 lg:grid-cols-3 2xl:grid-cols-5">
        {Array.from({ length: 5 }).map((_, i) => (
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
      {/* 列数按「金额放得下」定：5 列只在 2xl 起用，窄窗口退到 3/2 列 */}
      {/* 金额卡片只做概览，视觉上让位给下方的曲线/日历 */}
      <div className="grid grid-cols-2 gap-2 sm:grid-cols-3 lg:grid-cols-5">
        <StatCard label="初始资金" value={`¥${formatMoney(account.initial_capital)}`} />
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


// ── 买入 Dialog ────────────────────────────────────────

function BuyDialog({ roleName, onClose }: { roleName: string; onClose: () => void }) {
  const placeOrder = usePlaceOrder()
  const { data: account } = useRoleAccount(roleName)
  const [symbol, setSymbol] = useState("")
  const [stockName, setStockName] = useState("")
  const [shares, setShares] = useState("")
  // null = 跟随现价；"" = 用户主动清空，两者必须区分，否则价格框清不掉
  const [priceOverride, setPriceOverride] = useState<string | null>(null)
  const [reason, setReason] = useState("")
  const [attempted, setAttempted] = useState(false)
  const [formError, setFormError] = useState<string | null>(null)
  const [results, setResults] = useState<StockSearchItem[]>([])
  const [searching, setSearching] = useState(false)
  const [searched, setSearched] = useState(false)
  const [searchError, setSearchError] = useState<string | null>(null)
  // 从下拉里选中后回填代码，不应再触发一次搜索把下拉弹回来。
  // 记录要跳过的具体代码，而不是一个 boolean：连续操作时 boolean 很容易
  // 残留下来，把用户下一次真正输入也一起跳过。
  const skipSearchFor = useRef<string | null>(null)
  const commandRef = useRef<HTMLDivElement | null>(null)
  const sharesRef = useRef<HTMLInputElement | null>(null)

  // 只对合法 6 位代码拉行情；带 sh/sz 前后缀时也提取出 6 位代码
  const quoteSymbol = extractStockCode(symbol) ?? ""
  const { data: quote, isLoading: quoteLoading } = useRealtimeQuote(quoteSymbol)
  const livePx = typeof quote?.last_px === "number" && quote.last_px > 0 ? quote.last_px : null
  const suspended = !!quote?.trade_status && quote.trade_status !== "TRADE"
  // 停牌时现价不能作为成交参考，强制用户手填
  const autoPx = suspended ? null : livePx
  const price = priceOverride ?? (autoPx === null ? "" : autoPx.toFixed(2))
  // 代码填了、行情也拉完了，却还是拿不到现价：只能手动填，得说清楚
  const quoteMissing = quoteSymbol !== "" && !quoteLoading && livePx === null
  const cash = account?.cash ?? null
  const maxShares = maxBuyableShares(cash, Number(price))

  const handleSymbolChange = (value: string) => {
    setSymbol(value)
    // 代码一变，旧股票的 name / 价格立刻失效；后端会信任 stock_name，不能留旧值
    setStockName("")
    setPriceOverride(null)
    setResults([])
    setSearched(false)
    setSearchError(null)
    setSearching(value.trim() !== "")
  }

  useEffect(() => {
    const q = symbol.trim()
    if (skipSearchFor.current === q) {
      skipSearchFor.current = null
      return
    }
    skipSearchFor.current = null
    if (!q) return
    let cancelled = false
    const timer = setTimeout(async () => {
      setSearching(true)
      try {
        const items = await searchStocks(q)
        if (cancelled) return
        setResults(items)
      } catch (e) {
        if (cancelled) return
        setResults([])
        // 搜索失败必须说出来——否则下拉只会显示「无匹配股票」，
        // 用户会以为这只股票不存在（实际是后端/网络挂了）
        setSearchError(errorText(e))
      } finally {
        if (!cancelled) {
          setSearching(false)
          setSearched(true)
        }
      }
    }, 300)
    return () => {
      cancelled = true
      clearTimeout(timer)
    }
  }, [symbol])

  const pickStock = (item: StockSearchItem) => {
    skipSearchFor.current = item.code
    setSymbol(item.code)
    setStockName(item.name)
    setPriceOverride(null)
    setResults([])
    setSearched(false)
    setSearchError(null)
    // 选中时可能有旧请求还没回来，cleanup 会把它标成 cancelled、finally 不再复位，
    // 不复位 searching 的话下拉会永远停在「搜索中…」
    setSearching(false)
    // 选完股票直接接着填股数，不用再点一下输入框
    sharesRef.current?.focus()
  }

  // 搜索完成且无结果时也要展开，让用户看到「无匹配股票」而不是一片空白
  const showSuggestions = symbol.trim() !== "" && (searching || searched || results.length > 0)

  // 候选列表不是 cmdk 的输入框驱动的，方向键/回车转发给 cmdk（与聊天页斜杠菜单同一手法）
  const handleSymbolKeyDown = (e: React.KeyboardEvent<HTMLInputElement>) => {
    if (e.nativeEvent.isComposing) return
    const pickable = showSuggestions && results.length > 0
    // 回车在代码框里是「选中候选」，不是提交表单——避免刚打完代码就误下单
    if (e.key === "Enter") {
      e.preventDefault()
      if (!pickable) return
    } else if (e.key === "ArrowDown" || e.key === "ArrowUp") {
      if (!pickable) return
      e.preventDefault()
    } else {
      return
    }
    commandRef.current?.dispatchEvent(new KeyboardEvent("keydown", { key: e.key, bubbles: true }))
  }

  // ── 校验 ──
  const sharesNum = Number(shares)
  const priceNum = Number(price)
  const symbolOk = extractStockCode(symbol) !== null
  const sharesOk = shares.trim() !== "" && Number.isFinite(sharesNum) && sharesNum > 0
  const priceOk = price.trim() !== "" && Number.isFinite(priceNum) && priceNum > 0
  const lotOk = sharesOk && sharesNum % BOARD_LOT === 0
  const valid = symbolOk && sharesOk && lotOk && priceOk
  const estimated = sharesOk && priceOk ? sharesNum * priceNum : null

  const symbolError = attempted && !symbolOk ? "请输入 6 位股票代码，或从搜索结果中选择股票" : null
  const sharesError = attempted
    ? !sharesOk
      ? "股数必须为正数"
      : !lotOk
        ? `股数必须是 ${BOARD_LOT} 的整数倍`
        : null
    : null
  const priceError =
    attempted && !priceOk
      ? quoteLoading
        ? "现价获取中，请稍候或手动输入价格"
        : suspended
          ? "停牌中不自动带入现价，请手动填写"
          : "请填写买入价格"
      : null

  const quickShares = BUY_SHARE_RATIOS.map(({ label, ratio }) => ({
    label,
    shares: ratio === 1 ? maxShares : Math.floor((maxShares * ratio) / BOARD_LOT) * BOARD_LOT,
  })).filter((o) => o.shares >= BOARD_LOT)

  const handleSubmit = async () => {
    setAttempted(true)
    setFormError(null)
    if (!valid || placeOrder.isPending) return
    try {
      await placeOrder.mutateAsync({
        roleName,
        side: "buy",
        symbol: symbol.trim(),
        stockName: stockName.trim() || undefined,
        shares: sharesNum,
        price: priceNum,
        reason: reason.trim() || undefined,
      })
      onClose()
    } catch (e) {
      // 后端 400 的中文原因（如「股数必须是 100 的整数倍」「可用资金不足」）由 fetchJson 抛出
      setFormError(errorText(e))
    }
  }

  return (
    <Dialog open onOpenChange={(open) => !open && onClose()}>
      <DialogContent className="sm:max-w-md">
        <DialogHeader>
          <DialogTitle>买入</DialogTitle>
          <DialogDescription className="sr-only">手动记录一笔买入成交</DialogDescription>
        </DialogHeader>
        <form
          className="flex flex-col gap-3"
          onSubmit={(e) => {
            e.preventDefault()
            void handleSubmit()
          }}
          onKeyDown={(e) => {
            // 中文输入法用 Enter 选词时不要顺带提交表单
            if (e.key === "Enter" && e.nativeEvent.isComposing) e.preventDefault()
          }}
        >
          <div className="flex flex-col gap-1.5">
            <label className="text-secondary text-caption">股票代码 / 名称</label>
            <Input
              value={symbol}
              onChange={(e) => handleSymbolChange(e.target.value)}
              onKeyDown={handleSymbolKeyDown}
              placeholder="输入代码或名称，如 300272 / 开能健康"
              autoComplete="off"
              aria-invalid={symbolError ? true : undefined}
              className="font-number text-caption"
              autoFocus
            />
            {symbolError && <FieldError>{symbolError}</FieldError>}
            {showSuggestions && (
              <div className="border-hairline bg-elevated overflow-hidden rounded-lg border">
                <Command ref={commandRef} shouldFilter={false} className="bg-transparent">
                  <CommandList className="max-h-52">
                    {searchError ? (
                      <div className="text-danger text-caption px-3 py-3">
                        股票搜索失败：{searchError}
                      </div>
                    ) : (
                      <CommandEmpty className="text-secondary text-caption py-3">
                        {searching ? "搜索中…" : "无匹配股票，可直接输入 6 位代码"}
                      </CommandEmpty>
                    )}
                    {results.map((r) => (
                      <CommandItem
                        key={r.code}
                        value={r.code}
                        onSelect={() => pickStock(r)}
                        className="hover:bg-hover data-selected:bg-hover text-caption flex cursor-pointer items-center gap-2 px-3 py-2"
                      >
                        <span className="font-number text-primary">{r.code}</span>
                        <span className="text-secondary truncate">{r.name}</span>
                      </CommandItem>
                    ))}
                  </CommandList>
                </Command>
              </div>
            )}
          </div>

          <div className="flex flex-col gap-1.5">
            <label className="text-secondary text-caption">股票名称（选中后自动带出）</label>
            <Input
              value={stockName}
              onChange={(e) => setStockName(e.target.value)}
              placeholder="留空则由行情自动补全"
              className="text-caption"
            />
          </div>

          <div className="grid grid-cols-2 gap-3">
            <div className="flex flex-col gap-1.5">
              <label className="text-secondary text-caption">股数（100 的整数倍）</label>
              <Input
                ref={sharesRef}
                value={shares}
                onChange={(e) => setShares(e.target.value)}
                placeholder="100"
                inputMode="numeric"
                aria-invalid={sharesError ? true : undefined}
                className="font-number text-caption"
              />
              {sharesError && <FieldError>{sharesError}</FieldError>}
            </div>
            <div className="flex flex-col gap-1.5">
              <div className="flex items-center justify-between gap-2">
                <label className="text-secondary text-caption">买入价格</label>
                {livePx !== null && !suspended && (
                  <Button
                    type="button"
                    variant="link"
                    size="xs"
                    onClick={() => setPriceOverride(null)}
                    title="使用现价"
                    className="text-tertiary hover:text-brand h-auto p-0"
                  >
                    现价 {livePx.toFixed(2)}
                  </Button>
                )}
              </div>
              <Input
                value={price}
                onChange={(e) => setPriceOverride(e.target.value)}
                placeholder={autoPx === null ? "10.50" : autoPx.toFixed(2)}
                inputMode="decimal"
                aria-invalid={priceError ? true : undefined}
                className="font-number text-caption"
              />
              {priceError && <FieldError>{priceError}</FieldError>}
              {suspended && !priceError && (
                <p className="text-warning text-caption flex items-center gap-1">
                  <AlertTriangle className="size-3 shrink-0" />
                  该股票当前停牌，未自动带入现价，请手动确认成交价
                </p>
              )}
              {quoteMissing && !priceError && !suspended && (
                <p className="text-tertiary text-caption">现价获取失败，请手动填写价格</p>
              )}
            </div>
          </div>

          {(quickShares.length > 0 || maxShares >= BOARD_LOT) && (
            <div className="flex flex-wrap items-center gap-1.5">
              {quickShares.map((o) => (
                <Button
                  key={o.label}
                  type="button"
                  variant="outline"
                  size="xs"
                  onClick={() => setShares(String(o.shares))}
                  className="text-caption h-auto px-2 py-0.5"
                >
                  {o.label}
                </Button>
              ))}
              {maxShares >= BOARD_LOT && (
                <span className="text-tertiary text-caption ml-auto">最多可买 {maxShares} 股</span>
              )}
            </div>
          )}

          <div className="flex flex-col gap-1.5">
            <label className="text-secondary text-caption">备注（可选）</label>
            <Input
              value={reason}
              onChange={(e) => setReason(e.target.value)}
              placeholder="买入理由"
              className="text-caption"
            />
          </div>

          {estimated !== null && (
            <div className="bg-canvas border-hairline text-caption flex flex-col gap-1 rounded-lg border px-3 py-2">
              <div className="flex items-center justify-between">
                <span className="text-secondary">预估买入金额</span>
                <span className="font-number text-primary font-medium">
                  ¥{formatMoney(estimated)}
                </span>
              </div>
              <div className="flex items-center justify-between">
                <span className="text-secondary">可用资金</span>
                <span className="font-number text-secondary">
                  {cash === null ? DASH : `¥${formatMoney(cash)}`}
                </span>
              </div>
              {cash !== null && estimated > cash + 0.005 && (
                <p className="text-warning">
                  超出可用资金 ¥{formatMoney(estimated - cash)}，下单会被拒绝
                </p>
              )}
            </div>
          )}

          <DialogFooter>
            {formError && <span className="text-danger text-caption mr-auto">{formError}</span>}
            <Button type="button" variant="ghost" onClick={onClose}>
              取消
            </Button>
            <Button type="submit" disabled={placeOrder.isPending}>
              {placeOrder.isPending ? "提交中…" : "买入"}
            </Button>
          </DialogFooter>
        </form>
      </DialogContent>
    </Dialog>
  )
}

// ── 卖出 Dialog ────────────────────────────────────────

function SellDialog({
  roleName,
  position,
  onClose,
}: {
  roleName: string
  position: RolePosition
  onClose: () => void
}) {
  const placeOrder = usePlaceOrder()
  const [shares, setShares] = useState(String(position.shares))
  const [price, setPrice] = useState(
    position.last_price === null ? "" : String(position.last_price)
  )
  const [reason, setReason] = useState("")
  const [attempted, setAttempted] = useState(false)
  const [formError, setFormError] = useState<string | null>(null)

  const sharesNum = Number(shares)
  const priceNum = Number(price)
  const sharesOk =
    shares.trim() !== "" &&
    Number.isFinite(sharesNum) &&
    Number.isInteger(sharesNum) &&
    sharesNum > 0 &&
    sharesNum <= position.shares
  const priceOk = price.trim() !== "" && Number.isFinite(priceNum) && priceNum > 0
  const valid = sharesOk && priceOk
  // 已实现盈亏随输入实时变化：(卖价 − 成本价) × 股数
  const estimatedPnl = sharesOk && priceOk ? (priceNum - position.cost_price) * sharesNum : null
  const estimatedAmount = sharesOk && priceOk ? sharesNum * priceNum : null
  const sellOptions = sellShareOptions(position.shares)

  const sharesError = attempted
    ? shares.trim() === ""
      ? "请输入卖出股数"
      : !Number.isFinite(sharesNum) || sharesNum <= 0
        ? "股数必须为正数"
        : !Number.isInteger(sharesNum)
          ? "股数必须是整数"
          : sharesNum > position.shares
            ? `最多可卖 ${position.shares} 股`
            : null
    : null
  const priceError = attempted && !priceOk ? "价格必须为正数" : null

  const handleSubmit = async () => {
    setAttempted(true)
    setFormError(null)
    if (!valid || placeOrder.isPending) return
    try {
      await placeOrder.mutateAsync({
        roleName,
        side: "sell",
        symbol: position.symbol,
        stockName: position.stock_name,
        shares: sharesNum,
        price: priceNum,
        reason: reason.trim() || undefined,
      })
      onClose()
    } catch (e) {
      setFormError(errorText(e))
    }
  }

  return (
    <Dialog open onOpenChange={(open) => !open && onClose()}>
      <DialogContent className="sm:max-w-md">
        <DialogHeader>
          <DialogTitle>卖出 {position.stock_name || position.symbol}</DialogTitle>
          <DialogDescription className="sr-only">手动记录一笔卖出成交</DialogDescription>
        </DialogHeader>
        <form
          className="flex flex-col gap-3"
          onSubmit={(e) => {
            e.preventDefault()
            void handleSubmit()
          }}
          onKeyDown={(e) => {
            // 中文输入法用 Enter 选词时不要顺带提交表单
            if (e.key === "Enter" && e.nativeEvent.isComposing) e.preventDefault()
          }}
        >
          <div className="bg-canvas border-hairline text-caption grid grid-cols-3 gap-2 rounded-lg border px-3 py-2">
            <div>
              <div className="text-secondary">代码</div>
              <div className="font-number text-primary">{position.symbol}</div>
            </div>
            <div>
              <div className="text-secondary">成本价</div>
              <div className="font-number text-primary">{formatMoney(position.cost_price)}</div>
            </div>
            <div>
              <div className="text-secondary">可卖股数</div>
              <div className="font-number text-primary">{position.shares}</div>
            </div>
          </div>

          <div className="grid grid-cols-2 gap-3">
            <div className="flex flex-col gap-1.5">
              <label className="text-secondary text-caption">卖出股数（可部分卖出）</label>
              <Input
                value={shares}
                onChange={(e) => setShares(e.target.value)}
                placeholder={String(position.shares)}
                inputMode="numeric"
                aria-invalid={sharesError ? true : undefined}
                className="font-number text-caption"
                autoFocus
              />
              {sharesError && <FieldError>{sharesError}</FieldError>}
            </div>
            <div className="flex flex-col gap-1.5">
              <label className="text-secondary text-caption">卖出价格</label>
              <Input
                value={price}
                onChange={(e) => setPrice(e.target.value)}
                placeholder="10.50"
                inputMode="decimal"
                aria-invalid={priceError ? true : undefined}
                className="font-number text-caption"
              />
              {priceError && <FieldError>{priceError}</FieldError>}
            </div>
          </div>

          {sellOptions.length > 1 && (
            <div className="flex flex-wrap items-center gap-1.5">
              {sellOptions.map((o) => (
                <Button
                  key={o.label}
                  type="button"
                  variant="outline"
                  size="xs"
                  onClick={() => setShares(String(o.shares))}
                  className="text-caption h-auto px-2 py-0.5"
                >
                  {o.label}
                </Button>
              ))}
              <span className="text-tertiary text-caption ml-auto">可卖 {position.shares} 股</span>
            </div>
          )}

          <div className="flex flex-col gap-1.5">
            <label className="text-secondary text-caption">备注（可选）</label>
            <Input
              value={reason}
              onChange={(e) => setReason(e.target.value)}
              placeholder="卖出理由"
              className="text-caption"
            />
          </div>

          <div className="bg-canvas border-hairline text-caption flex flex-col gap-1 rounded-lg border px-3 py-2">
            <div className="flex items-center justify-between">
              <span className="text-secondary">预估卖出金额</span>
              <span className="font-number text-primary">
                {estimatedAmount === null ? DASH : `¥${formatMoney(estimatedAmount)}`}
              </span>
            </div>
            <div className="flex items-center justify-between">
              <span className="text-secondary">预估已实现盈亏</span>
              <span className={`font-number font-medium ${pctClass(estimatedPnl)}`}>
                {estimatedPnl === null ? DASH : formatSignedMoney(estimatedPnl)}
              </span>
            </div>
          </div>

          <DialogFooter>
            {formError && <span className="text-danger text-caption mr-auto">{formError}</span>}
            <Button type="button" variant="ghost" onClick={onClose}>
              取消
            </Button>
            <Button type="submit" disabled={placeOrder.isPending}>
              {placeOrder.isPending ? "提交中…" : "卖出"}
            </Button>
          </DialogFooter>
        </form>
      </DialogContent>
    </Dialog>
  )
}

// ── Main Component ────────────────────────────────────

const TABS = [
  { id: "holdings", label: "持仓管理" },
  { id: "performance", label: "盈利分析" },
]

type HoldingView = "current" | "trades" | "history"

const HOLDING_VIEWS: { id: HoldingView; label: string }[] = [
  { id: "current", label: "当前持仓" },
  { id: "trades", label: "成交流水" },
  { id: "history", label: "平仓记录" },
]

export default function RoleView() {
  const [selectedRole, setSelectedRole] = useState<string | null>(null)
  const [search, setSearch] = useState("")
  const [tab, setTab] = useState("holdings")
  const [holdingView, setHoldingView] = useState<HoldingView>("current")
  const [showCreate, setShowCreate] = useState(false)
  const [showDeleteConfirm, setShowDeleteConfirm] = useState<string | null>(null)
  const [showResetConfirm, setShowResetConfirm] = useState<string | null>(null)
  const [buyRole, setBuyRole] = useState<string | null>(null)
  const [showRoleSheet, setShowRoleSheet] = useState(false)
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
    setShowRoleSheet(false)
    setCreateError(null)
    setShowCreate(true)
  }, [])

  const openDelete = useCallback((name: string) => {
    setShowRoleSheet(false)
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

  const roleListProps: RoleListProps = {
    roles: visibleRoles,
    selectedRole: activeRole,
    onSelect: (name) => {
      setSelectedRole(name)
      setShowCreate(false)
      setShowRoleSheet(false)
    },
    onDelete: openDelete,
    onCreate: openCreate,
    roleStats,
    search,
    setSearch,
    totalCount: visibleRoles.length,
    rolesLoading: rolesPending && visibleRoles.length === 0,
    rolesError: visibleRoles.length === 0 && rolesQueryError ? errorText(rolesQueryError) : null,
    onRetryRoles: () => void refetchRoles(),
  }

  return (
    <>
      <div className="flex h-full flex-row">
        {/* Left sidebar — role list with search + PnL stats */}
        <Sidebar {...roleListProps} />
        <MobileRoleSheet {...roleListProps} open={showRoleSheet} onOpenChange={setShowRoleSheet} />

        {/* Main Content */}
        {/* 子项必须 shrink-0：卡片带 overflow-hidden，作为列向 flex item 会被压缩到
            低于内容高度，导致曲线底部（日期横轴）和脚注被裁掉，页面却不滚动。 */}
        <main className="readable-content flex flex-1 flex-col gap-6 overflow-y-auto px-7 py-6 [&>*]:shrink-0">
          <div className="flex items-center gap-3 md:hidden">
            <Button variant="outline" size="sm" onClick={() => setShowRoleSheet(true)}>
              <Users className="size-4" />
              角色列表
            </Button>
          </div>
          {!activeRole ? (
            <div className="text-secondary text-caption flex flex-1 items-center justify-center">
              {rolesPending && visibleRoles.length === 0
                ? "加载角色…"
                : rolesQueryError && visibleRoles.length === 0
                  ? "角色列表加载失败，请检查服务端"
                  : "选择或创建一个角色"}
            </div>
          ) : (
            <>
              {/* Header + Tabs */}
              <div className="flex flex-wrap items-center justify-between gap-3">
                <div className="flex items-baseline gap-3">
                  <h1
                    className="text-hero text-primary min-w-0 truncate font-medium"
                    title={currentRole?.name ?? activeRole}
                  >
                    {currentDisplayName}
                  </h1>
                  {currentRole?.readonly && (
                    <span className="text-caption text-tertiary bg-hover rounded px-2 py-0.5">
                      只读
                    </span>
                  )}
                </div>
                <div className="bg-hover flex gap-1 rounded-lg p-1">
                  {TABS.map((t) => (
                    <Button
                      key={t.id}
                      variant="ghost"
                      size="sm"
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
              </div>

              {/* Account overview */}
              <AccountOverview
                account={account}
                loading={accountQuery.isPending}
                error={accountQuery.error ? errorText(accountQuery.error) : null}
              />

              {/* Tab: Holdings */}
              {tab === "holdings" && (
                <div className="bg-surface border-hairline overflow-hidden rounded-lg border">
                  <div className="border-hairline flex flex-wrap items-center justify-between gap-3 border-b px-5 py-4">
                    <div>
                      <h3 className="text-h2 text-primary font-medium">持仓明细</h3>
                      <p className="text-tertiary text-caption mt-0.5">
                        {holdingView === "current"
                          ? `行情每 ${ROLE_LIVE_REFRESH_MS / 1000} 秒自动刷新`
                          : "仅显示最近 100 条"}
                      </p>
                    </div>
                    <div className="flex items-center gap-3">
                      <div className="flex gap-2">
                        {HOLDING_VIEWS.map((v) => (
                          <Button
                            key={v.id}
                            variant="ghost"
                            size="xs"
                            onClick={() => setHoldingView(v.id)}
                            className={`text-caption font-medium ${
                              holdingView === v.id
                                ? "bg-brand/20 text-brand hover:bg-brand/20 hover:text-brand"
                                : "text-secondary hover:text-primary"
                            }`}
                          >
                            {v.label}
                          </Button>
                        ))}
                      </div>
                      <Button size="sm" onClick={() => activeRole && setBuyRole(activeRole)}>
                        买入
                      </Button>
                    </div>
                  </div>
                  <div className="overflow-x-auto">
                    {holdingView === "current" &&
                      (positionsQuery.isPending ? (
                        <TableStatus kind="loading" />
                      ) : positionsQuery.isError ? (
                        <TableStatus
                          kind="error"
                          message={`持仓加载失败：${errorText(positionsQuery.error)}`}
                          onRetry={() => void positionsQuery.refetch()}
                        />
                      ) : positions.length > 0 ? (
                        <table className="text-caption w-full border-collapse">
                          <thead>
                            <tr>
                              <Th>股票</Th>
                              <Th>成本价</Th>
                              <Th>现价</Th>
                              <Th>数量</Th>
                              <Th>市值</Th>
                              <Th>浮动盈亏</Th>
                              <Th>盈亏%</Th>
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
                                <Td>{formatMoney(p.cost_price)}</Td>
                                <Td>{p.last_price === null ? DASH : formatMoney(p.last_price)}</Td>
                                <Td>{p.shares}</Td>
                                <Td>
                                  {p.market_value === null ? DASH : formatMoney(p.market_value)}
                                </Td>
                                <Td className={pctClass(p.unrealized_pnl)}>
                                  {p.unrealized_pnl === null
                                    ? DASH
                                    : formatSignedMoney(p.unrealized_pnl)}
                                </Td>
                                <Td className={pctClass(p.unrealized_pct)}>
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
                        <div className="text-secondary text-caption py-12 text-center">
                          当前空仓，买入后这里会出现持仓与卖出按钮
                        </div>
                      ))}

                    {holdingView === "trades" &&
                      (tradesQuery.isPending ? (
                        <TableStatus kind="loading" />
                      ) : tradesQuery.isError ? (
                        <TableStatus
                          kind="error"
                          message={`成交流水加载失败：${errorText(tradesQuery.error)}`}
                          onRetry={() => void tradesQuery.refetch()}
                        />
                      ) : trades.length > 0 ? (
                        <table className="text-caption w-full border-collapse">
                          <thead>
                            <tr>
                              <Th>成交时间</Th>
                              <Th>股票</Th>
                              <Th>方向</Th>
                              <Th>价格</Th>
                              <Th>数量</Th>
                              <Th>成本价</Th>
                              <Th>已实现盈亏</Th>
                              <Th>备注</Th>
                            </tr>
                          </thead>
                          <tbody>
                            {trades.map((t: RoleTrade) => (
                              <tr key={t.id}>
                                <Td muted>{t.traded_at}</Td>
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
                                <Td>{formatMoney(t.price)}</Td>
                                <Td>{t.shares}</Td>
                                <Td>{formatMoney(t.cost_price)}</Td>
                                <Td className={pctClass(t.realized_pnl)}>
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
                        <div className="text-secondary text-caption py-12 text-center">
                          暂无成交流水
                        </div>
                      ))}

                    {holdingView === "history" &&
                      (historyQuery.isPending ? (
                        <TableStatus kind="loading" />
                      ) : historyQuery.isError ? (
                        <TableStatus
                          kind="error"
                          message={`平仓记录加载失败：${errorText(historyQuery.error)}`}
                          onRetry={() => void historyQuery.refetch()}
                        />
                      ) : history.length > 0 ? (
                        <table className="text-caption w-full border-collapse">
                          <thead>
                            <tr>
                              <Th>股票</Th>
                              <Th>成本价</Th>
                              <Th>出场价</Th>
                              <Th>数量</Th>
                              <Th>已实现盈亏</Th>
                              <Th>盈亏%</Th>
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
                                <Td>{formatMoney(h.cost_price)}</Td>
                                <Td>{formatMoney(h.exit_price)}</Td>
                                <Td>{h.shares}</Td>
                                <Td className={pctClass(h.realized_pnl)}>
                                  {formatSignedMoney(h.realized_pnl)}
                                </Td>
                                <Td className={pctClass(h.realized_pct)}>
                                  {formatPct(h.realized_pct)}
                                </Td>
                                <Td muted>{h.closed_at}</Td>
                                <Td reason>{h.reason}</Td>
                              </tr>
                            ))}
                          </tbody>
                        </table>
                      ) : (
                        <div className="text-secondary text-caption py-12 text-center">
                          暂无平仓记录
                        </div>
                      ))}
                  </div>
                </div>
              )}

              {/* Tab: Performance */}
              {tab === "performance" && (
                <div className="bg-surface border-hairline overflow-hidden rounded-lg border">
                  <div className="border-hairline flex flex-wrap items-baseline justify-between gap-2 border-b px-5 py-4">
                    <h3 className="text-h2 text-primary font-medium">累计盈利曲线</h3>
                    <span className="text-tertiary text-caption">
                      按当日收盘价估值，支持曲线与日历视图
                    </span>
                  </div>
                  {activeRole ? (
                    <PnlPanel roleName={activeRole} />
                  ) : (
                    <TableStatus kind="error" message="未选择角色" />
                  )}
                </div>
              )}

              {/* Action Buttons */}
              <div className="mt-auto flex gap-2">
                <Button
                  variant="outline"
                  size="xs"
                  onClick={() => activeRole && openReset(activeRole)}
                  className="hover:border-up/30 hover:text-up text-secondary"
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
              <label className="text-secondary text-caption">角色名称</label>
              <Input
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
              <label className="text-secondary text-caption">初始资金（元）</label>
              <Input
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
      {buyRole && <BuyDialog roleName={buyRole} onClose={() => setBuyRole(null)} />}
      {sellTarget && (
        <SellDialog
          roleName={sellTarget.roleName}
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
  return (
    <th
      className={`bg-ink-dim border-hairline text-secondary text-caption border-b px-4 py-3 text-left font-medium whitespace-nowrap ${
        className ?? ""
      }`}
    >
      {children}
    </th>
  )
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
  let cls = "px-4 py-3 border-b border-hairline text-primary whitespace-nowrap"
  if (bold) cls += " font-semibold font-number text-caption"
  if (muted) cls += " text-caption text-secondary"
  if (reason) cls += " max-w-sidebar whitespace-normal text-caption text-secondary leading-relaxed"
  if (className) cls += " " + className
  return <td className={cls}>{children}</td>
}
