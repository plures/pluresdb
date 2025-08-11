export const DEBUG_ENABLED: boolean = (() => {
  try {
    const v = Deno.env.get("RUSTY_GUN_DEBUG") ?? "";
    return v === "1" || v.toLowerCase() === "true";
  } catch {
    return false;
  }
})();

export function debugLog(...args: unknown[]): void {
  if (DEBUG_ENABLED) {
    // deno-lint-ignore no-console
    console.log("[rusty-gun]", ...args);
  }
}




