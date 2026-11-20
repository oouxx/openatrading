/**
 * 思考级别：与 pi-rs `pi_ai::models::EXTENDED_THINKING_LEVELS` 对齐。
 *
 * 顺序即「思考量」从少到多，`clampThinkingLevel` 依赖这个顺序。
 */
export const THINKING_LEVELS = [
  { value: "off", label: "关闭" },
  { value: "minimal", label: "极简" },
  { value: "low", label: "低" },
  { value: "medium", label: "中" },
  { value: "high", label: "高" },
  { value: "xhigh", label: "极高" },
  { value: "max", label: "最高" },
]

/** 档位顺序（不含标签），clamp 时按它找邻近档位 */
const LEVEL_ORDER = THINKING_LEVELS.map((level) => level.value)

/** 拿不到模型信息时的兜底档位：不上 `xhigh`/`max`，它们需要模型显式声明 */
const FALLBACK_LEVELS = ["off", "minimal", "low", "medium", "high"]

/**
 * 下拉选项：只列模型声明支持的档位。
 *
 * 各模型差异很大——非推理模型只有 `off`，`claude-opus-5-5` 只有
 * `low~max`，`Ring-2.6-1T` 只有 `high`/`xhigh`，还有一批推理模型不支持
 * `off`。档位写死会把不支持的值塞给 pi-rs，再被静默 clamp。
 */
export function thinkingLevelOptions(
  supported?: readonly string[]
): { value: string; label: string }[] {
  const allowed = supported?.length ? supported : FALLBACK_LEVELS
  const ordered = THINKING_LEVELS.filter((level) => allowed.includes(level.value))
  // 模型声明了未知档位时至少保留它，避免下拉为空
  const unknown = allowed.filter((level) => !LEVEL_ORDER.includes(level))
  return [
    ...ordered.map(({ value, label }) => ({ value, label })),
    ...unknown.map((value) => ({ value, label: value })),
  ]
}

/**
 * 把级别收敛到模型支持的档位，逻辑与 pi-rs `clamp_thinking_level` 一致：
 * 先往更高档找，找不到再往低档找，最后退回第一个可用档位。
 */
export function clampThinkingLevel(level: string, supported?: readonly string[]): string {
  if (!supported?.length || supported.includes(level)) return level

  const index = LEVEL_ORDER.indexOf(level)
  if (index < 0) return supported[0]

  for (let i = index; i < LEVEL_ORDER.length; i += 1) {
    const candidate = LEVEL_ORDER[i]
    if (supported.includes(candidate)) return candidate
  }
  for (let i = index - 1; i >= 0; i -= 1) {
    const candidate = LEVEL_ORDER[i]
    if (supported.includes(candidate)) return candidate
  }
  return supported[0]
}
