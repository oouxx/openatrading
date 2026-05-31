import { useMutation, useQueryClient } from "@tanstack/react-query";
import { fetchJson } from "@/stores/data";
import { roleKeys } from "./queryKeys";

export function useCreateRole() {
  const queryClient = useQueryClient();
  return useMutation({
    mutationFn: ({
      name,
      displayName,
      strategy,
    }: {
      name: string;
      displayName: string;
      strategy: string;
    }) =>
      fetchJson("/api/roles", {
        method: "POST",
        body: JSON.stringify({ name, display_name: displayName, strategy }),
      }),
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: roleKeys.all });
    },
  });
}

export function useUpdateRoleStrategy() {
  const queryClient = useQueryClient();
  return useMutation({
    mutationFn: ({
      name,
      strategy,
    }: {
      name: string;
      strategy: string;
    }) =>
      fetchJson(`/api/roles/${name}/strategy`, {
        method: "PUT",
        body: JSON.stringify({ strategy }),
      }),
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: roleKeys.all });
    },
  });
}

export function useDeleteRole() {
  const queryClient = useQueryClient();
  return useMutation({
    mutationFn: (name: string) =>
      fetchJson(`/api/roles/${name}`, { method: "DELETE" }),
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: roleKeys.all });
    },
  });
}

export function useResetRole() {
  const queryClient = useQueryClient();
  return useMutation({
    mutationFn: (name: string) =>
      fetchJson(`/api/roles/${name}/reset`, { method: "POST" }),
    onSuccess: (_data, name) => {
      queryClient.invalidateQueries({ queryKey: roleKeys.positions(name) });
      queryClient.invalidateQueries({ queryKey: roleKeys.logs(name) });
      queryClient.invalidateQueries({ queryKey: roleKeys.pnlHistory(name) });
    },
  });
}
