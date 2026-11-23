import { useEffect, useRef, useState } from "react"
import { AnimatePresence, motion } from "motion/react"
import { useActiveLayout } from "@/stores/layoutStore"
import { motionTransition } from "@/lib/motion"

/**
 * 切换布局时的短暂提示。
 *
 * 布局切换已经挪到设置里、靠 ⌥⌘1–9 触发，界面上不再有常驻的布局下拉——所以按完
 * 快捷键必须有一下确认"我现在在哪个布局"，否则九宫格变了但人不知道发生了什么。
 * 只在**布局 id 变化**时出现，首帧不弹。
 */
export default function LayoutToast() {
  const active = useActiveLayout()
  const [shown, setShown] = useState<string | null>(null)
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

  return (
    <div className="pointer-events-none fixed inset-x-0 top-4 z-50 flex justify-center">
      <AnimatePresence>
        {shown && (
          <motion.div
            key={shown}
            role="status"
            className="bg-surface border-hairline shadow-modal text-body text-primary rounded-full border px-3.5 py-1.5 font-medium"
            initial={{ opacity: 0, y: -6 }}
            animate={{ opacity: 1, y: 0 }}
            exit={{ opacity: 0 }}
            transition={motionTransition.fast}
          >
            {shown}
          </motion.div>
        )}
      </AnimatePresence>
    </div>
  )
}
