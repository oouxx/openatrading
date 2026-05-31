import { useQuery } from "@tanstack/react-query";
import { fetchJson } from "@/stores/data";
import { aiRolesKeys } from "./queryKeys";

export interface AiRoleItem {
  name: string;
  display_name: string;
}

export function useAiRoles() {
  return useQuery({
    queryKey: aiRolesKeys.all,
    queryFn: () => fetchJson<AiRoleItem[]>("/api/ai/roles"),
    staleTime: Infinity,
  });
}
