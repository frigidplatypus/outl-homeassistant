/**
 * Stub for `@tauri-apps/api/mocks`.
 *
 * Only the dev-only `?mock=1` path imports it (behind `import.meta.env.DEV`,
 * which is eliminated from production bundles). Provided so the module
 * resolves if it is ever pulled; the no-ops are never reached in the addon.
 */

export function mockIPC(_handler: unknown, _options?: unknown): void {
  // no-op
}

export function mockWindows(_label?: string): void {
  // no-op
}
