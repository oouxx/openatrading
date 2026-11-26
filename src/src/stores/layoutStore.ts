import { create } from "zustand"
import { fetchLayoutSnapshot, pushLayoutSnapshot } from "@/lib/layoutsApi"
import { useActiveStockStore } from "./activeStockStore"
import { isRightPanelView } from "./rightPanelStore"

/**
 * 工作区布局模型：一棵分割树，不是固定列数。
 *
 * ```
 * split(horizontal)
 * ├── pane(chat)
 * └── split(vertical)
 *     ├── split(horizontal) [ market, market ]
 *     └── split(horizontal) [ market, market ]
 * ```
 *
 * 每个 split 都渲染成一个可拖拽的 `Group`，所以面板数量不限、宽高都能拖。
 * 三档内置布局只是三个预置的树，自定义布局走完全相同的渲染路径——"再加一种排布"
 * 只需要改数据，不需要改渲染层。
 *
 * 类型与常量在 `lib/layoutTypes.ts`，图标与渲染器在 `lib/paneRegistry.tsx`。
 */
import {
  MARKET_PANE_KINDS,
  PANE_KINDS,
  countLeaves,
  eachLeaf,
  findLeaf,
  isFollowPaneKind,
  isMarketPaneKind,
  type LayoutNode,
  type PaneKind,
  type PaneLeaf,
  type SplitNode,
  type DragonDetailTarget,
  type PaneDetail,
  type PlateDetailTarget,
  type SplitOrientation,
  type WorkspaceLayout,
} from "@/lib/layoutTypes"

// 这些符号原先定义在本文件，保留再导出避免调用方大规模改 import
export {
  FOLLOW_PANE_KINDS,
  isFollowPaneKind,
  GENERAL_PANE_KINDS,
  MARKET_PANE_KINDS,
  PANE_KINDS,
  PANE_KIND_LABELS,
  PANE_HORIZONTAL_ONLY,
  countLeaves,
  eachLeaf,
  findLeaf,
  isMarketPaneKind,
  paneKinds,
  paneSizing,
  resolvePaneKind,
} from "@/lib/layoutTypes"
export type {
  DragonDetailTarget,
  GeneralPaneKind,
  LayoutNode,
  MarketPaneKind,
  PaneDetail,
  PaneKind,
  PaneLeaf,
  PlateDetailTarget,
  SplitNode,
  SplitOrientation,
  WorkspaceLayout,
} from "@/lib/layoutTypes"

// ── 存储 key ──────────────────────────────────────────────────────────────

const CUSTOM_KEY = "workspace-custom-layouts-v2"
const LEGACY_CUSTOM_KEY = "workspace-custom-layouts"
const PRESET_KINDS_KEY = "workspace-preset-kinds"
/** 内置布局里被"解绑"固定的格子（同样跟布局一起同步）。 */
const PRESET_SYMBOLS_KEY = "workspace-preset-symbols"
/** 上一版：只记"这一格显示哪个行情视图"。 */
const LEGACY_PRESET_VIEWS_KEY = "workspace-preset-views"
const ACTIVE_KEY = "workspace-active-layout"
/** 本地缓存最后一次写入的时间，用来和服务端快照比新旧。 */
const UPDATED_AT_KEY = "workspace-updated-at"
/** 上一版的单个布局 id，用于迁移。 */
const LEGACY_ACTIVE_KEY = "workspace-layout"

function newNodeId(prefix: string): string {
  const uuid = globalThis.crypto?.randomUUID?.()
  return `${prefix}-${uuid ?? Math.random().toString(36).slice(2, 10)}`
}

function split(id: string, orientation: SplitOrientation, children: LayoutNode[]): SplitNode {
  return { id, type: "split", orientation, children }
}

function pane(id: string, kind: PaneKind): PaneLeaf {
  return { id, type: "pane", kind }
}

export const PRESET_LAYOUTS: WorkspaceLayout[] = [
  {
    id: "review",
    name: "复盘",
    builtin: true,
    storageId: "chat-layout",
    // 就是整块对话：右侧那条"行情面板"竖条已经没用了——九个行情视图各自都是独立的
    // 面板类型，要看盘就把格子拆出来放，不再需要一条只跑 tab 的窄栏。
    root: pane("chat", "chat"),
  },
  {
    id: "ledger",
    name: "记账",
    builtin: true,
    storageId: "layout-ledger",
    // 右边上下分：模拟盘在上、自选在下——两张表都是全宽才好读，横着切三栏每栏都太窄
    root: split("ledger-root", "horizontal", [
      pane("chat", "chat"),
      split("ledger-stack", "vertical", [pane("roles", "roles"), pane("watchlist", "watchlist")]),
    ]),
  },
]

export const PRESET_LAYOUT_IDS = PRESET_LAYOUTS.map((layout) => layout.id)

// ── 树的读与写 ─────────────────────────────────────────────────────────────

/** 替换某个节点（保持其余分支的引用不变）。 */
function replaceNode(node: LayoutNode, id: string, replacement: LayoutNode): LayoutNode {
  if (node.id === id) return replacement
  if (node.type === "pane") return node
  return {
    ...node,
    children: node.children.map((child) => replaceNode(child, id, replacement)),
  }
}

/** 删除一个叶子；父级只剩一个孩子时把父级折叠掉。返回 null 表示整棵树没了。 */
function removeLeaf(node: LayoutNode, id: string): LayoutNode | null {
  if (node.type === "pane") return node.id === id ? null : node
  const children = node.children
    .map((child) => removeLeaf(child, id))
    .filter((child): child is LayoutNode => child !== null)
  if (children.length === 0) return null
  if (children.length === 1) return children[0]
  return { ...node, children }
}

/** 布局里已用到的行情类型，拆分新格时优先挑没用过的榜。 */
function collectMarketKinds(node: LayoutNode, into = new Set<PaneKind>()): Set<PaneKind> {
  eachLeaf(node, (leaf) => {
    if (isMarketPaneKind(leaf.kind)) into.add(leaf.kind)
  })
  return into
}

// ── 持久化 ────────────────────────────────────────────────────────────────

/** 深度上限：导入的是用户粘贴的 JSON，不做限制会直接被嵌套结构打爆调用栈。 */
const MAX_DEPTH = 6

function sanitizeNode(raw: unknown, depth = 0): LayoutNode | null {
  if (depth > MAX_DEPTH) return null
  if (!raw || typeof raw !== "object") return null
  const record = raw as Record<string, unknown>
  if (typeof record.id !== "string" || !record.id) return null

  if (record.type === "pane") {
    // 旧版是 kind:"market" + view:"dragon-tiger"，现在视图本身就是 kind
    if (record.kind === "market" && isRightPanelView(record.view)) {
      return { id: record.id, type: "pane", kind: record.view }
    }
    if (
      typeof record.kind !== "string" ||
      !(PANE_KINDS as readonly string[]).includes(record.kind)
    ) {
      return null
    }
    const kind = record.kind as PaneKind
    // 看盘格解绑后固定的那只票要留下来，换机器/重开也还在
    if (isFollowPaneKind(kind) && typeof record.symbol === "string" && record.symbol) {
      return {
        id: record.id,
        type: "pane",
        kind,
        symbol: record.symbol,
        symbolName: typeof record.symbolName === "string" ? record.symbolName : undefined,
      }
    }
    return { id: record.id, type: "pane", kind }
  }

  if (record.type === "split") {
    if (record.orientation !== "horizontal" && record.orientation !== "vertical") return null
    if (!Array.isArray(record.children)) return null
    const children = record.children
      .map((child) => sanitizeNode(child, depth + 1))
      .filter((child): child is LayoutNode => child !== null)
    if (children.length === 0) return null
    return { id: record.id, type: "split", orientation: record.orientation, children }
  }

  return null
}

/** 上一版是"列的数组"，迁移成一行左右分割，用户之前搭的样子不会丢。 */
function migrateLegacyLayout(raw: unknown): WorkspaceLayout | null {
  if (!raw || typeof raw !== "object") return null
  const record = raw as Record<string, unknown>
  if (typeof record.id !== "string" || !record.id || record.builtin) return null
  if (!Array.isArray(record.panes)) return null
  const panes = record.panes
    .map((item) => sanitizeNode({ ...(item as Record<string, unknown>), type: "pane" }))
    .filter((item): item is LayoutNode => item !== null)
  if (panes.length === 0) return null
  const id = record.id
  return {
    id,
    name: typeof record.name === "string" && record.name.trim() ? record.name.trim() : "自定义布局",
    builtin: false,
    storageId: typeof record.storageId === "string" ? record.storageId : `layout-${id}`,
    root: panes.length === 1 ? panes[0] : split(`${id}-root`, "horizontal", panes),
  }
}

function loadCustomLayouts(): WorkspaceLayout[] {
  try {
    const stored =
      localStorage.getItem(CUSTOM_KEY) ?? localStorage.getItem(LEGACY_CUSTOM_KEY) ?? "null"
    const parsed: unknown = JSON.parse(stored)
    if (!Array.isArray(parsed)) return []
    return parsed
      .map((item) => {
        const record = item as Record<string, unknown>
        const root = sanitizeNode(record?.root)
        if (root && typeof record.id === "string" && record.id) {
          return {
            id: record.id,
            name:
              typeof record.name === "string" && record.name.trim()
                ? record.name.trim()
                : "自定义布局",
            builtin: false,
            storageId:
              typeof record.storageId === "string" && record.storageId
                ? record.storageId
                : `layout-${record.id}`,
            root,
          } satisfies WorkspaceLayout
        }
        return migrateLegacyLayout(item)
      })
      .filter((layout): layout is WorkspaceLayout => layout !== null)
  } catch {
    return []
  }
}

/** 内置布局里被换过内容的面板：`{ 布局 id: { 节点 id: 面板类型 } }`。 */
function parsePresetKinds(raw: unknown): Record<string, Record<string, PaneKind>> {
  if (!raw || typeof raw !== "object" || Array.isArray(raw)) return {}
  const result: Record<string, Record<string, PaneKind>> = {}
  for (const [layoutId, value] of Object.entries(raw as Record<string, unknown>)) {
    if (!value || typeof value !== "object") continue
    const entries = Object.entries(value as Record<string, unknown>).filter(
      ([, kind]) => typeof kind === "string" && (PANE_KINDS as readonly string[]).includes(kind)
    ) as [string, PaneKind][]
    if (entries.length > 0) result[layoutId] = Object.fromEntries(entries)
  }
  return result
}

function loadPresetKinds(): Record<string, Record<string, PaneKind>> {
  try {
    // 旧 key 存的是"这一格显示哪个行情视图"，视图 id 正好也是现在的 kind，直接可读
    const stored =
      localStorage.getItem(PRESET_KINDS_KEY) ?? localStorage.getItem(LEGACY_PRESET_VIEWS_KEY)
    return parsePresetKinds(JSON.parse(stored ?? "null"))
  } catch {
    return {}
  }
}

/**
 * 内置布局里被解绑固定的格子：`{ 布局 id: { 节点 id: { code, name } } }`。
 *
 * 内置布局的树是代码里的预设（只读），钉住的股票没法写进叶子，所以和
 * `presetKinds` 一样走覆盖表，跟着快照一起存。
 */
export type PresetSymbols = Record<string, Record<string, { code: string; name: string }>>

function parsePresetSymbols(raw: unknown): PresetSymbols {
  if (!raw || typeof raw !== "object" || Array.isArray(raw)) return {}
  const result: PresetSymbols = {}
  for (const [layoutId, value] of Object.entries(raw as Record<string, unknown>)) {
    if (!value || typeof value !== "object") continue
    const out: Record<string, { code: string; name: string }> = {}
    for (const [nodeId, entry] of Object.entries(value as Record<string, unknown>)) {
      if (!entry || typeof entry !== "object") continue
      const item = entry as Record<string, unknown>
      if (typeof item.code !== "string" || !item.code) continue
      out[nodeId] = { code: item.code, name: typeof item.name === "string" ? item.name : item.code }
    }
    if (Object.keys(out).length > 0) result[layoutId] = out
  }
  return result
}

function loadPresetSymbols(): PresetSymbols {
  try {
    return parsePresetSymbols(JSON.parse(localStorage.getItem(PRESET_SYMBOLS_KEY) ?? "null"))
  } catch {
    return {}
  }
}

/**
 * 本机记着"上次看的是哪一页"；没记过返回 null（新机器 / 首次启动）。
 *
 * 注意 `persist()` 写的是 JSON，所以读回来是带引号的（`"custom-layout-xxx"`）。
 * 以前这里直接拿裸字符串去比对布局 id，永远对不上，于是每次启动都落回「复盘」——
 * 这就是"记不住上次用的布局"的根因。旧版本可能留过裸字符串，一并兼容。
 */
function readStoredActiveId(): string | null {
  let raw: string | null
  try {
    raw = localStorage.getItem(ACTIVE_KEY) ?? localStorage.getItem(LEGACY_ACTIVE_KEY)
  } catch {
    // localStorage 不可用或被禁用：当作没选过
    return null
  }
  if (!raw) return null
  try {
    const parsed: unknown = JSON.parse(raw)
    return typeof parsed === "string" && parsed ? parsed : raw
  } catch {
    return raw
  }
}

function loadActiveId(customs: WorkspaceLayout[]): string {
  const known = new Set([...PRESET_LAYOUT_IDS, ...customs.map((layout) => layout.id)])
  const stored = readStoredActiveId()
  if (stored && known.has(stored)) return stored
  return "review"
}

function loadLocalUpdatedAt(): number {
  try {
    const raw = Number(localStorage.getItem(UPDATED_AT_KEY))
    return Number.isFinite(raw) ? raw : 0
  } catch {
    return 0
  }
}

function persist(key: string, value: unknown) {
  try {
    localStorage.setItem(key, JSON.stringify(value))
  } catch {
    // 写入失败时仍更新内存状态，保持交互可用
  }
}

// ── 服务端快照 ────────────────────────────────────────────────────────────

/**
 * 存到后端的整份工作区状态：自定义布局 + 内置布局的内容覆盖 + 当前选中的布局。
 *
 * 面板宽度不在里面——那是 `react-resizable-panels` 按 `storageId` 存在本地的，
 * 属于"这台机器的窗口"，换设备不该跟着走（否则一块 1280 的屏会把 4K 屏的比例带歪）。
 */
export interface LayoutSnapshot {
  version: number
  /** 最后修改时间（epoch ms），用于新旧仲裁：关应用太快没推上去时不会丢改动。 */
  updatedAt: number
  activeId: string
  /** 内置布局里被换过的面板：`{ 布局 id: { 节点 id: 面板类型 } }`。 */
  presetKinds: Record<string, Record<string, PaneKind>>
  /** 内置布局里被解绑固定的股票：`{ 布局 id: { 节点 id: { code, name } } }`。 */
  presetSymbols: PresetSymbols
  layouts: { id: string; name: string; storageId: string; root: LayoutNode }[]
}

export const LAYOUT_SNAPSHOT_VERSION = 1

export function toSnapshot(state: {
  layouts: WorkspaceLayout[]
  activeId: string
  presetKinds: Record<string, Record<string, PaneKind>>
  presetSymbols: PresetSymbols
}): LayoutSnapshot {
  return {
    version: LAYOUT_SNAPSHOT_VERSION,
    updatedAt: Date.now(),
    activeId: state.activeId,
    presetKinds: state.presetKinds,
    presetSymbols: state.presetSymbols,
    layouts: state.layouts
      .filter((layout) => !layout.builtin)
      .map(({ id, name, storageId, root }) => ({ id, name, storageId, root })),
  }
}

/** 解析服务端快照；形状不对返回 null（调用方回落到本地缓存）。 */
export function parseSnapshot(raw: unknown): {
  layouts: WorkspaceLayout[]
  presetKinds: Record<string, Record<string, PaneKind>>
  presetSymbols: PresetSymbols
  activeId: string
  updatedAt: number
} | null {
  if (!raw || typeof raw !== "object" || Array.isArray(raw)) return null
  const record = raw as Record<string, unknown>
  if (!Array.isArray(record.layouts)) return null

  const layouts: WorkspaceLayout[] = []
  for (const item of record.layouts) {
    if (!item || typeof item !== "object") continue
    const layout = item as Record<string, unknown>
    if (typeof layout.id !== "string" || !layout.id) continue
    const root = sanitizeNode(layout.root)
    if (!root) continue
    layouts.push({
      id: layout.id,
      name:
        typeof layout.name === "string" && layout.name.trim() ? layout.name.trim() : "自定义布局",
      builtin: false,
      storageId:
        typeof layout.storageId === "string" && layout.storageId
          ? layout.storageId
          : `layout-${layout.id}`,
      root,
    })
  }

  // 旧快照里这个字段叫 presetViews，值就是视图 id，和现在的 kind 同构
  const presetKinds = parsePresetKinds(record.presetKinds ?? record.presetViews)

  const known = new Set([...PRESET_LAYOUT_IDS, ...layouts.map((layout) => layout.id)])
  const activeId =
    typeof record.activeId === "string" && known.has(record.activeId) ? record.activeId : "review"

  const updatedAt = typeof record.updatedAt === "number" ? record.updatedAt : 0

  return {
    layouts,
    presetKinds,
    presetSymbols: parsePresetSymbols(record.presetSymbols),
    activeId,
    updatedAt,
  }
}

// ── 导入 / 导出 ────────────────────────────────────────────────────────────

/** 单次导入最多接收的布局数，防止一份离谱的 JSON 把切换器塞满。 */
const IMPORT_LIMIT = 20

/** 导出文件格式版本，将来改结构时按它做兼容。 */
export const LAYOUT_EXPORT_VERSION = 1

export interface LayoutExportFile {
  app: "openatrading"
  kind: "workspace-layouts"
  version: number
  exportedAt: string
  layouts: { name: string; root: LayoutNode }[]
}

/** 只导出"怎么排"，不导出布局 id / 尺寸存档：导进来的东西永远是新的一份。 */
export function serializeLayouts(layouts: WorkspaceLayout[]): LayoutExportFile {
  return {
    app: "openatrading",
    kind: "workspace-layouts",
    version: LAYOUT_EXPORT_VERSION,
    exportedAt: new Date().toISOString(),
    layouts: layouts.map((layout) => ({ name: layout.name, root: layout.root })),
  }
}

/**
 * 宽松解析：既吃得下本文件导出的信封格式，也吃得下单独一棵树或一个 `{name, root}`。
 * 用户手抄一段布局过来时，不该因为少一层信封就报错。
 */
export function parseLayoutFile(raw: unknown): {
  layouts: { name: string; root: LayoutNode }[]
  error?: string
} {
  const items: unknown[] = Array.isArray(raw)
    ? raw
    : raw && typeof raw === "object" && Array.isArray((raw as { layouts?: unknown }).layouts)
      ? ((raw as { layouts: unknown[] }).layouts ?? [])
      : [raw]

  const layouts: { name: string; root: LayoutNode }[] = []
  for (const item of items) {
    if (!item || typeof item !== "object") continue
    const record = item as Record<string, unknown>
    const root = sanitizeNode("root" in record ? record.root : record)
    if (!root) continue
    const name = typeof record.name === "string" ? record.name.trim() : ""
    layouts.push({ name, root })
  }

  if (layouts.length === 0) {
    return { layouts: [], error: "没有可用的布局：缺少 root 字段，或节点格式不对" }
  }
  return { layouts }
}

const initialCustoms = loadCustomLayouts()
const initialPresetKinds = loadPresetKinds()
const initialPresetSymbols = loadPresetSymbols()

export interface ImportResult {
  ok: boolean
  /** ok 为 false 时给出原因。 */
  error?: string
  /** 导入成功后激活的布局 id。 */
  id?: string
  /** 本次导入了几个布局。 */
  count?: number
}

interface LayoutState {
  /** 内置预设 + 用户自定义，顺序即切换器里的展示顺序。 */
  layouts: WorkspaceLayout[]
  activeId: string
  /** 内置布局里被用户换过内容的格（结构只读，换行情内容可以）。 */
  presetKinds: Record<string, Record<string, PaneKind>>
  /** 内置布局里被解绑固定的格（同一张覆盖表的另一种值）。 */
  presetSymbols: PresetSymbols
  /** 正在结构编辑的自定义布局 id；null 表示不在编辑态。 */
  editingId: string | null
  /** 被放大到整块工作区的格子（不持久化，属于临时视图状态）。 */
  maximizedPaneId: string | null
  /** 鼠标所在/最近交互的格子，⌘↵ 的对象。 */
  activePaneId: string | null
  /**
   * 每一格自己打开的"子页面"**栈**：点股票 → K 线，点板块 → 板块详情。
   *
   * 用栈而不是单值：在板块详情里点成分股看 K 线，关掉 K 线应该回到板块详情，
   * 而不是一路退回榜单。
   *
   * 以前这些是全局状态或组件内部 useState：前者只有右侧行情面板会渲染（网格里点了
   * 没反应或跑到别的面板去），后者放大/还原一下就丢了。子页面必须属于它所在的那一格，
   * 且要能在换榜时统一关掉。
   */
  paneDetail: Record<string, PaneDetail[]>

  /** 从后端拉一次快照；服务端还没有存档时把本地缓存推上去。 */
  hydrate: () => Promise<void>
  setLayout: (id: string) => void
  /** 克隆成一份可编辑的自定义布局，返回新布局 id。内置预设同样可以克隆。 */
  cloneLayout: (id: string) => string
  /** 从白板新建：只有一格对话，其余由用户自己拆。 */
  createBlankLayout: () => string
  importLayouts: (payload: string) => ImportResult
  renameLayout: (id: string, name: string) => void
  deleteLayout: (id: string) => void
  resetPreset: (id: string) => void

  startEditing: (id: string) => void
  stopEditing: () => void

  setActivePane: (paneId: string) => void
  /** 把这只票设为工作区当前标的：所有跟随中的看盘格一起换票。 */
  selectStock: (symbol: string, name: string) => void
  openPanePlate: (paneId: string, plate: PlateDetailTarget) => void
  openPaneDragon: (paneId: string, item: DragonDetailTarget) => void
  closePaneDetail: (paneId: string) => void
  /** 最大化/还原某一格；传 null 表示还原。 */
  setMaximizedPane: (paneId: string | null) => void

  setPaneKind: (layoutId: string, nodeId: string, kind: PaneKind) => void
  /**
   * 把这一格**解绑**：固定在这一只上，不再跟工作区当前标的走。
   *
   * 跟随才是默认——格子上没有固定值就渲染当前标的。解绑是唯一能让它停下来的
   * 动作（看盘墙就是这么拼出来的：几格各解绑一只），`followActiveStock` 是反向操作。
   */
  setPaneSymbol: (layoutId: string, nodeId: string, symbol: string, name: string) => void
  /** 解绑的反向操作：清掉固定的股票，重新跟随工作区当前标的。 */
  followActiveStock: (layoutId: string, nodeId: string) => void
  splitPane: (layoutId: string, nodeId: string, orientation: SplitOrientation) => void
  removePane: (layoutId: string, nodeId: string) => void
}

/**
 * 服务端推送的防抖器。拖分隔条不会写布局、但拆分/改名/换视图会；连续操作只发一次，
 * 断网或网关没起来时静默失败——本地缓存已经写好了，下次改动会再推。
 */
let remoteTimer: ReturnType<typeof setTimeout> | null = null
/** 拉取完成前用户已经改过布局，就别再用服务端快照盖掉他的操作。 */
let localEditsPending = false

function scheduleRemoteSave(snapshot: LayoutSnapshot) {
  if (remoteTimer) clearTimeout(remoteTimer)
  remoteTimer = setTimeout(() => {
    remoteTimer = null
    void pushLayoutSnapshot(snapshot).catch(() => {
      // 后端不可用时保持静默：本地缓存是完整的，下一次改动会自动重试
    })
  }, 600)
}

export const useLayoutStore = create<LayoutState>((set, get) => {
  /** 本地缓存（毫秒级）与服务端（跨设备）同时写：前者保证刷新不丢，后者保证换机器还在。 */
  const commit = () => {
    const { layouts, presetKinds, presetSymbols, activeId } = get()
    persist(
      CUSTOM_KEY,
      layouts.filter((layout) => !layout.builtin)
    )
    persist(PRESET_KINDS_KEY, presetKinds)
    persist(PRESET_SYMBOLS_KEY, presetSymbols)
    persist(ACTIVE_KEY, activeId)
    persist(UPDATED_AT_KEY, Date.now())
    localEditsPending = true
    scheduleRemoteSave(toSnapshot({ layouts, activeId, presetKinds, presetSymbols }))
  }

  const updateLayout = (
    layoutId: string,
    updater: (layout: WorkspaceLayout) => WorkspaceLayout
  ): WorkspaceLayout[] =>
    get().layouts.map((layout) => (layout.id === layoutId ? updater(layout) : layout))

  /** 从覆盖表里去掉某一格的固定值；没有变化时原样返回（调用方据此跳过 set）。 */
  const withoutPresetSymbol = (
    all: PresetSymbols,
    layoutId: string,
    nodeId: string
  ): PresetSymbols => {
    const current = all[layoutId]
    if (!current?.[nodeId]) return all
    const next = { ...current }
    delete next[nodeId]
    const out = { ...all }
    if (Object.keys(next).length > 0) out[layoutId] = next
    else delete out[layoutId]
    return out
  }

  /** 关掉某一格上打开的"下一层页面"（换榜/换类型时用）。 */
  const clearPaneDetail = (paneId: string) =>
    set((state) => {
      if (!state.paneDetail[paneId]) return state
      const paneDetail = { ...state.paneDetail }
      delete paneDetail[paneId]
      return { paneDetail }
    })

  const guardEditable = (layoutId: string): WorkspaceLayout | null => {
    const layout = get().layouts.find((item) => item.id === layoutId)
    // 内置布局的结构是预设，改结构请先复制
    return layout && !layout.builtin ? layout : null
  }

  return {
    layouts: [...PRESET_LAYOUTS, ...initialCustoms],
    activeId: loadActiveId(initialCustoms),
    presetKinds: initialPresetKinds,
    presetSymbols: initialPresetSymbols,
    editingId: null,
    maximizedPaneId: null,
    activePaneId: null,
    paneDetail: {},

    hydrate: async () => {
      const pushLocal = async () => {
        const { layouts, presetKinds, presetSymbols, activeId } = get()
        const hasLocalState =
          layouts.some((layout) => !layout.builtin) ||
          Object.keys(presetKinds).length > 0 ||
          Object.keys(presetSymbols).length > 0 ||
          activeId !== "review"
        if (!hasLocalState) return
        const snapshot = toSnapshot({ layouts, activeId, presetKinds, presetSymbols })
        persist(UPDATED_AT_KEY, snapshot.updatedAt)
        await pushLayoutSnapshot(snapshot)
      }

      try {
        const remote = parseSnapshot(await fetchLayoutSnapshot())

        if (!remote) {
          // 服务端还没有存档（首次升级 / 新机器）：把本地已有的推上去
          await pushLocal()
          return
        }
        // 用户在拉取期间已经改过布局：以他的操作为准，别用旧快照盖掉
        if (localEditsPending) return

        const local = loadLocalUpdatedAt()
        if (local > remote.updatedAt) {
          // 本地更新（上次关应用太快，防抖还没推出去）：补推本地版本
          await pushLocal()
          return
        }
        if (local === remote.updatedAt) return

        // 快照里记着的布局可能已经不存在了（预设被删掉、或布局在别处删了）：
        // 落到第一个预设，避免 activeId 一直指向一个渲染不出来的 id
        const knownIds = new Set([...PRESET_LAYOUT_IDS, ...remote.layouts.map((l) => l.id)])
        // **"在看哪一页"以本机为准**：它是视图状态，不是共享内容。以前这里直接
        // 采纳快照里的 activeId，于是另一台设备（或 Web 端）切一下布局，这台机器
        // 下次启动就被改回去了——表现就是"记不住上次用的布局"。
        // 本机没选过（新机器/首次启动）才用快照记的那一页。
        const localChoice = readStoredActiveId()
        const activeId =
          localChoice && knownIds.has(localChoice)
            ? localChoice
            : knownIds.has(remote.activeId)
              ? remote.activeId
              : PRESET_LAYOUT_IDS[0]

        set({
          layouts: [...PRESET_LAYOUTS, ...remote.layouts],
          activeId,
          presetKinds: remote.presetKinds,
          presetSymbols: remote.presetSymbols,
        })
        // 把服务端版本缓存到本地，下次打开可以先用缓存渲染
        persist(
          CUSTOM_KEY,
          remote.layouts.filter((layout) => !layout.builtin)
        )
        persist(PRESET_KINDS_KEY, remote.presetKinds)
        persist(PRESET_SYMBOLS_KEY, remote.presetSymbols)
        persist(ACTIVE_KEY, activeId)
        persist(UPDATED_AT_KEY, remote.updatedAt)
      } catch {
        // 后端不可用：继续用本地缓存，不做提示（用户此刻并不需要知道）
      }
    },

    setLayout: (id) => {
      if (!get().layouts.some((layout) => layout.id === id)) return
      // 换布局后原来的格子 id 就不存在了，最大化与子页面状态必须一起清掉
      set({ activeId: id, editingId: null, maximizedPaneId: null, paneDetail: {} })
      commit()
    },

    cloneLayout: (id) => {
      const { layouts } = get()
      const source = layouts.find((layout) => layout.id === id) ?? layouts[0]
      const created = makeCustom(source.name, source.root, layouts)
      // 克隆出来的布局直接进编辑态：用户点"克隆"就是为了改它
      set({ layouts: [...layouts, created], activeId: created.id, editingId: created.id })
      commit()
      return created.id
    },

    createBlankLayout: () => {
      const layouts = get().layouts
      const id = `custom-${newNodeId("layout")}`
      const created: WorkspaceLayout = {
        id,
        name: uniqueName("新布局", new Set(layouts.map((layout) => layout.name))),
        builtin: false,
        storageId: `layout-${id}`,
        // 白板 = 一格对话；想再要什么内容，点右上角拆分 / 编辑面板加
        root: { id: newNodeId("pane"), type: "pane", kind: "chat" },
      }
      set({ layouts: [...layouts, created], activeId: id, editingId: id })
      commit()
      return id
    },

    importLayouts: (payload) => {
      let raw: unknown
      try {
        raw = JSON.parse(payload)
      } catch {
        return { ok: false, error: "不是合法的 JSON" }
      }
      const parsed = parseLayoutFile(raw)
      if (parsed.error || parsed.layouts.length === 0) {
        return { ok: false, error: parsed.error ?? "没有可用的布局" }
      }

      const { layouts } = get()
      const used = new Set(layouts.map((layout) => layout.name))
      const created: WorkspaceLayout[] = []
      for (const item of parsed.layouts.slice(0, IMPORT_LIMIT)) {
        // 导入的布局一律换新 id 与尺寸存档：不会覆盖本地同名布局，也不会继承别人的拖拽结果
        const id = `custom-${newNodeId("layout")}`
        created.push({
          id,
          name: uniqueName(item.name || "导入布局", used),
          builtin: false,
          storageId: `layout-${id}`,
          root: cloneWithNewIds(item.root),
        })
      }
      set({ layouts: [...layouts, ...created], activeId: created[0].id, editingId: null })
      commit()
      return { ok: true, id: created[0].id, count: created.length }
    },

    renameLayout: (id, name) => {
      const next = name.trim()
      if (!next) return
      set({
        layouts: updateLayout(id, (layout) =>
          layout.builtin ? layout : { ...layout, name: next }
        ),
      })
      commit()
    },

    deleteLayout: (id) => {
      const { layouts, activeId } = get()
      const target = layouts.find((layout) => layout.id === id)
      if (!target || target.builtin) return
      set({
        layouts: layouts.filter((layout) => layout.id !== id),
        activeId: activeId === id ? "review" : activeId,
        editingId: null,
      })
      commit()
    },

    resetPreset: (id) => {
      const layout = get().layouts.find((item) => item.id === id)
      if (!layout?.builtin) return
      const presetKinds = { ...get().presetKinds }
      delete presetKinds[id]
      const presetSymbols = { ...get().presetSymbols }
      delete presetSymbols[id]
      set({ presetKinds, presetSymbols })
      commit()
    },

    startEditing: (id) => {
      if (!guardEditable(id)) return
      // 结构编辑要看全貌，先还原再进编辑态
      set({ editingId: id, maximizedPaneId: null })
    },

    stopEditing: () => set({ editingId: null }),

    setActivePane: (paneId) => {
      if (get().activePaneId === paneId) return
      set({ activePaneId: paneId })
    },

    selectStock: (symbol, name) => useActiveStockStore.getState().set({ code: symbol, name }),

    openPanePlate: (paneId, plate) =>
      set((state) => ({
        paneDetail: {
          ...state.paneDetail,
          [paneId]: [...(state.paneDetail[paneId] ?? []), { type: "plate", plate }],
        },
      })),

    openPaneDragon: (paneId, item) =>
      set((state) => ({
        paneDetail: {
          ...state.paneDetail,
          [paneId]: [...(state.paneDetail[paneId] ?? []), { type: "dragon", item }],
        },
      })),

    // 关闭只退一层：上一层还留着就回到上一层
    closePaneDetail: (paneId) =>
      set((state) => {
        const stack = state.paneDetail[paneId]
        if (!stack || stack.length === 0) return state
        const paneDetail = { ...state.paneDetail }
        if (stack.length === 1) delete paneDetail[paneId]
        else paneDetail[paneId] = stack.slice(0, -1)
        return { paneDetail }
      }),

    setMaximizedPane: (paneId) =>
      set((state) => ({
        maximizedPaneId: paneId,
        activePaneId: paneId ?? state.activePaneId,
      })),

    setPaneKind: (layoutId, nodeId, kind) => {
      const layout = get().layouts.find((item) => item.id === layoutId)
      const leaf = layout ? findLeaf(layout.root, nodeId) : null
      if (!layout || !leaf) return

      // 内置布局里换内容不改结构：行情格换榜、或换成「个股详情 / 五档 / 下单 / 自选 /
      // 模拟盘」都允许，记在覆盖表里，预设本身不动。
      //
      // `market-tabs` 是 selfPanel（Panel 包裹方式由结构决定，运行期换不了）；
      // `chat` 可以切，但一个布局只能有一条（见 `PaneKindSwitcher` 的禁用逻辑）。
      if (layout.builtin) {
        if (kind === "market-tabs") return
        clearPaneDetail(nodeId)
        set({
          presetKinds: {
            ...get().presetKinds,
            [layoutId]: { ...(get().presetKinds[layoutId] ?? {}), [nodeId]: kind },
          },
          // 换类型时把旧的固定值丢掉：它是"上一格内容"的标的，留着只会让新格子
          // 莫名固定在一只票上
          presetSymbols: withoutPresetSymbol(get().presetSymbols, layoutId, nodeId),
        })
        commit()
        return
      }

      // 换榜就关掉这一格的子页面，否则新榜单会被上一层 K 线盖住
      clearPaneDetail(nodeId)
      set({
        layouts: updateLayout(layoutId, (item) => ({
          ...item,
          root: replaceNode(item.root, nodeId, {
            id: leaf.id,
            type: "pane",
            kind,
            // 换类型时把"解绑固定的股票"一起丢掉，避免残留脏字段
            ...(isFollowPaneKind(kind) ? { symbol: leaf.symbol, symbolName: leaf.symbolName } : {}),
          }),
        })),
      })
      commit()
    },

    setPaneSymbol: (layoutId, nodeId, symbol, name) => {
      const layout = get().layouts.find((item) => item.id === layoutId)
      if (!layout) return
      // 内置布局的树是代码里的预设，写不进去：固定的股票记在覆盖表里（同 presetKinds）
      if (layout.builtin) {
        set({
          presetSymbols: {
            ...get().presetSymbols,
            [layoutId]: {
              ...(get().presetSymbols[layoutId] ?? {}),
              [nodeId]: { code: symbol, name },
            },
          },
        })
        commit()
        return
      }
      set({
        layouts: updateLayout(layoutId, (item) => {
          const leaf = findLeaf(item.root, nodeId)
          if (!leaf) return item
          return {
            ...item,
            root: replaceNode(item.root, nodeId, { ...leaf, symbol, symbolName: name }),
          }
        }),
      })
      commit()
    },

    followActiveStock: (layoutId, nodeId) => {
      const layout = get().layouts.find((item) => item.id === layoutId)
      if (!layout) return
      if (layout.builtin) {
        const presetSymbols = withoutPresetSymbol(get().presetSymbols, layoutId, nodeId)
        if (presetSymbols === get().presetSymbols) return
        set({ presetSymbols })
        commit()
        return
      }
      set({
        layouts: updateLayout(layoutId, (item) => {
          const leaf = findLeaf(item.root, nodeId)
          if (!leaf || !leaf.symbol) return item
          // 解绑就是去掉这两个字段：渲染层见到没有 `symbol` 就回到跟随
          const next: PaneLeaf = { ...leaf }
          delete next.symbol
          delete next.symbolName
          return { ...item, root: replaceNode(item.root, nodeId, next) }
        }),
      })
      commit()
    },

    splitPane: (layoutId, nodeId, orientation) => {
      if (!guardEditable(layoutId)) return
      set({
        layouts: updateLayout(layoutId, (item) => {
          const leaf = findLeaf(item.root, nodeId)
          if (!leaf) return item
          // 新面板挑一个还没在当前布局里出现过的榜，避免拆出来就是重复的两张表
          const used = collectMarketKinds(item.root)
          const kind = MARKET_PANE_KINDS.find((candidate) => !used.has(candidate))
          const added: PaneLeaf = {
            id: newNodeId("pane"),
            kind: kind ?? "strong-stocks",
            type: "pane",
          }
          return {
            ...item,
            root: replaceNode(
              item.root,
              nodeId,
              split(newNodeId("split"), orientation, [leaf, added])
            ),
          }
        }),
      })
      commit()
    },

    removePane: (layoutId, nodeId) => {
      if (!guardEditable(layoutId)) return
      set({
        layouts: updateLayout(layoutId, (item) => {
          if (countLeaves(item.root) <= 1) return item
          const root = removeLeaf(item.root, nodeId)
          return root ? { ...item, root } : item
        }),
      })
      commit()
    },
  }
})

/** 名字冲突时补序号，"复盘 副本" 连点两次不会出现两个同名布局。 */
function uniqueName(base: string, used: Set<string>): string {
  if (!used.has(base)) {
    used.add(base)
    return base
  }
  let index = 2
  while (used.has(`${base} ${index}`)) index += 1
  const name = `${base} ${index}`
  used.add(name)
  return name
}

/** 从任意布局（含内置预设）派生一个新的自定义布局。 */
function makeCustom(
  sourceName: string,
  sourceRoot: LayoutNode,
  existing: WorkspaceLayout[]
): WorkspaceLayout {
  const id = `custom-${newNodeId("layout")}`
  return {
    id,
    name: uniqueName(`${sourceName} 副本`, new Set(existing.map((layout) => layout.name))),
    builtin: false,
    storageId: `layout-${id}`,
    root: cloneWithNewIds(sourceRoot),
  }
}

/** 深拷贝整棵树并换新 id，用于克隆/导入。 */
function cloneWithNewIds(node: LayoutNode): LayoutNode {
  if (node.type === "pane") return { ...node, id: newNodeId("pane") }
  return {
    ...node,
    id: newNodeId("split"),
    children: node.children.map(cloneWithNewIds),
  }
}

export function useActiveLayout(): WorkspaceLayout {
  return useLayoutStore(
    (state) => state.layouts.find((layout) => layout.id === state.activeId) ?? state.layouts[0]
  )
}
