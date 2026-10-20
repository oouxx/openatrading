import type { RefObject } from "react"
import type { IChartApi } from "lightweight-charts"

export interface CrosshairInfo {
  time: string
  open: string
  high: string
  low: string
  close: string
  volume: string
  change: string
  changeNum: number
}

/**
 * Cross-panel wiring for KlineChart. The chart registers its refs here, and
 * KlineDrawer reads them without importing component internals.
 */
export interface KlineChartStatics {
  __crosshairRef?: RefObject<((info: CrosshairInfo) => void) | null>
  __pendingCrosshairListener?: (info: CrosshairInfo) => void
  setCrosshairListener?: (fn: (info: CrosshairInfo) => void) => void
  chartRef?: RefObject<IChartApi | null>
}

const statics: KlineChartStatics = {}

export function getKlineChartStatics(): KlineChartStatics {
  return statics
}
