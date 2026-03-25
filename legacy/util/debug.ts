/**
 * `true` when the `PLURESDB_DEBUG` environment variable is set to `"1"` or
 * `"true"` (case-insensitive).  Works in both Deno and Node.js runtimes.
 */
export const DEBUG_ENABLED: boolean = (() => {
  try {
    let v = "";
    // Check for Deno environment
    if (typeof (globalThis as any).Deno !== "undefined") {
      const Deno = (globalThis as any).Deno;
      if (Deno.env && Deno.env.get) {
        v = Deno.env.get("PLURESDB_DEBUG") ?? "";
      }
    } else {
      // Check for Node.js environment
      const globalProcess = (globalThis as any).process;
      if (typeof globalProcess !== "undefined" && globalProcess?.env) {
        v = globalProcess.env.PLURESDB_DEBUG ?? "";
      }
    }
    return v === "1" || v.toLowerCase() === "true";
  } catch {
    return false;
  }
})();

/**
 * Log a debug message to `console.log` when {@link DEBUG_ENABLED} is `true`.
 *
 * All arguments are prefixed with `[pluresdb]` for easy filtering.
 * In production builds with `PLURESDB_DEBUG` unset this function is a no-op.
 *
 * @param args - Any values to log.
 */
export function debugLog(...args: unknown[]): void {
  if (!DEBUG_ENABLED) return;
  // deno-lint-ignore no-console
  console.log("[pluresdb]", ...args);
}
