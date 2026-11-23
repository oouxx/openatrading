import { fetchJson } from "@/stores/data"
import type { LayoutSnapshot } from "@/stores/layoutStore"

/**
 * 布局快照的读写。后端只做"搬运"（`{atrading_home}/workspace-layouts.json`），
 * 校验与语义都在前端，所以这里就是一次普通的 GET/PUT。
 */
export async function fetchLayoutSnapshot(): Promise<unknown | null> {
  const data = await fetchJson<unknown>("/api/layouts")
  return data ?? null
}

export async function pushLayoutSnapshot(snapshot: LayoutSnapshot): Promise<void> {
  await fetchJson("/api/layouts", {
    method: "PUT",
    body: JSON.stringify(snapshot),
  })
}
