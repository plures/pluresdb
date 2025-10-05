import { writable } from "svelte/store";

export type Toast = {
  id: number;
  kind?: "success" | "error" | "info";
  text: string;
  timeout?: number;
};

let nextId = 1;
export const toasts = writable<Toast[]>([]);

export function push(text: string, kind: Toast["kind"] = "info", timeout = 2000) {
  const id = nextId++;
  const t: Toast = { id, text, kind, timeout };
  toasts.update((list) => [...list, t]);
  if (timeout > 0) setTimeout(() => dismiss(id), timeout);
}

export function dismiss(id: number) {
  toasts.update((list) => list.filter((t) => t.id !== id));
}
