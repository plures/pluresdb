/**
 * `true` when the `PLURESDB_DEBUG` environment variable is set to `"1"` or
 * `"true"` (case-insensitive).  Works in both Deno and Node.js runtimes.
 */
export const DEBUG_ENABLED: boolean = (() => {
  try {
    let v = "";
    const g = globalThis as Record<string, unknown>;
    // Check for Deno environment
    if (typeof g.Deno !== "undefined") {
      const DenoRef = g.Deno as { env?: { get?: (k: string) => string | undefined } };
      if (DenoRef.env && DenoRef.env.get) {
        v = DenoRef.env.get("PLURESDB_DEBUG") ?? "";
      }
    } else {
      // Check for Node.js environment
      const globalProcess = g.process as
        | { env?: Record<string, string | undefined> }
        | undefined;
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
