export const DEBUG_ENABLED: boolean = (() => {
  try {
    let v = "";
    // Check for Deno environment
    if (typeof (globalThis as any).Deno !== "undefined") {
      const Deno = (globalThis as any).Deno;
      if (Deno.env && Deno.env.get) {
        v = Deno.env.get("PLURESDB_DEBUG") ?? "";
      }
    } else if (typeof process !== "undefined" && process.env) {
      // Check for Node.js environment
      v = process.env.PLURESDB_DEBUG ?? "";
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
