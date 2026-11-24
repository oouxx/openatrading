import MarketView from "@/components/MarketView"
import { PaneNavProvider } from "@/components/PaneNav"
import {
  Select,
  SelectContent,
  SelectGroup,
  SelectItem,
  SelectLabel,
  SelectTrigger,
  SelectValue,
} from "@/components/ui/select"
import { isMarketPaneKind, useLayoutStore, type PaneKind } from "@/stores/layoutStore"
import type { MarketPaneKind } from "@/stores/layoutStore"
import { SWITCHABLE_PANE_KINDS } from "@/lib/paneRegistry"
import PaneKindOption from "@/components/PaneKindOption"

/**
 * 行情格：一个格子只看一个榜，表头下拉是"这一格放什么"。
 *
 * 表头下拉是"这一格放什么"：九个行情榜和通用面板（个股详情 / 五档 / 下单 / 自选 /
 * 模拟盘）在同一份列表里，分组展示。行情格的换面板只在这一处，角落里不再重复一颗；
 * 非行情格没有这个表头，才退回角落的 `PaneKindSwitcher`。
 *
 * 点表格里的股票：这一格被拖到别的格上关联过，就在那一格打开 K 线；否则在本格开
 * 子页面。关联 / 解除都由格子角落的把手拖动完成（见 `usePaneDragLink`）。
 */
export default function MarketPane({
  leafId,
  layoutId,
  kind,
}: {
  leafId: string
  layoutId: string
  kind: MarketPaneKind
}) {
  const setPaneKind = useLayoutStore((s) => s.setPaneKind)

  return (
    <PaneNavProvider leafId={leafId} layoutId={layoutId}>
      <div className="bg-surface @container flex h-full min-w-0 flex-col overflow-hidden">
        <header className="border-hairline flex h-9 shrink-0 items-center gap-1 border-b px-1.5">
          {/* 这一格放什么：通用面板和九个行情榜在同一个下拉里（和角落里那份一模一样，
              所以角落那颗已经不再重复出现） */}
          <Select
            value={kind}
            onValueChange={(next) => setPaneKind(layoutId, leafId, next as PaneKind)}
          >
            <SelectTrigger
              size="sm"
              aria-label="这一格放什么"
              title="这一格放什么"
              className="text-caption text-primary hover:bg-hover h-7 min-w-0 gap-1.5 border-transparent bg-transparent px-1 font-medium shadow-none"
            >
              {/* 图标由 SelectValue 带出来（选中项本身就是 PaneKindOption：图标 + 名字） */}
              <SelectValue />
            </SelectTrigger>
            <SelectContent className="max-h-[70vh]">
              <SelectGroup>
                <SelectLabel>通用</SelectLabel>
                {SWITCHABLE_PANE_KINDS.filter((item) => !isMarketPaneKind(item)).map((item) => (
                  <SelectItem key={item} value={item}>
                    <PaneKindOption kind={item} />
                  </SelectItem>
                ))}
              </SelectGroup>
              <SelectGroup>
                <SelectLabel>行情</SelectLabel>
                {SWITCHABLE_PANE_KINDS.filter((item) => isMarketPaneKind(item)).map((item) => (
                  <SelectItem key={item} value={item}>
                    <PaneKindOption kind={item} />
                  </SelectItem>
                ))}
              </SelectGroup>
            </SelectContent>
          </Select>
        </header>

        <div className="relative min-h-0 flex-1 overflow-y-auto">
          <MarketView view={kind} boundary="MarketPane" />
        </div>
      </div>
    </PaneNavProvider>
  )
}
