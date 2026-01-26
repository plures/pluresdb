export const DEBUG_ENABLED: boolean = (() => {
  try {
    let v = "";
    // Check for Deno environment
    if (typeof (globalThis as any).Deno !== "undefined") {
      const Deno = (globalThis as any).Deno;
      if (Deno.env && Deno.env.get) {
        v = Deno.env.get("PLURESDB_DEBUG") ?? "";
      }
    } else if (typeof (globalThis as any).process !== "undefined") {
      // Check for Node.js environment
      const process = (globalThis as any).process;
      if (process && process.env) {
        v = process.env.PLURESDB_DEBUG ?? "";
      }
    }
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
