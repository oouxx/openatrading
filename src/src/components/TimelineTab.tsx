import { useDapanAbnormal } from "@/hooks/query/useDapanAbnormal"
import { DataEmpty, DataError, DataLoading } from "@/components/data-state"

function typeEmoji(t: number): string {
  if (t === 1) return "📈"
  if (t === 2) return "📉"
  return "📊"
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

  return (
    <div>
      {dapanLoading ? (
        <DataLoading label="加载大盘异动…" size="table" rows={5} />
      ) : dapanError ? (
        <DataError title="大盘异动获取失败" description={dapanError.message} />
      ) : (
        <>
          <div className="text-caption mb-6 flex items-center gap-3">
            <span className="text-secondary">
              共 <b className="text-primary">{dapanList.length}</b> 条异动
            </span>
          </div>

          {dapanList.length === 0 ? (
            <DataEmpty title="今日暂无大盘异动" size="table" />
          ) : (
            <div className="space-y-2">
              {dapanList.map((item, idx) => (
                <div
                  key={item.id || idx}
                  className="bg-surface border-hairline hover:bg-hover flex items-start gap-3 rounded-lg border px-5 py-4 transition-colors"
                >
                  <span className="text-caption mt-0.5 shrink-0">
                    {typeEmoji(item.bk_yi_dong_type)}
                  </span>
                  <div className="min-w-0 flex-1">
                    <div className="flex flex-wrap items-center gap-2">
                      <span className="text-h2 text-primary">{item.title}</span>
                      <span className="bg-brand/15 text-brand text-caption font-number rounded-sm px-2 py-0.5">
                        {item.bkj_info?.name || "—"}
                      </span>
                      <span className="text-caption font-number text-tertiary">
                        {formatTs(item.created_at)}
                      </span>
                    </div>
                  </div>
                </div>
              ))}
            </div>
          )}
        </>
      )}
    </div>
  )
}
