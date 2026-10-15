import { useState, useEffect, useMemo, useCallback, useRef } from "react"
import { AnimatePresence } from "framer-motion"
import { useQueries } from "@tanstack/react-query"
import { AlertTriangle } from "lucide-react"
import { Button } from "@/components/ui/button"
import { Input } from "@/components/ui/input"
import { Skeleton } from "@/components/ui/skeleton"
import {
  Dialog,
  DialogContent,
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from "@/components/ui/dialog"
import {
  Command,
  CommandEmpty,
  CommandItem,
  CommandList,
} from "@/components/ui/command"
import {
  fetchJson,
  type RoleEntry,
  type RolePosition,
  type RoleAccount,
  type RoleTrade,
  type ClosedPosition,
  type PnlPoint,
} from "@/stores/data"
import { searchStocks } from "@/lib/stock-utils"
import { roleKeys } from "@/hooks/query/queryKeys"
import {
  useRoleList,
  useRolePositions,
  useRoleAccount,
  useRoleTrades,
  useRoleHistory,
  useRolePnlHistory,
} from "@/hooks/query/useRoleQueries"
import {
  useCreateRole,
  useDeleteRole,
  useResetRole,
  usePlaceOrder,
} from "@/hooks/query/useRoleMutations"
import KlineDrawer from "@/components/KlineDrawer"

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
function formatPct(v: number): string {
  return `${v >= 0 ? "+" : ""}${v.toFixed(2)}%`
}

/** 涨跌色：正数走 `text-up`（涨—红），负数走 `text-down`（跌—绿） */
function pctClass(v: number | null): string {
  if (v === null || v === 0) return "text-primary"
  return v > 0 ? "text-up" : "text-down"
}

function FieldError({ children }: { children: string }) {
  return <p className="text-danger text-caption mt-1">{children}</p>
}

// ── Sidebar ───────────────────────────────────────────

function Sidebar({
  roles,
  selectedRole,
  onSelect,
  onDelete,
  onCreate,
  rolePnl,
  rolePositionCounts,
  search,
  setSearch,
  totalCount,
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
    const displayName = r.display_name || r.name
    return displayName.toLowerCase().includes(search.toLowerCase())
  })

  return (
    <aside className="bg-surface border-hairline hidden w-sidebar min-w-sidebar flex-col overflow-y-auto border-r md:flex">
      <div className="border-hairline flex items-center justify-between gap-2 border-b p-4">
        <h2 className="text-h2 font-medium text-primary">角色列表</h2>
        <span className="text-secondary text-caption">{totalCount} 个</span>
      </div>
      <div className="border-hairline border-b px-4 py-3">
        <input
          type="text"
          placeholder="搜索角色…"
          value={search}
          onChange={(e) => setSearch(e.target.value)}
          className="bg-canvas border-hairline focus:border-brand w-full rounded-lg border px-3 py-2 text-caption text-primary transition-colors outline-none"
        />
      </div>
      <div className="flex-1 overflow-y-auto py-2">
        {filtered.map((role) => {
          const displayName = role.display_name || role.name
          const pnl = rolePnl[role.name] || 0
          const count = rolePositionCounts[role.name] || 0
          return (
            <div
              key={role.name}
              className={`flex cursor-pointer items-center justify-between gap-3 border-l-[3px] px-4 py-3 transition-colors ${
                selectedRole === role.name
                  ? "border-l-ai bg-brand/10"
                  : "hover:bg-hover border-l-transparent"
              }`}
              onClick={() => onSelect(role.name)}
            >
              <div className="min-w-0 flex-1">
                <div className="truncate text-caption font-medium text-primary">{displayName}</div>
                <div className="text-secondary mt-0.5 flex gap-3 text-caption">
                  <span>{count} 持仓</span>
                  <span
                    className={`font-number text-caption font-semibold ${isNaN(pnl) ? "text-secondary" : pnl > 0 ? "text-up" : pnl < 0 ? "text-down" : "text-primary"}`}
                  >
                    {count > 0
                      ? isNaN(pnl)
                        ? "N/A"
                        : `${pnl >= 0 ? "+" : ""}${pnl.toFixed(1)}%`
                      : "空仓"}
                  </span>
                </div>
              </div>
              <button
                onClick={(e) => {
                  e.stopPropagation()
                  onDelete(role.name)
                }}
                className="hover:bg-up/10 hover:text-up text-secondary flex size-6 shrink-0 cursor-pointer items-center justify-center rounded-md border-none bg-transparent text-h2 opacity-0 transition-all"
                style={{ opacity: selectedRole === role.name ? 1 : undefined }}
              >
                ×
              </button>
            </div>
          )
        })}
        <button
          onClick={onCreate}
          className="hover:bg-hover hover:text-brand text-secondary w-full cursor-pointer px-4 py-3 text-left text-caption transition-colors"
        >
          + 新建角色
        </button>
      </div>
    </aside>
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
    <div className="bg-surface border-hairline rounded-lg border px-4 py-4">
      <div className="text-secondary flex items-center justify-between gap-2 text-caption tracking-wider uppercase">
        <span>{label}</span>
        {action}
      </div>
      <div className={`mt-1 font-number text-h1 font-semibold ${toneCls}`}>{value}</div>
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
    return (
      <div className="border-danger/30 bg-danger/10 text-danger rounded-lg border px-3 py-2 text-caption">
        账户信息加载失败：{error}
      </div>
    )
  }

  if (loading || !account) {
    return (
      <div className="grid grid-cols-2 gap-3 md:grid-cols-5">
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
        <div className="border-warning/30 bg-warning/10 text-warning flex items-center gap-2 rounded-lg border px-3 py-2 text-caption">
          <AlertTriangle className="size-3.5 shrink-0" />
          部分持仓行情获取失败，持仓市值与权益为不完整值，仅供参考
        </div>
      )}
      <div className="grid grid-cols-2 gap-3 md:grid-cols-5">
        <StatCard
          label="初始资金"
          value={`¥${formatMoney(account.initial_capital)}`}
        />
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

// ── P&L Chart (SVG) — uses backend pnl-history endpoint ──

function PnlChart({ points, initialCapital }: { points: PnlPoint[]; initialCapital: number }) {
  if (points.length === 0) {
    return (
      <div className="text-secondary py-12 text-center text-caption">暂无交易数据，无法生成盈利曲线</div>
    )
  }
  const W = 700
  const H = 260
  const padL = 50
  const padR = 20
  const padT = 20
  const padB = 30
  const plotW = W - padL - padR
  const plotH = H - padT - padB

  // Map each point to equity delta from initial capital
  const values = points.map((p) => (p.equity ?? initialCapital) - initialCapital)
  const minVal = Math.min(0, ...values) - 100
  const maxVal = Math.max(0, ...values) + 100
  const range = maxVal - minVal || 1
  const x = (i: number) => padL + (i / Math.max(points.length - 1, 1)) * plotW
  const y = (v: number) => padT + (1 - (v - minVal) / range) * plotH
  const zeroY = y(0)

  let pathD = ""
  let areaD = ""
  points.forEach((p, i) => {
    const cmd = i === 0 ? "M" : "L"
    const val = (p.equity ?? initialCapital) - initialCapital
    const xv = x(i).toFixed(1)
    const yv = y(val).toFixed(1)
    pathD += `${cmd}${xv},${yv} `
    areaD += `${cmd}${xv},${yv} `
  })
  areaD += `L${x(points.length - 1).toFixed(1)},${zeroY} L${x(0).toFixed(1)},${zeroY} Z`

  // Y labels
  let yLabels = ""
  for (let i = 0; i <= 4; i++) {
    const val = minVal + (range * i) / 4
    const ly = y(val)
    yLabels += `<text x="${padL - 8}" y="${ly + 3}" text-anchor="end" font-size="9" fill="var(--color-tertiary)">¥${val.toFixed(0)}</text>`
    yLabels += `<line x1="${padL}" y1="${ly}" x2="${W - padR}" y2="${ly}" stroke="rgba(255,255,255,0.06)" stroke-width="0.5" stroke-dasharray="3,3"/>`
  }

  // X labels — show MM-DD
  const labelStep = Math.max(1, Math.floor(points.length / 5))
  const xLabels = points
    .filter((_, i) => i % labelStep === 0 || i === points.length - 1)
    .map((p) => {
      const idx = points.indexOf(p)
      return `<text x="${x(idx).toFixed(1)}" y="${H - 8}" text-anchor="middle" font-size="9" fill="var(--color-tertiary)">${p.date.slice(5)}</text>`
    })
    .join("")

  // Highlight final point with a larger circle if it reflects live mark-to-market
  const circles = points
    .map((p, i) => {
      const isLast = i === points.length - 1
      const r = isLast ? "4.5" : "3"
      const fill = isLast ? "var(--color-accent)" : "var(--color-accent)"
      const stroke = "var(--color-primary)"
      const val = (p.equity ?? initialCapital) - initialCapital
      return `<circle cx="${x(i).toFixed(1)}" cy="${y(val).toFixed(1)}" r="${r}" fill="${fill}" stroke="${stroke}" stroke-width="1.5"/>`
    })
    .join("")

  return (
    <svg
      viewBox={`0 0 ${W} ${H}`}
      style={{ width: "100%", height: 280 }}
      dangerouslySetInnerHTML={{
        __html: `
      ${yLabels}
      <line x1="${padL}" y1="${zeroY}" x2="${W - padR}" y2="${zeroY}" stroke="var(--color-hairline)" stroke-width="1" stroke-dasharray="6,4"/>
      <path d="${areaD}" fill="var(--color-accent)" opacity="0.08"/>
      <path d="${pathD}" fill="none" stroke="var(--color-accent)" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"/>
      ${circles}
      ${xLabels}
    `,
      }}
    />
  )
}

// ── 买入 Dialog ────────────────────────────────────────

function BuyDialog({
  roleName,
  onClose,
}: {
  roleName: string
  onClose: () => void
}) {
  const placeOrder = usePlaceOrder()
  const [symbol, setSymbol] = useState("")
  const [stockName, setStockName] = useState("")
  const [shares, setShares] = useState("")
  const [price, setPrice] = useState("")
  const [reason, setReason] = useState("")
  const [attempted, setAttempted] = useState(false)
  const [formError, setFormError] = useState<string | null>(null)
  const [results, setResults] = useState<{ code: string; name: string }[]>([])
  const [searching, setSearching] = useState(false)
  const [searched, setSearched] = useState(false)
  // 从下拉里选中后回填代码，不应再触发一次搜索把下拉弹回来
  const skipNextSearch = useRef(false)

  const handleSymbolChange = (value: string) => {
    setSymbol(value)
    setSearched(false)
    if (value.trim() === "") setResults([])
  }

  useEffect(() => {
    if (skipNextSearch.current) {
      skipNextSearch.current = false
      return
    }
    const q = symbol.trim()
    if (!q) return
    let cancelled = false
    const timer = setTimeout(async () => {
      setSearching(true)
      try {
        const items = await searchStocks(q)
        if (!cancelled) setResults(items)
      } catch {
        if (!cancelled) setResults([])
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

  const pickStock = (item: { code: string; name: string }) => {
    skipNextSearch.current = true
    setSymbol(item.code)
    setStockName(item.name)
    setResults([])
    setSearched(false)
  }

  // ── 校验 ──
  const sharesNum = Number(shares)
  const priceNum = Number(price)
  const symbolOk = symbol.trim() !== ""
  const sharesOk = shares.trim() !== "" && Number.isFinite(sharesNum) && sharesNum > 0
  const priceOk = price.trim() !== "" && Number.isFinite(priceNum) && priceNum > 0
  const lotOk = sharesOk && sharesNum % 100 === 0
  const valid = symbolOk && sharesOk && lotOk && priceOk

  const symbolError = attempted && !symbolOk ? "请输入股票代码" : null
  const sharesError = attempted
    ? !sharesOk
      ? "股数必须为正数"
      : !lotOk
        ? "股数必须是 100 的整数倍"
        : null
    : null
  const priceError = attempted && !priceOk ? "价格必须为正数" : null

  const handleSubmit = async () => {
    setAttempted(true)
    setFormError(null)
    if (!valid) return
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
      // 后端 400 的中文原因（如「股数必须是 100 的整数倍」）由 fetchJson 抛出
      setFormError(e instanceof Error ? e.message : String(e))
    }
  }

  // 搜索完成且无结果时也要展开，让用户看到「无匹配股票」而不是一片空白
  const showSuggestions = symbol.trim() !== "" && (searching || searched || results.length > 0)

  return (
    <Dialog open onOpenChange={(open) => !open && onClose()}>
      <DialogContent className="sm:max-w-md">
        <DialogHeader>
          <DialogTitle>买入</DialogTitle>
        </DialogHeader>
        <div className="flex flex-col gap-3">
          <div className="flex flex-col gap-1.5">
            <label className="text-secondary text-caption">股票代码</label>
            <Input
              value={symbol}
              onChange={(e) => handleSymbolChange(e.target.value)}
              placeholder="例如 300272"
              aria-invalid={symbolError ? true : undefined}
              className="font-number text-caption"
              autoFocus
            />
            {symbolError && <FieldError>{symbolError}</FieldError>}
            {showSuggestions && (
              <div className="border-hairline bg-elevated overflow-hidden rounded-lg border">
                <Command shouldFilter={false} className="bg-transparent">
                  <CommandList className="max-h-52">
                    <CommandEmpty className="text-secondary text-caption py-3">
                      {searching ? "搜索中…" : "无匹配股票"}
                    </CommandEmpty>
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
            <label className="text-secondary text-caption">股票名称（可选）</label>
            <Input
              value={stockName}
              onChange={(e) => setStockName(e.target.value)}
              placeholder="选中搜索结果后自动带出"
              className="text-caption"
            />
          </div>

          <div className="grid grid-cols-2 gap-3">
            <div className="flex flex-col gap-1.5">
              <label className="text-secondary text-caption">股数（100 的整数倍）</label>
              <Input
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
              <label className="text-secondary text-caption">买入价格</label>
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

          <div className="flex flex-col gap-1.5">
            <label className="text-secondary text-caption">备注（可选）</label>
            <Input
              value={reason}
              onChange={(e) => setReason(e.target.value)}
              placeholder="买入理由"
              className="text-caption"
            />
          </div>

          {valid && (
            <div className="bg-canvas border-hairline flex items-center justify-between rounded-lg border px-3 py-2 text-caption">
              <span className="text-secondary">预估买入金额</span>
              <span className="font-number text-primary font-medium">
                ¥{formatMoney(sharesNum * priceNum)}
              </span>
            </div>
          )}
        </div>
        <DialogFooter>
          {formError && <span className="text-danger text-caption mr-auto">{formError}</span>}
          <Button variant="ghost" onClick={onClose}>
            取消
          </Button>
          <Button onClick={() => void handleSubmit()} disabled={placeOrder.isPending}>
            {placeOrder.isPending ? "提交中…" : "买入"}
          </Button>
        </DialogFooter>
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
  const [price, setPrice] = useState(position.last_price === null ? "" : String(position.last_price))
  const [reason, setReason] = useState("")
  const [attempted, setAttempted] = useState(false)
  const [formError, setFormError] = useState<string | null>(null)

  const sharesNum = Number(shares)
  const priceNum = Number(price)
  const sharesOk =
    shares.trim() !== "" && Number.isFinite(sharesNum) && sharesNum > 0 && sharesNum <= position.shares
  const priceOk = price.trim() !== "" && Number.isFinite(priceNum) && priceNum > 0
  const valid = sharesOk && priceOk

  const sharesError = attempted
    ? shares.trim() === ""
      ? "请输入卖出股数"
      : !Number.isFinite(sharesNum) || sharesNum <= 0
        ? "股数必须为正数"
        : sharesNum > position.shares
          ? `最多可卖 ${position.shares} 股`
          : null
    : null
  const priceError = attempted && !priceOk ? "价格必须为正数" : null

  // 预估已实现盈亏随输入实时变化：(卖价 − 成本价) × 股数
  const estimatedPnl = sharesOk && priceOk ? (priceNum - position.cost_price) * sharesNum : null

  const handleSubmit = async () => {
    setAttempted(true)
    setFormError(null)
    if (!valid) return
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
      setFormError(e instanceof Error ? e.message : String(e))
    }
  }

  return (
    <Dialog open onOpenChange={(open) => !open && onClose()}>
      <DialogContent className="sm:max-w-md">
        <DialogHeader>
          <DialogTitle>卖出 {position.stock_name}</DialogTitle>
        </DialogHeader>
        <div className="flex flex-col gap-3">
          <div className="bg-canvas border-hairline grid grid-cols-3 gap-2 rounded-lg border px-3 py-2 text-caption">
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

          <div className="flex flex-col gap-1.5">
            <label className="text-secondary text-caption">备注（可选）</label>
            <Input
              value={reason}
              onChange={(e) => setReason(e.target.value)}
              placeholder="卖出理由"
              className="text-caption"
            />
          </div>

          <div className="bg-canvas border-hairline flex items-center justify-between rounded-lg border px-3 py-2 text-caption">
            <span className="text-secondary">预估已实现盈亏</span>
            <span className={`font-number font-medium ${pctClass(estimatedPnl)}`}>
              {estimatedPnl === null ? DASH : formatSignedMoney(estimatedPnl)}
            </span>
          </div>
        </div>
        <DialogFooter>
          {formError && <span className="text-danger text-caption mr-auto">{formError}</span>}
          <Button variant="ghost" onClick={onClose}>
            取消
          </Button>
          <Button onClick={() => void handleSubmit()} disabled={placeOrder.isPending}>
            {placeOrder.isPending ? "提交中…" : "卖出"}
          </Button>
        </DialogFooter>
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
  { id: "history", label: "历史持仓" },
]

export default function RoleView() {
  const [selectedRole, setSelectedRole] = useState<string | null>(null)
  const [search, setSearch] = useState("")
  const [tab, setTab] = useState("holdings")
  const [holdingView, setHoldingView] = useState<HoldingView>("current")
  const [showCreate, setShowCreate] = useState(false)
  const [showDeleteConfirm, setShowDeleteConfirm] = useState<string | null>(null)
  const [showResetConfirm, setShowResetConfirm] = useState<string | null>(null)
  const [showBuy, setShowBuy] = useState(false)
  const [sellTarget, setSellTarget] = useState<RolePosition | null>(null)
  const [newName, setNewName] = useState("")
  const [selectedStock, setSelectedStock] = useState<{ symbol: string; name: string } | null>(null)

  const { data: roles = [] } = useRoleList()
  const { data: positions = [] } = useRolePositions(selectedRole ?? "")
  const {
    data: account,
    isPending: accountPending,
    error: accountError,
  } = useRoleAccount(selectedRole ?? "")
  const { data: trades = [] } = useRoleTrades(selectedRole ?? "")
  const { data: history = [] } = useRoleHistory(selectedRole ?? "")
  const { data: pnlPoints = [] } = useRolePnlHistory(selectedRole ?? "")

  const createRoleMutation = useCreateRole()
  const deleteRoleMutation = useDeleteRole()
  const resetRoleMutation = useResetRole()

  // Select first role on load — mount initialization
  useEffect(() => {
    if (roles.length > 0 && !selectedRole) {
      // eslint-disable-next-line react-hooks/set-state-in-effect
      setSelectedRole(roles[0].name)
    }
  }, [roles])

  const currentRole = roles.find((r) => r.name === selectedRole)

  // Fetch all roles' positions for sidebar PnL stats
  const positionQueries = useQueries({
    queries: roles.map((r) => ({
      queryKey: roleKeys.positions(r.name),
      queryFn: () => fetchJson<RolePosition[]>(`/api/roles/${r.name}/positions`),
      staleTime: 5 * 60 * 1000,
      enabled: !!r.name,
    })),
  })

  const rolePnl = useMemo(() => {
    const result: Record<string, number> = {}
    roles.forEach((r, i) => {
      const pos = positionQueries[i]?.data
      if (!pos?.length) {
        result[r.name] = 0
        return
      }
      // 只统计有行情的持仓；全部缺行情时给 NaN，让 UI 显示 N/A 而不是 0%
      let totalValue = 0
      let totalCost = 0
      for (const p of pos) {
        if (p.last_price === null) continue
        totalValue += p.last_price * p.shares
        totalCost += p.cost_price * p.shares
      }
      result[r.name] = totalCost > 0 ? ((totalValue - totalCost) / totalCost) * 100 : Number.NaN
    })
    return result
  }, [roles, positionQueries])

  const rolePositionCounts = useMemo(() => {
    const result: Record<string, number> = {}
    roles.forEach((r, i) => {
      result[r.name] = positionQueries[i]?.data?.length || 0
    })
    return result
  }, [roles, positionQueries])

  const createRole = useCallback(async () => {
    const name = newName.trim()
    if (!name) return
    const id =
      name
        .toLowerCase()
        .replace(/[^a-z0-9一-鿿_-]/g, "_")
        .replace(/_+/g, "_")
        .replace(/^_|_$/g, "")
        .slice(0, 64) || "role_" + Date.now().toString(36)
    await createRoleMutation.mutateAsync({
      name: id,
      displayName: name,
    })
    setShowCreate(false)
    setNewName("")
    setTimeout(() => setSelectedRole(id), 100)
  }, [newName, createRoleMutation])

  const deleteRole = useCallback(
    async (name: string) => {
      await deleteRoleMutation.mutateAsync(name)
      setShowDeleteConfirm(null)
      if (selectedRole === name) setSelectedRole(null)
    },
    [selectedRole, deleteRoleMutation]
  )

  const resetRole = useCallback(
    async (name: string) => {
      await resetRoleMutation.mutateAsync(name)
      setShowResetConfirm(null)
    },
    [resetRoleMutation]
  )

  return (
    <>
      <div className="flex h-full flex-row">
        {/* Left sidebar — role list with search + PnL stats */}
        <Sidebar
          roles={roles}
          selectedRole={selectedRole}
          onSelect={(name) => {
            setSelectedRole(name)
            setShowCreate(false)
          }}
          onDelete={(name) => setShowDeleteConfirm(name)}
          onCreate={() => setShowCreate(true)}
          rolePnl={rolePnl}
          rolePositionCounts={rolePositionCounts}
          search={search}
          setSearch={setSearch}
          totalCount={roles.length}
        />

        {/* Main Content */}
        <main className="readable-content flex flex-1 flex-col gap-5 overflow-y-auto px-7 py-6">
          {!selectedRole ? (
            <div className="text-secondary flex flex-1 items-center justify-center text-caption">
              选择或创建一个角色
            </div>
          ) : (
            <>
              {/* Header + Tabs */}
              <div className="flex flex-wrap items-center justify-between gap-3">
                <div className="flex items-baseline gap-3">
                  <h1 className="text-hero font-medium text-primary">
                    {currentRole?.name}
                  </h1>
                  {currentRole?.readonly && (
                    <span className="rounded bg-white/10 px-2 py-0.5 text-caption text-primary/50">
                      只读
                    </span>
                  )}
                </div>
                <div className="bg-hover flex gap-1 rounded-lg p-1">
                  {TABS.map((t) => (
                    <button
                      key={t.id}
                      onClick={() => setTab(t.id)}
                      className={`cursor-pointer rounded-lg px-4 py-2 text-caption font-medium transition-colors ${
                        tab === t.id
                          ? "bg-surface text-primary shadow-card"
                          : "text-secondary hover:text-primary"
                      }`}
                    >
                      {t.label}
                    </button>
                  ))}
                </div>
              </div>

              {/* Account overview */}
              <AccountOverview
                account={account}
                loading={accountPending}
                error={accountError instanceof Error ? accountError.message : null}
              />

              {/* Tab: Holdings */}
              {tab === "holdings" && (
                <div className="bg-surface border-hairline overflow-hidden rounded-lg border">
                  <div className="border-hairline flex flex-wrap items-center justify-between gap-3 border-b px-5 py-4">
                    <h3 className="text-h2 font-medium text-primary">持仓明细</h3>
                    <div className="flex items-center gap-3">
                      <div className="flex gap-2">
                        {HOLDING_VIEWS.map((v) => (
                          <button
                            key={v.id}
                            onClick={() => setHoldingView(v.id)}
                            className={`cursor-pointer rounded-md px-3 py-1 text-caption font-medium transition-colors ${
                              holdingView === v.id
                                ? "bg-brand/20 text-brand"
                                : "text-secondary hover:text-primary"
                            }`}
                          >
                            {v.label}
                          </button>
                        ))}
                      </div>
                      <Button size="sm" onClick={() => setShowBuy(true)}>
                        买入
                      </Button>
                    </div>
                  </div>
                  <div className="overflow-x-auto">
                    {holdingView === "current" && (
                      positions.length > 0 ? (
                        <table className="w-full border-collapse text-caption">
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
                              <Th>操作</Th>
                            </tr>
                          </thead>
                          <tbody>
                            {positions.map((p: RolePosition) => (
                              <tr key={p.symbol}>
                                <Td bold>
                                  <button
                                    className="hover:text-brand cursor-pointer text-left transition-colors"
                                    onClick={() =>
                                      setSelectedStock({ symbol: p.symbol, name: p.stock_name })
                                    }
                                  >
                                    {p.stock_name}
                                  </button>
                                  <span className="text-secondary ml-2 font-number text-caption">
                                    {p.symbol}
                                  </span>
                                </Td>
                                <Td>{formatMoney(p.cost_price)}</Td>
                                <Td>{p.last_price === null ? DASH : formatMoney(p.last_price)}</Td>
                                <Td>{p.shares}</Td>
                                <Td>{p.market_value === null ? DASH : formatMoney(p.market_value)}</Td>
                                <Td className={pctClass(p.unrealized_pnl)}>
                                  {p.unrealized_pnl === null
                                    ? DASH
                                    : formatSignedMoney(p.unrealized_pnl)}
                                </Td>
                                <Td className={pctClass(p.unrealized_pct)}>
                                  {p.unrealized_pct === null ? DASH : formatPct(p.unrealized_pct)}
                                </Td>
                                <Td muted>{p.opened_at}</Td>
                                <Td>
                                  <Button
                                    variant="outline"
                                    size="xs"
                                    onClick={() => setSellTarget(p)}
                                  >
                                    卖出
                                  </Button>
                                </Td>
                              </tr>
                            ))}
                          </tbody>
                        </table>
                      ) : (
                        <div className="text-secondary py-12 text-center text-caption">空仓</div>
                      )
                    )}

                    {holdingView === "trades" && (
                      trades.length > 0 ? (
                        <table className="w-full border-collapse text-caption">
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
                                  <button
                                    className="hover:text-brand cursor-pointer text-left transition-colors"
                                    onClick={() =>
                                      setSelectedStock({ symbol: t.symbol, name: t.stock_name })
                                    }
                                  >
                                    {t.stock_name}
                                  </button>
                                  <span className="text-secondary ml-2 font-number text-caption">
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
                        <div className="text-secondary py-12 text-center text-caption">
                          暂无成交流水
                        </div>
                      )
                    )}

                    {holdingView === "history" && (
                      history.length > 0 ? (
                        <table className="w-full border-collapse text-caption">
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
                                  <button
                                    className="hover:text-brand cursor-pointer text-left transition-colors"
                                    onClick={() =>
                                      setSelectedStock({ symbol: h.symbol, name: h.stock_name })
                                    }
                                  >
                                    {h.stock_name}
                                  </button>
                                  <span className="text-secondary ml-2 font-number text-caption">
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
                        <div className="text-secondary py-12 text-center text-caption">
                          暂无历史持仓记录
                        </div>
                      )
                    )}
                  </div>
                </div>
              )}

              {/* Tab: Performance */}
              {tab === "performance" && (
                <div className="bg-surface border-hairline overflow-hidden rounded-lg border">
                  <div className="border-hairline border-b px-5 py-4">
                    <h3 className="text-h2 font-medium text-primary">累计盈利曲线</h3>
                  </div>
                  {/* 基准线取自账户的初始资金，账户未就绪时先占位，避免用 0 画出一条错误曲线 */}
                  {account ? (
                    <PnlChart points={pnlPoints} initialCapital={account.initial_capital} />
                  ) : (
                    <div className="px-5 py-6">
                      <Skeleton className="h-[280px] w-full" />
                    </div>
                  )}
                  <div className="border-hairline text-secondary flex gap-5 border-t px-5 py-3 text-caption">
                    <span className="flex items-center gap-2">
                      <span className="bg-brand inline-block size-2 rounded-full" />
                      累计盈亏
                    </span>
                    <span className="flex items-center gap-2">
                      <span
                        className="inline-block size-2 rounded-full"
                        style={{ background: "var(--color-hairline)" }}
                      />
                      基准线
                    </span>
                  </div>
                </div>
              )}

              {/* Action Buttons */}
              <div className="mt-auto flex gap-2">
                <button
                  onClick={() => setShowResetConfirm(selectedRole)}
                  className="border-hairline hover:border-up/30 hover:text-up text-secondary cursor-pointer rounded-md border px-3 py-1 text-caption transition-colors"
                >
                  重置账户
                </button>
              </div>
            </>
          )}
        </main>
      </div>

      {/* Create Modal */}
      <Dialog open={showCreate} onOpenChange={setShowCreate}>
        <DialogContent className="sm:max-w-md">
          <DialogHeader>
            <DialogTitle>新建角色</DialogTitle>
          </DialogHeader>
          <div className="flex flex-col gap-1.5">
            <label className="text-secondary text-caption">角色名称</label>
            <Input
              type="text"
              value={newName}
              onChange={(e) => setNewName(e.target.value)}
              className="text-caption"
              placeholder="例如：套利策略 v2"
              onKeyDown={(e) => e.key === "Enter" && void createRole()}
              autoFocus
            />
          </div>
          <DialogFooter>
            <Button variant="ghost" onClick={() => setShowCreate(false)}>
              取消
            </Button>
            <Button onClick={() => void createRole()}>创建</Button>
          </DialogFooter>
        </DialogContent>
      </Dialog>

      {/* Delete Confirm */}
      <Dialog
        open={showDeleteConfirm !== null}
        onOpenChange={(open) => !open && setShowDeleteConfirm(null)}
      >
        <DialogContent className="sm:max-w-sm">
          <DialogHeader>
            <DialogTitle>删除角色</DialogTitle>
          </DialogHeader>
          <p className="text-secondary text-caption">确定删除此角色？此操作不可恢复。</p>
          <DialogFooter>
            <Button variant="ghost" onClick={() => setShowDeleteConfirm(null)}>
              取消
            </Button>
            <Button
              variant="destructive"
              onClick={() => showDeleteConfirm && void deleteRole(showDeleteConfirm)}
            >
              删除
            </Button>
          </DialogFooter>
        </DialogContent>
      </Dialog>

      {/* Reset Confirm */}
      <Dialog
        open={showResetConfirm !== null}
        onOpenChange={(open) => !open && setShowResetConfirm(null)}
      >
        <DialogContent className="sm:max-w-sm">
          <DialogHeader>
            <DialogTitle>重置账户</DialogTitle>
          </DialogHeader>
          <p className="text-secondary text-caption">
            重置将清空该角色的全部持仓与成交流水，历史无法恢复；初始资金不变。
            要换一个本金请新建角色。
          </p>
          <DialogFooter>
            <Button variant="ghost" onClick={() => setShowResetConfirm(null)}>
              取消
            </Button>
            <Button
              variant="destructive"
              onClick={() => showResetConfirm && void resetRole(showResetConfirm)}
            >
              重置
            </Button>
          </DialogFooter>
        </DialogContent>
      </Dialog>

      {/* Buy / Sell dialogs — 关闭即卸载，表单状态自然重置 */}
      {showBuy && selectedRole && (
        <BuyDialog roleName={selectedRole} onClose={() => setShowBuy(false)} />
      )}
      {sellTarget && selectedRole && (
        <SellDialog
          roleName={selectedRole}
          position={sellTarget}
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

function Th({ children }: { children: React.ReactNode }) {
  return (
    <th className="bg-ink-dim border-hairline text-secondary border-b px-4 py-3 text-left text-caption font-medium tracking-wider whitespace-nowrap uppercase">
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
