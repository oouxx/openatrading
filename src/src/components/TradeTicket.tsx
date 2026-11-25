import { useEffect, useRef, useState } from "react"
import { Button } from "@/components/ui/button"
import { Input } from "@/components/ui/input"
import { Command, CommandEmpty, CommandItem, CommandList } from "@/components/ui/command"
import { useRoleAccount } from "@/hooks/query/useRoleQueries"
import { usePlaceOrder } from "@/hooks/query/useRoleMutations"
import { useRealtimeQuote } from "@/hooks/query/useRealtimeQuote"
import { searchStocks } from "@/lib/stock-utils"
import { formatMoney } from "@/lib/format"
import {
  BOARD_LOT,
  BUY_SHARE_RATIOS,
  extractStockCode,
  maxBuyableShares,
  sellShareOptions,
} from "@/lib/trade"
import type { RolePosition, StockSearchItem } from "@/stores/data"

/**
 * 模拟盘下单票：买入 / 卖出两个表单。
 *
 * 以前它们只存在于 `RoleView` 的两个 Dialog 里，网格想放一格"下单"就得复制一遍。
 * 抽出来之后：弹窗（模拟盘页）与网格格子（`TradePane`）渲染的是同一个表单，
 * 校验、错误文案、快捷仓位只有一份。
 */

const DASH = "—"

function errorText(error: unknown): string {
  if (error instanceof Error) return error.message
  return error ? String(error) : "未知错误"
}

function FieldError({ children }: { children: string }) {
  return <p className="text-danger text-caption mt-1">{children}</p>
}

/**
 * 下单表单里的一个字段：label（关联 input）+ 右侧内联提示 + 控件 + 错误。
 *
 * 买/卖两个表单原来各写一遍这套排布，错误提示的位置、字号、间距容易漂移。
 */
function OrderField({
  label,
  id,
  hint,
  error,
  children,
}: {
  label: string
  id: string
  /** 内联提示：现价、「最多可买 N 股」这类一眼要看到的补充值 */
  hint?: React.ReactNode
  error?: string | null
  children: React.ReactNode
}) {
  return (
    <div className="flex flex-col gap-1.5">
      <div className="flex items-baseline justify-between gap-2">
        <label htmlFor={id} className="text-secondary text-caption">
          {label}
        </label>
        {hint}
      </div>
      {children}
      {error && <FieldError>{error}</FieldError>}
    </div>
  )
}

/**
 * 下单摘要：金额这类「填完才敢点确认」的数字集中在一个块里。
 *
 * 常驻显示（字段非法时值为 —），而不是校验通过才出现——「可用资金」是决定
 * 买不买得起的前提信息，不该在填错股数时消失。
 */
function OrderSummary({
  rows,
  warn,
}: {
  rows: { label: string; value: React.ReactNode; strong?: boolean }[]
  warn?: string | null
}) {
  return (
    <div className="bg-canvas border-hairline text-caption flex flex-col gap-1 rounded-lg border px-3 py-2">
      {rows.map((r) => (
        <div key={r.label} className="flex items-center justify-between gap-3">
          <span className="text-secondary">{r.label}</span>
          <span
            className={`font-number ${r.strong ? "text-primary font-medium" : "text-secondary"}`}
          >
            {r.value}
          </span>
        </div>
      ))}
      {warn && <p className="text-warning">{warn}</p>}
    </div>
  )
}

/** 快捷仓位按钮（1/3 仓、半仓、全仓…）。 */
function QuickShares({
  options,
  value,
  onPick,
}: {
  options: { label: string; shares: number }[]
  value: string
  onPick: (shares: number) => void
}) {
  if (options.length === 0) return null
  return (
    <div className="flex flex-wrap items-center gap-1.5">
      {options.map((option) => (
        <Button
          key={option.label}
          type="button"
          variant="outline"
          size="xs"
          onClick={() => onPick(option.shares)}
          aria-pressed={value === String(option.shares)}
          className={`h-7 px-2.5 ${
            value === String(option.shares)
              ? "border-brand text-brand hover:border-brand hover:text-brand"
              : "border-hairline text-secondary hover:bg-hover"
          }`}
        >
          {option.label}
        </Button>
      ))}
    </div>
  )
}

// ── 买入 ──────────────────────────────────────────────

export function BuyTicketForm({
  roleName,
  preset,
  onSymbolChange,
  onDone,
}: {
  roleName: string
  /**
   * 已经确定的标的（网格里跟随工作区当前股票时用）。
   * 传了就隐藏搜索框、价格跟随现价；换标的由父级给 `key` 让它重挂载。
   */
  preset?: { code: string; name: string } | null
  /** 标的变了就通知外面（弹窗右侧的五档要跟着换） */
  onSymbolChange?: (code: string) => void
  onDone?: () => void
}) {
  const placeOrder = usePlaceOrder()
  const { data: account } = useRoleAccount(roleName)
  const [symbol, setSymbol] = useState(preset?.code ?? "")
  const [stockName, setStockName] = useState(preset?.name ?? "")
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
  // 候选由外部过滤，cmdk 不会在结果变化后自动重选第一行，这里自己记高亮
  const [activeCode, setActiveCode] = useState<string | null>(null)
  const highlightedCode = results.some((item) => item.code === activeCode)
    ? activeCode
    : (results[0]?.code ?? null)

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
    onSymbolChange?.(value)
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
        // 指数不能下单：`000001` 裸码落到后端就成了平安银行
        // （ETF / 可转债是场内可交易的，照常列出）
        setResults(items.filter((item) => item.kind !== "index"))
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
    const picked = item.symbol ?? item.code
    skipSearchFor.current = picked
    setSymbol(picked)
    onSymbolChange?.(picked)
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

  // 候选列表不是 cmdk 的输入框驱动的：方向键转发给 cmdk 移动高亮，
  // 回车直接选中高亮项（高亮失效时退回第一条）。
  const handleSymbolKeyDown = (e: React.KeyboardEvent<HTMLInputElement>) => {
    if (e.nativeEvent.isComposing) return
    const pickable = showSuggestions && results.length > 0
    // 回车在代码框里是「选中候选」，不是提交表单——避免刚打完代码就误下单
    if (e.key === "Enter") {
      e.preventDefault()
      if (!pickable) return
      const active = results.find((item) => item.code === highlightedCode) ?? results[0]
      if (active) pickStock(active)
      return
    }
    if (e.key === "ArrowDown" || e.key === "ArrowUp") {
      if (!pickable) return
      e.preventDefault()
      commandRef.current?.dispatchEvent(new KeyboardEvent("keydown", { key: e.key, bubbles: true }))
    }
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
      setShares("")
      setReason("")
      setAttempted(false)
      onDone?.()
    } catch (e) {
      // 后端 400 的中文原因（如「股数必须是 100 的整数倍」「可用资金不足」）由 fetchJson 抛出
      setFormError(errorText(e))
    }
  }

  return (
    <form
      className="flex min-w-0 flex-col gap-3"
      onSubmit={(e) => {
        e.preventDefault()
        void handleSubmit()
      }}
      onKeyDown={(e) => {
        // 中文输入法用 Enter 选词时不要顺带提交表单
        if (e.key === "Enter" && e.nativeEvent.isComposing) e.preventDefault()
      }}
    >
      {preset ? (
        <div className="flex items-baseline gap-2">
          <span className="text-primary text-body truncate font-medium">{preset.name}</span>
          <span className="text-tertiary font-number text-caption">
            {preset.code.replace(/^[a-z]+/i, "")}
          </span>
        </div>
      ) : (
        <OrderField label="股票" id="buy-symbol" error={symbolError}>
          <div className="relative">
            <Input
              id="buy-symbol"
              value={symbol}
              onChange={(e) => handleSymbolChange(e.target.value)}
              onKeyDown={handleSymbolKeyDown}
              placeholder="输入代码或名称，如 300272 / 开能健康"
              autoComplete="off"
              aria-invalid={symbolError ? true : undefined}
              className="font-number text-caption"
              autoFocus
            />
            {/* 搜索结果必须脱离表单文档流：列表高度不能把弹窗/格子撑大。 */}
            {showSuggestions && (
              <div className="border-hairline bg-elevated shadow-modal absolute inset-x-0 top-full z-20 mt-1 overflow-hidden rounded-lg border">
                <Command
                  ref={commandRef}
                  shouldFilter={false}
                  value={highlightedCode ?? undefined}
                  onValueChange={(value) => setActiveCode(value)}
                  className="bg-transparent"
                >
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
        </OrderField>
      )}

      {/* 名称只做确认，不再提供手填输入框：留空时后端会用行情补全，
          手填名称只会带来拼错和前后不一致 */}
      {!preset && stockName && !showSuggestions && (
        <div className="text-caption -mt-1 flex items-center gap-2">
          <span className="text-primary truncate font-medium">{stockName}</span>
        </div>
      )}

      <div className="grid grid-cols-2 gap-3">
        <OrderField
          label="股数"
          id="buy-shares"
          hint={
            cash !== null && maxShares < BOARD_LOT ? (
              <span className="text-warning text-caption">可用资金不足一手</span>
            ) : maxShares >= BOARD_LOT ? (
              <span className="text-tertiary text-caption">最多 {maxShares} 股</span>
            ) : undefined
          }
          error={sharesError}
        >
          <Input
            id="buy-shares"
            ref={sharesRef}
            value={shares}
            onChange={(e) => setShares(e.target.value)}
            placeholder="100"
            inputMode="numeric"
            aria-invalid={sharesError ? true : undefined}
            className="font-number text-caption"
          />
        </OrderField>
        <OrderField
          label="价格"
          id="buy-price"
          hint={
            /* 现价由五档面板实时给出；这里只在用户手动改过价格时给一个
               回到现价的动作，避免同一个数字在一屏里出现两次 */
            suspended ? (
              <span className="text-warning text-caption">停牌，需手填价格</span>
            ) : livePx !== null && priceOverride !== null ? (
              <Button
                type="button"
                variant="link"
                size="xs"
                onClick={() => setPriceOverride(null)}
                className="text-brand text-caption h-auto p-0"
              >
                恢复现价 {livePx.toFixed(2)}
              </Button>
            ) : quoteLoading && quoteSymbol ? (
              <span className="text-tertiary text-caption">行情加载中…</span>
            ) : quoteMissing ? (
              <span className="text-tertiary text-caption">取不到现价，请手动填写</span>
            ) : undefined
          }
          error={priceError}
        >
          <Input
            id="buy-price"
            value={price}
            onChange={(e) => setPriceOverride(e.target.value)}
            placeholder={autoPx === null ? "10.50" : autoPx.toFixed(2)}
            inputMode="decimal"
            aria-invalid={priceError ? true : undefined}
            className="font-number text-caption"
          />
        </OrderField>
      </div>

      <QuickShares options={quickShares} value={shares} onPick={(n) => setShares(String(n))} />

      <OrderField label="备注" id="buy-reason">
        <Input
          id="buy-reason"
          value={reason}
          onChange={(e) => setReason(e.target.value)}
          placeholder="记录买入理由（可选）"
          className="text-caption"
        />
      </OrderField>

      <OrderSummary
        rows={[
          {
            label: "预估金额",
            value: estimated === null ? DASH : `¥${formatMoney(estimated)}`,
            strong: true,
          },
          { label: "可用资金", value: cash === null ? DASH : `¥${formatMoney(cash)}` },
        ]}
        warn={
          cash !== null && estimated !== null && estimated > cash + 0.005
            ? `超出可用资金 ¥${formatMoney(estimated - cash)}，下单会被拒绝`
            : null
        }
      />

      <div className="flex items-center gap-2">
        {formError && <span className="text-danger text-caption mr-auto">{formError}</span>}
        <Button type="submit" disabled={placeOrder.isPending} className="ml-auto">
          {placeOrder.isPending ? "提交中…" : "买入"}
        </Button>
      </div>
    </form>
  )
}

// ── 卖出 ──────────────────────────────────────────────

export function SellTicketForm({
  roleName,
  position,
  onDone,
}: {
  roleName: string
  position: RolePosition
  onDone?: () => void
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
      setReason("")
      setAttempted(false)
      onDone?.()
    } catch (e) {
      setFormError(errorText(e))
    }
  }

  return (
    <form
      className="flex min-w-0 flex-col gap-3"
      onSubmit={(e) => {
        e.preventDefault()
        void handleSubmit()
      }}
      onKeyDown={(e) => {
        // 中文输入法用 Enter 选词时不要顺带提交表单
        if (e.key === "Enter" && e.nativeEvent.isComposing) e.preventDefault()
      }}
    >
      <div className="flex items-baseline gap-2">
        <span className="text-primary text-body truncate font-medium">
          {position.stock_name || position.symbol}
        </span>
        <span className="text-tertiary font-number text-caption">
          {position.symbol} · 成本 {formatMoney(position.cost_price)} · 持仓 {position.shares} 股
        </span>
      </div>

      <div className="grid grid-cols-2 gap-3">
        <OrderField
          label="股数"
          id="sell-shares"
          hint={<span className="text-tertiary text-caption">最多 {position.shares} 股</span>}
          error={sharesError}
        >
          <Input
            id="sell-shares"
            value={shares}
            onChange={(e) => setShares(e.target.value)}
            placeholder={String(position.shares)}
            inputMode="numeric"
            aria-invalid={sharesError ? true : undefined}
            className="font-number text-caption"
          />
        </OrderField>
        <OrderField
          label="价格"
          id="sell-price"
          hint={<span className="text-tertiary text-caption">现价与五档见五档格</span>}
          error={priceError}
        >
          <Input
            id="sell-price"
            value={price}
            onChange={(e) => setPrice(e.target.value)}
            placeholder="10.50"
            inputMode="decimal"
            aria-invalid={priceError ? true : undefined}
            className="font-number text-caption"
          />
        </OrderField>
      </div>

      <QuickShares options={sellOptions} value={shares} onPick={(n) => setShares(String(n))} />

      <OrderField label="备注" id="sell-reason">
        <Input
          id="sell-reason"
          value={reason}
          onChange={(e) => setReason(e.target.value)}
          placeholder="记录卖出理由（可选）"
          className="text-caption"
        />
      </OrderField>

      <OrderSummary
        rows={[
          {
            label: "成交金额",
            value: estimatedAmount === null ? DASH : `¥${formatMoney(estimatedAmount)}`,
            strong: true,
          },
          {
            label: "预估盈亏",
            value:
              estimatedPnl === null
                ? DASH
                : `${estimatedPnl >= 0 ? "+" : "-"}¥${formatMoney(Math.abs(estimatedPnl))}`,
          },
        ]}
      />

      <div className="flex items-center gap-2">
        {formError && <span className="text-danger text-caption mr-auto">{formError}</span>}
        <Button type="submit" disabled={placeOrder.isPending} className="ml-auto">
          {placeOrder.isPending ? "提交中…" : "卖出"}
        </Button>
      </div>
    </form>
  )
}
