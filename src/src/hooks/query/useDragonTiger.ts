import { useQuery } from "@tanstack/react-query";
import { apiFetch } from "@/stores/data";
import { dragonTigerKeys } from "./queryKeys";

export interface SeatDetail {
  dept_name: string;
  seat_type: string | null;
  buy_amt: number | null;
  buy_ratio: number | null;
  sell_amt: number | null;
  sell_ratio: number | null;
  net_amt: number | null;
}

export interface DragonTigerEntry {
  code: string;
  name: string;
  trade_date: string;
  close_price: number | null;
  change_rate: number | null;
  billboard_net_amt: number | null;
  billboard_buy_amt: number | null;
  billboard_sell_amt: number | null;
  turnover_rate: number | null;
  free_market_cap: number | null;
  explanation: string | null;
  d1_chg: number | null;
  d2_chg: number | null;
  d5_chg: number | null;
  d10_chg: number | null;
  buy_seats: SeatDetail[];
  sell_seats: SeatDetail[];
}

export function useDragonTiger() {
  return useQuery({
    queryKey: dragonTigerKeys.all,
    queryFn: async () => {
      const resp = await apiFetch("/api/dragon-tiger/combined");
      if (!resp.ok) throw new Error(`HTTP ${resp.status}`);
      return resp.json() as Promise<DragonTigerEntry[]>;
    },
    staleTime: 60 * 1000,
  });
}
