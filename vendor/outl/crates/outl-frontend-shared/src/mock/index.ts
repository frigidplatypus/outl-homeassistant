/**
 * Browser mock of the Tauri backend, for running a client in a plain
 * browser (`bun run dev`, then open `?mock=1`). It exists so the UI can
 * be looked at, screenshotted and reviewed without a native build. It
 * is never part of a production bundle: clients only import it behind
 * `import.meta.env.DEV`.
 */
import { mockIPC, mockWindows } from "@tauri-apps/api/mocks";
import { createMockBackend } from "./backend";

export { createMockBackend } from "./backend";
export { MockWorkspace } from "./workspace";

/** Whether the current URL asked for the mock (`?mock` or `?mock=1`). */
export function wantsMock(search = globalThis.location?.search ?? ""): boolean {
  const v = new URLSearchParams(search).get("mock");
  return v !== null && v !== "0" && v !== "false";
}

export function installMockBackend(): void {
  mockWindows("main");
  const backend = createMockBackend();
  mockIPC((cmd, args) => backend(cmd, args as Record<string, unknown>), { shouldMockEvents: true });
  // Onboarding is per device and lives in localStorage; the mock is a
  // workspace that already exists, so skip it.
  try {
    localStorage.setItem("outl.onboarded", "1");
  } catch {
    // storage disabled, onboarding will show; harmless
  }
}
