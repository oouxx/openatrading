import { Fragment, useMemo, type ReactNode } from "react"
import { Group, Panel, Separator, useDefaultLayout } from "react-resizable-panels"
import RightPanel from "@/components/RightPanel"
import PaneEditOverlay from "@/components/PaneEditOverlay"
import PaneMaximizeButton from "@/components/PaneMaximizeButton"
import { Link2, Move } from "lucide-react"
import { Button } from "@/components/ui/button"
import PaneStockPage from "@/components/PaneStockPage"
import PaneKindSwitcher from "@/components/PaneKindSwitcher"
import { usePaneDragLink } from "@/hooks/usePaneDragLink"
import { usePaneDragStore } from "@/stores/paneDragStore"
import { cn } from "@/lib/utils"

import { PANE_DEFINITIONS } from "@/lib/paneRegistry"
import {
  countLeaves,
  findLeaf,
  groupOf,
  groupPeerLabels,
  paneSizing,
  resolvePaneKind,
  useLayoutStore,
  type LayoutNode,
  type PaneLeaf,
  type SplitNode,
  type SplitOrientation,
  type WorkspaceLayout,
} from "@/stores/layoutStore"

/**
 * 面板之间的拖拽分隔条：左右分割是竖条，上下分割是横条。
 *
 * 视觉只有 1px 细线，但用 `before` 把可抓取范围向两侧各撑 4px——3×3 的格子里
 * 分隔条本身只有 4px 宽，不给一点容错就会一直点空。双击可把这一组恢复默认比例
 * （react-resizable-panels 内建行为）。
 */
function PaneSeparator({
  orientation,
  children,
}: {
  orientation: SplitOrientation
  children?: ReactNode
}) {
  return (
    <Separator
      className={
        orientation === "horizontal"
          ? "bg-hairline data-[separator=active]:bg-brand/60 data-[separator=hover]:bg-brand/40 relative w-1 shrink-0 cursor-col-resize transition-colors before:absolute before:inset-y-0 before:-left-1 before:w-3 before:content-['']"
          : "bg-hairline data-[separator=active]:bg-brand/60 data-[separator=hover]:bg-brand/40 relative h-1 w-full shrink-0 cursor-row-resize transition-colors before:absolute before:-top-1 before:h-3 before:w-full before:content-['']"
      }
    >
      {children}
    </Separator>
  )
}

/**
 * 角落的"换这一格的内容"，用了覆盖表里的**实际类型**做当前值（内置预设换过榜/换过
 * 面板的格子要显示自己真正在放的东西）。
 */
function PaneKindButton({
  leaf,
  layout,
  alwaysVisible,
}: {
  leaf: PaneLeaf
  layout: WorkspaceLayout
  alwaysVisible?: boolean
}) {
  const presetKinds = useLayoutStore((s) => s.presetKinds)
  const kind = resolvePaneKind(layout, leaf, presetKinds)
  return (
    <PaneKindSwitcher
      layoutId={layout.id}
      leafId={leaf.id}
      kind={kind}
      alwaysVisible={alwaysVisible}
    />
  )
}

/**
 * 格子角落的拖动把手：按住拖到另一格上 = 进同一个联动组，拖到空白处 = 退出组。
 *
 * 它同时是**组标识**：在组里就换成链环图标 + 品牌色（每格恒定一枚，一对多也不会
 * 长出第二个徽标）。提示里会列出同组的其它格子。
 */
function PaneDragHandle({
  layoutId,
  leafId,
  linked,
  peers,
  alwaysVisible,
}: {
  layoutId: string
  leafId: string
  linked: boolean
  /** 同组其它格子的名字，用在提示里 */
  peers: string[]
  alwaysVisible?: boolean
}) {
  const begin = usePaneDragLink(layoutId)
  const dragging = usePaneDragStore((s) => s.sourceId === leafId)
  const Icon = linked ? Link2 : Move
  const label = linked
    ? `已联动${peers.length > 0 ? `：与${peers.map((name) => `「${name}」`).join("")}一组` : ""}（拖到空白处解除，拖到别的格改组）`
    : "拖到另一格联动（榜单与看盘格互拖都行）"

  return (
    <Button
      variant="ghost"
      size="icon-xs"
      aria-label={label}
      title={label}
      onPointerDown={(event) => begin(leafId, event)}
      className={cn(
        "bg-surface/90 border-hairline hover:bg-hover text-tertiary hover:text-primary absolute top-1.5 right-19 z-30 rounded-md border transition-opacity",
        alwaysVisible || dragging || linked
          ? "opacity-100"
          : "opacity-0 group-hover/pane:opacity-100",
        (dragging || linked) && "text-brand hover:text-brand"
      )}
    >
      <Icon />
    </Button>
  )
}

/**
 * 每格都挂把手：**两个方向都认**——把接收方（个股详情 / 对话格）拖到榜单上，同样
 * 等于"榜单派发到它"，关联方向永远由谁是派发方决定，跟谁被拖动无关。
 */
function PaneDragHandleSlot({
  leaf,
  layout,
  alwaysVisible,
}: {
  leaf: PaneLeaf
  layout: WorkspaceLayout
  alwaysVisible?: boolean
}) {
  // 订阅 links：拖拽改关联后把手要立刻亮/灭。关联的方向由谁是派发方决定，
  // 所以派发方和接收方都算"有关联"
  useLayoutStore((s) => s.groups[layout.id])
  const linked = groupOf(layout.id, leaf.id) !== null
  const peers = linked ? groupPeerLabels(layout.id, leaf.id) : []
  return (
    <PaneDragHandle
      layoutId={layout.id}
      leafId={leaf.id}
      linked={linked}
      peers={peers}
      alwaysVisible={alwaysVisible}
    />
  )
}

/** 首次打开时的分配比例：按各面板"想要多少宽度"归一化成百分比，避免超宽被裁。 */
function defaultSizes(children: LayoutNode[]): string[] {
  // 分割子树的权重按叶子数给：3×3 那种九格网格不该和对话栏五五开
  const weights = children.map((child) =>
    child.type === "pane" ? paneSizing(child.kind).defaultSize : 260 * countLeaves(child)
  )
  const total = weights.reduce((sum, weight) => sum + weight, 0)
  const percents = weights.map((weight) => Math.round((weight / total) * 10000) / 100)
  // 四舍五入会让合计差 0.01，补给最后一列，否则首屏会留一条缝
  const drift = 100 - percents.reduce((sum, value) => sum + value, 0)
  percents[percents.length - 1] = Math.round((percents[percents.length - 1] + drift) * 100) / 100
  return percents.map((value) => `${value}%`)
}

/**
 * 一格的定位外壳：`group/pane` 让右上角按钮能跟着悬停出现，顺便把"鼠标在哪一格"
 * 记进 store —— ⌘⇧F 需要一个作用对象，光标所在格就是最不意外的那一个。
 */
function PaneFrame({ paneId, children }: { paneId: string; children: ReactNode }) {
  const setActivePane = useLayoutStore((s) => s.setActivePane)
  const dragging = usePaneDragStore((s) => s.sourceId !== null)
  const isDropTarget = usePaneDragStore((s) => s.hoverId === paneId)

  return (
    <div
      data-pane-id={paneId}
      className="group/pane relative h-full min-h-0 min-w-0"
      onPointerEnter={() => setActivePane(paneId)}
      onPointerDown={() => setActivePane(paneId)}
    >
      {children}
      {/* 拖拽中：落点高亮 + 一句"松手关联到此格" */}
      {dragging && isDropTarget && (
        <div className="border-brand/70 bg-brand/[0.06] pointer-events-none absolute inset-0 z-40 rounded-md border-2 border-dashed">
          <span className="bg-brand text-pure-white text-caption shadow-modal absolute top-1/2 left-1/2 -translate-x-1/2 -translate-y-1/2 rounded-md px-2 py-1">
            松手关联到此格
          </span>
        </div>
      )}
    </div>
  )
}

/** 某一格铺满整个工作区时，用单列 Group 承载它（Panel 必须待在 Group 里）。 */
function MaximizedPaneLayout({
  leaf,
  layout,
  renderPane,
}: {
  leaf: PaneLeaf
  layout: WorkspaceLayout
  renderPane: (leaf: PaneLeaf) => ReactNode
}) {
  if (PANE_DEFINITIONS[leaf.kind].selfPanel) {
    return (
      <Group
        id={`${layout.storageId}:maximized`}
        orientation="horizontal"
        className="h-full min-h-0 min-w-0"
      >
        <RightPanel
          panelId={leaf.id}
          cornerActions={<PaneMaximizeButton leafId={leaf.id} alwaysVisible />}
        />
      </Group>
    )
  }

  return (
    <Group
      id={`${layout.storageId}:maximized`}
      orientation="horizontal"
      className="h-full min-h-0 min-w-0"
    >
      <Panel id={leaf.id} className="min-h-0 min-w-0">
        <PaneFrame paneId={leaf.id}>
          {renderPane(leaf)}
          <PaneStockPage leafId={leaf.id} />
          <PaneDragHandleSlot leaf={leaf} layout={layout} alwaysVisible />
          <PaneKindButton leaf={leaf} layout={layout} alwaysVisible />
          <PaneMaximizeButton leafId={leaf.id} alwaysVisible />
        </PaneFrame>
      </Panel>
    </Group>
  )
}

export default function LayoutNodeView({
  node,
  layout,
  renderPane,
  isRoot = true,
  parentOrientation,
}: {
  node: LayoutNode
  layout: WorkspaceLayout
  renderPane: (leaf: PaneLeaf) => ReactNode
  isRoot?: boolean
  /** 父级分割方向：决定这一格能不能换成"行情面板"。 */
  parentOrientation?: SplitOrientation
}) {
  const editing = useLayoutStore((s) => s.editingId === layout.id)
  const maximizedPaneId = useLayoutStore((s) => s.maximizedPaneId)

  // 根层有放大目标时只挂载那一格：其它分支整个不渲染，图表和轮询也不会在后台跑
  if (isRoot && maximizedPaneId) {
    const target = findLeaf(node, maximizedPaneId)
    if (target) return <MaximizedPaneLayout leaf={target} layout={layout} renderPane={renderPane} />
  }

  if (node.type === "pane") {
    // 自己就是 Panel 的类型（目前只有行情面板），必须待在 Group 里；
    // 整棵树只剩它一格时补一个单列 Group
    if (PANE_DEFINITIONS[node.kind].selfPanel) {
      return (
        <Group
          id={`${layout.storageId}:single`}
          orientation="horizontal"
          className="h-full min-h-0 min-w-0"
        >
          <RightPanel
            panelId={node.id}
            cornerActions={
              <>
                {!editing && <PaneMaximizeButton leafId={node.id} />}
                {editing && (
                  <PaneEditOverlay
                    leaf={node}
                    layout={layout}
                    parentOrientation={parentOrientation}
                  />
                )}
              </>
            }
          />
        </Group>
      )
    }
    return (
      <PaneFrame paneId={node.id}>
        {renderPane(node)}
        {/* 每一格都能当 K 线子页面的宿主：拖拽关联到哪一格就在哪一格开 */}
        <PaneStockPage leafId={node.id} />
        {!editing && <PaneDragHandleSlot leaf={node} layout={layout} />}
        {!editing && <PaneKindButton leaf={node} layout={layout} />}
        {!editing && <PaneMaximizeButton leafId={node.id} />}
        {editing && (
          <PaneEditOverlay leaf={node} layout={layout} parentOrientation={parentOrientation} />
        )}
      </PaneFrame>
    )
  }

  return (
    <SplitView
      node={node}
      layout={layout}
      renderPane={renderPane}
      isRoot={isRoot}
      editing={editing}
    />
  )
}

function SplitView({
  node,
  layout,
  renderPane,
  isRoot,
  editing,
}: {
  node: SplitNode
  layout: WorkspaceLayout
  renderPane: (leaf: PaneLeaf) => ReactNode
  isRoot: boolean
  editing: boolean
}) {
  // panelIds 必须是稳定引用：每次渲染都新建数组会让 defaultLayout 反复重算，
  // 首次没有存档时还会把用户刚拖好的宽度弹回去
  const panelIds = useMemo(() => node.children.map((child) => child.id), [node.children])
  // 每一层分割各存一份尺寸：根节点沿用布局自己的 key，老用户调好的宽度不会丢
  const { defaultLayout, onLayoutChanged } = useDefaultLayout({
    id: isRoot ? layout.storageId : `${layout.storageId}:${node.id}`,
    panelIds,
  })
  const sizes = defaultSizes(node.children)

  return (
    <Group
      id={isRoot ? layout.storageId : `${layout.storageId}:${node.id}`}
      orientation={node.orientation}
      defaultLayout={defaultLayout}
      onLayoutChanged={onLayoutChanged}
      className="h-full min-h-0 min-w-0"
    >
      {node.children.map((child, index) => (
        <Fragment key={child.id}>
          {index > 0 && <PaneSeparator orientation={node.orientation} />}
          {child.type === "pane" && PANE_DEFINITIONS[child.kind].selfPanel ? (
            // 行情面板自己就是 Panel（折叠成图标条依赖 collapsible），不能再包一层
            <RightPanel
              panelId={child.id}
              cornerActions={
                <>
                  {!editing && <PaneMaximizeButton leafId={child.id} />}
                  {editing && (
                    <PaneEditOverlay
                      leaf={child}
                      layout={layout}
                      parentOrientation={node.orientation}
                    />
                  )}
                </>
              }
            />
          ) : (
            <PanelWithNode
              node={child}
              layout={layout}
              renderPane={renderPane}
              orientation={node.orientation}
              defaultSize={sizes[index]}
            />
          )}
        </Fragment>
      ))}
    </Group>
  )
}

function PanelWithNode({
  node,
  layout,
  renderPane,
  orientation,
  defaultSize,
}: {
  node: LayoutNode
  layout: WorkspaceLayout
  renderPane: (leaf: PaneLeaf) => ReactNode
  orientation: SplitOrientation
  defaultSize: string
}) {
  const kind = node.type === "pane" ? node.kind : null
  const minSize = kind
    ? // 上下分割里的下限按高度算，用宽度下限会直接放不下
      orientation === "horizontal"
      ? paneSizing(kind).minSize
      : Math.min(paneSizing(kind).minSize, 160)
    : 200

  return (
    <Panel id={node.id} minSize={minSize} defaultSize={defaultSize} className="min-h-0 min-w-0">
      <LayoutNodeView
        node={node}
        layout={layout}
        renderPane={renderPane}
        isRoot={false}
        parentOrientation={orientation}
      />
    </Panel>
  )
}
