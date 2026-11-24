import { useEffect, useRef, useState } from "react"
import { AnimatePresence, motion } from "motion/react"
import { useActiveLayout, useLayoutStore } from "@/stores/layoutStore"
import { motionTransition } from "@/lib/motion"

/**
 * 切换布局时的短暂提示。
 *
 * 布局切换已经挪到设置里、靠 ⌥⌘1–9 触发，界面上不再有常驻的布局下拉——所以按完
 * 快捷键必须有一下确认"我现在在哪个布局"，否则九宫格变了但人不知道发生了什么。
 * 只在**布局 id 变化**时出现，首帧不弹。进入/退出结构编辑也给一句（⌘E / ⌘S 之后
 * 没有别的视觉反馈，光看边框变化太弱）。
 */
/** 编辑态切换的提示文案：进去说怎么改，出来说已保存。 */
const EDITING_TOAST = {
  enter: "编辑布局：拖分隔条调宽高，格子右下角可拆分 / 换内容",
  exit: "布局已保存",
} as const

export default function LayoutToast() {
  const active = useActiveLayout()
  const editingId = useLayoutStore((s) => s.editingId)
  const [shown, setShown] = useState<string | null>(null)
  const [editingToast, setEditingToast] = useState<string | null>(null)
  const prevEditing = useRef<string | null>(null)
  // 启动后紧接着的那次变化来自后端快照对齐（hydrate），不是用户操作，不弹提示
  const readyAt = useRef<number | null>(null)

  useEffect(() => {
    // 首次挂载时记下"可以开始提示"的时间点（渲染期读时间会让组件不纯）
    if (readyAt.current === null) readyAt.current = Date.now() + 800
    if (Date.now() < readyAt.current) return
    setShown(active.name)
    const timer = setTimeout(() => setShown(null), 1200)
    return () => clearTimeout(timer)
  }, [active.id, active.name])

  useEffect(() => {
    const prev = prevEditing.current
    prevEditing.current = editingId
    if (prev === editingId) return
    setEditingToast(editingId ? EDITING_TOAST.enter : EDITING_TOAST.exit)
    const timer = setTimeout(() => setEditingToast(null), 1800)
    return () => clearTimeout(timer)
  }, [editingId])

  return (
    <div className="pointer-events-none fixed inset-x-0 top-4 z-50 flex justify-center">
      <AnimatePresence>
        {(editingToast ?? shown) && (
          <motion.div
            key={editingToast ?? shown ?? ""}
            role="status"
            className="bg-surface border-hairline shadow-modal text-body text-primary rounded-full border px-3.5 py-1.5 font-medium"
            initial={{ opacity: 0, y: -6 }}
            animate={{ opacity: 1, y: 0 }}
            exit={{ opacity: 0 }}
            transition={motionTransition.fast}
          >
            {editingToast ?? shown}
          </motion.div>
        )}
      </AnimatePresence>
    </div>
  )
}
