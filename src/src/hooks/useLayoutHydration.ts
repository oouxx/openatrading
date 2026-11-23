import { useEffect } from "react"
import { useLayoutStore } from "@/stores/layoutStore"

/**
 * 启动时把布局和后端对齐一次：服务端有存档就以它为准（本地只当缓存），
 * 没有存档就把本地已有的推上去（首次升级 / 换机器后的第一次写入）。
 *
 * 只在这里拉一次：布局是低频数据，没必要轮询，也不存在第二个写入口。
 */
export function useLayoutHydration() {
  const hydrate = useLayoutStore((s) => s.hydrate)

  useEffect(() => {
    void hydrate()
  }, [hydrate])
}
