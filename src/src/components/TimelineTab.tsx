import { ArrowDownRight, ArrowUpRight, Activity } from "lucide-react"
import { useDapanAbnormal } from "@/hooks/query/useDapanAbnormal"
import { DataEmpty, DataError, DataLoading } from "@/components/data-state"

function TypeIcon({ type }: { type: number }) {
  if (type === 1) return <ArrowUpRight className="text-up size-4" aria-hidden />
  if (type === 2) return <ArrowDownRight className="text-down size-4" aria-hidden />
  return <Activity className="text-tertiary size-4" aria-hidden />
}

function formatTs(ts: number): string {
  if (!ts) return ""
  const d = new Date(ts * 1000)
  const h = d.getHours().toString().padStart(2, "0")
  const m = d.getMinutes().toString().padStart(2, "0")
  return `${h}:${m}`
}

export default function TimelineTab() {
  const { data: dapanList = [], isLoading: dapanLoading, error: dapanError } = useDapanAbnormal()

  if (dapanLoading) {
    return (
      <div className="@container p-4 @2xl:p-6">
        <DataLoading label="加载大盘异动…" size="table" rows={5} />
      </div>
    )
  }

  if (dapanError) {
    return (
      <div className="@container p-4 @2xl:p-6">
        <DataError title="大盘异动获取失败" description={dapanError.message} />
      </div>
    )
  }

  return (
    <div className="@container p-4 @2xl:p-6">
      <p className="text-caption text-tertiary pb-3">共 {dapanList.length} 条异动</p>

      {dapanList.length === 0 ? (
        <DataEmpty title="今日暂无大盘异动" size="table" />
      ) : (
        <div className="border-hairline bg-surface divide-hairline divide-y overflow-hidden rounded-lg border">
          {dapanList.map((item, idx) => (
            <div key={item.id || idx} className="flex items-start gap-3 px-4 py-3.5">
              <span className="mt-0.5 shrink-0">
                <TypeIcon type={item.bk_yi_dong_type} />
              </span>
              <div className="flex min-w-0 flex-1 flex-wrap items-center gap-2">
                <span className="text-body text-primary">{item.title}</span>
                <span className="bg-hover text-secondary text-micro rounded-sm px-1.5 py-0.5">
                  {item.bkj_info?.name || "—"}
                </span>
              </div>
              <span className="text-tertiary text-caption font-number shrink-0">
                {formatTs(item.created_at)}
              </span>
            </div>
          ))}
        </div>
      )}
    </div>
  )
}
