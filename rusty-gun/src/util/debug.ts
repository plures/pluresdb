export const DEBUG_ENABLED: boolean = (() => {
  try {
    const v = Deno.env.get("PLURESDB_DEBUG") ?? "";
    return v === "1" || v.toLowerCase() === "true";
  } catch {
    return false;
  }
})();

export function debugLog(...args: unknown[]): void {
  if (!DEBUG_ENABLED) return;
  // deno-lint-ignore no-console
  console.log("[pluresdb]", ...args);
}




